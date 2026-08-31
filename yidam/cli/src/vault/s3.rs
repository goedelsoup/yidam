//! The S3 backend — three verbs, signed, over the `reqwest` already in the default build.
//!
//! # It owns its runtime
//!
//! [`super::Store`] is synchronous, for the reason `store.rs` gives: an async trait would put
//! `tokio` in the signature of every caller and therefore in the ungated half of this module,
//! spending what the split bought for operations that are one command invocation rather than
//! a loop. So the runtime is built here and blocked on here. Nothing above this file knows
//! there is one.
//!
//! # Streaming, in one direction and not the other
//!
//! A PUT streams from disk, because a vector index runs to hundreds of megabytes and
//! buffering one to upload it would make the memory cost of storing an artifact proportional
//! to its size. That is possible only because `x-amz-content-sha256` is known before the body
//! is read — which a content-addressed store always knows, since naming the artifact *is*
//! computing its digest.
//!
//! A GET streams to a temporary file beside the destination and renames, so a reader never
//! finds a partial artifact under a name that asserts its contents.
//!
//! # No multipart
//!
//! A single PUT caps at 5 GiB, which is S3's limit and not a choice made here. Over that the
//! upload is refused with a message that says so rather than failing at the server with an
//! `EntityTooLarge` nobody can act on. Multipart is a second signing surface and a state
//! machine; RFC-0023 defers it and names the limit rather than discovering it.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::cas::ContentHash;
use super::config::VaultConfig;
use super::sigv4::{Credentials, Signable, EMPTY_PAYLOAD_SHA256};
use super::store::Store;

/// The largest body a single `PUT` may carry. S3's limit, not ours.
pub const MAX_SINGLE_PUT: u64 = 5 * 1024 * 1024 * 1024;

/// Where an `s3://` url points.
#[derive(Debug, PartialEq, Eq)]
pub struct S3Location {
    pub bucket: String,
    /// May be empty, for a store rooted at the bucket.
    pub prefix: String,
}

/// Parse `s3://bucket[/prefix…]`.
pub fn parse_s3_url(url: &str) -> Result<S3Location> {
    let rest = url
        .strip_prefix("s3://")
        .context("not an s3:// url")?
        .trim_matches('/');
    let (bucket, prefix) = match rest.split_once('/') {
        Some((b, p)) => (b, p),
        None => (rest, ""),
    };
    if bucket.is_empty() {
        bail!("vault url {url:?} names no bucket — expected `s3://bucket/prefix`");
    }
    Ok(S3Location {
        bucket: bucket.to_string(),
        prefix: prefix.trim_matches('/').to_string(),
    })
}

pub struct S3Store {
    location: S3Location,
    region: String,
    /// Scheme and authority, e.g. `https://s3.example.net`. Derived from `endpoint`, or from
    /// the region when the store is AWS itself.
    endpoint: String,
    path_style: bool,
    creds: Credentials,
    runtime: tokio::runtime::Runtime,
    /// A clock, injected so the signing path is exercisable at a fixed time.
    now: fn() -> u64,
}

impl S3Store {
    pub fn new(vault: &str, cfg: &VaultConfig) -> Result<Self> {
        let location = parse_s3_url(&cfg.url)?;
        let region = cfg.region.clone().unwrap_or_else(|| {
            // Every S3-compatible store needs *a* region in the signing scope, and MinIO and
            // friends conventionally accept this one. Defaulting is better than refusing: a
            // corpus on a local MinIO has no meaningful region to declare.
            "us-east-1".to_string()
        });
        let endpoint = match &cfg.endpoint {
            Some(e) => e.trim_end_matches('/').to_string(),
            None => format!("https://s3.{region}.amazonaws.com"),
        };
        // Path style by default when an endpoint is named: MinIO, Ceph and R2 all want it,
        // and a custom endpoint is overwhelmingly one of those. AWS itself prefers
        // virtual-host, which is what the absent-endpoint branch gets.
        let path_style = cfg.path_style.unwrap_or(cfg.endpoint.is_some());
        let creds = super::creds::resolve(vault, |k| std::env::var(k).ok())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("building the runtime for the S3 transport")?;
        Ok(S3Store {
            location,
            region,
            endpoint,
            path_style,
            creds,
            runtime,
            now: unix_now,
        })
    }

    /// The object key for an artifact, without a leading slash.
    fn key(&self, hash: &ContentHash) -> String {
        hash.key(&self.location.prefix)
    }

    /// The URL and the `Host` header for one key.
    ///
    /// Returned together because the signature covers `host` verbatim: a request sent to one
    /// authority and signed for another is rejected with no explanation of which half is
    /// wrong.
    fn target(&self, key: &str) -> Result<(String, String, String)> {
        let (scheme, authority) = split_endpoint(&self.endpoint)?;
        if self.path_style {
            let path = format!("/{}/{key}", self.location.bucket);
            Ok((
                format!("{scheme}://{authority}{path}"),
                authority.to_string(),
                path,
            ))
        } else {
            let host = format!("{}.{authority}", self.location.bucket);
            let path = format!("/{key}");
            Ok((format!("{scheme}://{host}{path}"), host, path))
        }
    }

    /// Everything a request needs, signed. Also what `--dry-run` renders.
    pub fn sign(
        &self,
        method: &str,
        hash: &ContentHash,
        payload_sha256: &str,
    ) -> Result<SignedRequest> {
        let key = self.key(hash);
        let (url, host, path) = self.target(&key)?;
        let timestamp = crate::dates::amz_datetime((self.now)());
        let signable = Signable {
            method,
            host: &host,
            path: &path,
            query: "",
            payload_sha256,
            timestamp: &timestamp,
            region: &self.region,
        };
        Ok(SignedRequest {
            canonical_request: signable.canonical_request(&self.creds),
            headers: signable.headers_to_send(&self.creds),
            method: method.to_string(),
            url,
        })
    }

    fn client(&self) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .user_agent(concat!("yidam-vault/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building the HTTP client")
    }
}

/// A request with its signature already computed.
pub struct SignedRequest {
    /// The exact string that was signed. `vault push --dry-run` prints this, because it is
    /// the only artifact of a signing bug a person can actually inspect.
    pub canonical_request: String,
    pub headers: Vec<(String, String)>,
    pub method: String,
    pub url: String,
}

fn split_endpoint(endpoint: &str) -> Result<(&str, &str)> {
    endpoint
        .split_once("://")
        .filter(|(s, a)| !s.is_empty() && !a.is_empty())
        .context(format!(
            "vault endpoint {endpoint:?} has no scheme — expected `https://host[:port]`"
        ))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Turn a non-success response into an error that says what the server actually said.
///
/// S3 reports failures as an XML body, and a bare status code sends a reader hunting. The
/// body is included verbatim and truncated, because it is the only place a `SignatureDoesNotMatch`
/// says which header it disagreed about.
async fn check(resp: reqwest::Response, what: &str) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let body = body.trim();
    let shown: String = body.chars().take(600).collect();
    bail!(
        "{what} failed: HTTP {status}{}",
        if shown.is_empty() {
            String::new()
        } else {
            format!("\n  {shown}")
        }
    )
}

impl Store for S3Store {
    fn describe(&self) -> String {
        let prefix = if self.location.prefix.is_empty() {
            String::new()
        } else {
            format!("/{}", self.location.prefix)
        };
        format!("s3://{}{prefix}", self.location.bucket)
    }

    fn has(&self, hash: &ContentHash) -> Result<bool> {
        let req = self.sign("HEAD", hash, EMPTY_PAYLOAD_SHA256)?;
        let client = self.client()?;
        self.runtime.block_on(async {
            let mut r = client.head(&req.url);
            for (k, v) in &req.headers {
                r = r.header(k, v);
            }
            let resp = r
                .send()
                .await
                .with_context(|| format!("HEAD {}", req.url))?;
            if resp.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(false);
            }
            check(resp, &format!("HEAD {}", req.url)).await?;
            Ok(true)
        })
    }

    fn get(&self, hash: &ContentHash, dest: &Path) -> Result<()> {
        let req = self.sign("GET", hash, EMPTY_PAYLOAD_SHA256)?;
        let client = self.client()?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let tmp = temp_beside(dest, hash);
        let result = self.runtime.block_on(async {
            let mut r = client.get(&req.url);
            for (k, v) in &req.headers {
                r = r.header(k, v);
            }
            let resp = r.send().await.with_context(|| format!("GET {}", req.url))?;
            let mut resp = check(resp, &format!("GET {}", req.url)).await?;
            // Streamed rather than `bytes()`: the whole point of a vault is that it holds
            // things too big to want in memory.
            let mut file = std::fs::File::create(&tmp)
                .with_context(|| format!("creating {}", tmp.display()))?;
            while let Some(chunk) = resp.chunk().await.context("reading the response body")? {
                use std::io::Write;
                file.write_all(&chunk)
                    .with_context(|| format!("writing {}", tmp.display()))?;
            }
            file.sync_all().ok();
            anyhow::Ok(())
        });
        if let Err(e) = result {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        std::fs::rename(&tmp, dest)
            .with_context(|| format!("moving {} into place at {}", tmp.display(), dest.display()))
    }

    fn explain_put(&self, hash: &ContentHash) -> Option<String> {
        self.sign("PUT", hash, hash.as_str())
            .ok()
            .map(|r| format!("{} {}\n\n{}", r.method, r.url, r.canonical_request))
    }

    fn put(&self, hash: &ContentHash, src: &Path) -> Result<()> {
        let size = std::fs::metadata(src)
            .with_context(|| format!("reading {}", src.display()))?
            .len();
        if size > MAX_SINGLE_PUT {
            bail!(
                "{} is {size} bytes, and a single PUT caps at {MAX_SINGLE_PUT}.\n  \
                 Multipart upload is specified in RFC-0023 and not built. Refused here rather \
                 than at the server, which reports this as `EntityTooLarge` after the upload.",
                src.display()
            );
        }
        // The payload hash is the artifact's own name, which is the property that makes a
        // streamed body signable at all — see the module header.
        let req = self.sign("PUT", hash, hash.as_str())?;
        let client = self.client()?;
        let src = src.to_path_buf();
        self.runtime.block_on(async move {
            let file = tokio::fs::File::open(&src)
                .await
                .with_context(|| format!("opening {}", src.display()))?;
            let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file));
            let mut r = client
                .put(&req.url)
                .header("content-length", size)
                .body(body);
            for (k, v) in &req.headers {
                r = r.header(k, v);
            }
            let resp = r.send().await.with_context(|| format!("PUT {}", req.url))?;
            check(resp, &format!("PUT {}", req.url)).await?;
            anyhow::Ok(())
        })
    }
}

fn temp_beside(dest: &Path, hash: &ContentHash) -> PathBuf {
    let dir = dest.parent().unwrap_or(Path::new("."));
    dir.join(format!(".{}.{}.part", hash.as_str(), std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(url: &str, endpoint: Option<&str>, path_style: Option<bool>) -> VaultConfig {
        VaultConfig {
            url: url.to_string(),
            audience: Some("tests".into()),
            holds: None,
            region: Some("us-east-1".into()),
            endpoint: endpoint.map(str::to_string),
            path_style,
        }
    }

    fn store(url: &str, endpoint: Option<&str>, path_style: Option<bool>) -> S3Store {
        let c = cfg(url, endpoint, path_style);
        let location = parse_s3_url(&c.url).unwrap();
        S3Store {
            location,
            region: "us-east-1".into(),
            endpoint: c
                .endpoint
                .clone()
                .unwrap_or_else(|| "https://s3.us-east-1.amazonaws.com".into()),
            path_style: c.path_style.unwrap_or(c.endpoint.is_some()),
            creds: Credentials {
                access_key_id: "AKIDEXAMPLE".into(),
                secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
                session_token: None,
            },
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
            // A fixed clock, so a signature is a function of its inputs and a golden can pin
            // the canonical request. 2015-08-30T12:36:00Z.
            now: || 1440938160,
        }
    }

    fn hash() -> ContentHash {
        ContentHash::parse("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            .unwrap()
    }

    #[test]
    fn an_s3_url_splits_into_a_bucket_and_a_prefix() {
        assert_eq!(
            parse_s3_url("s3://corpus-artifacts/yidam").unwrap(),
            S3Location {
                bucket: "corpus-artifacts".into(),
                prefix: "yidam".into()
            }
        );
        assert_eq!(
            parse_s3_url("s3://bucket").unwrap(),
            S3Location {
                bucket: "bucket".into(),
                prefix: String::new()
            }
        );
        assert_eq!(
            parse_s3_url("s3://bucket/a/b/").unwrap().prefix,
            "a/b".to_string()
        );
        assert!(parse_s3_url("s3://").is_err());
    }

    /// Path style puts the bucket in the path and signs the endpoint's host; virtual style
    /// puts it in the host and signs *that*. Getting the pair out of step is the classic S3
    /// signing bug and the server reports it only as a bad signature.
    #[test]
    fn addressing_style_moves_the_bucket_between_host_and_path_consistently() {
        let s = store("s3://bucket/pre", Some("https://minio.local:9000"), None);
        let req = s.sign("GET", &hash(), EMPTY_PAYLOAD_SHA256).unwrap();
        assert!(req
            .url
            .starts_with("https://minio.local:9000/bucket/pre/sha256/ba/"));
        assert!(
            req.canonical_request.contains("host:minio.local:9000"),
            "{}",
            req.canonical_request
        );
        assert!(req
            .canonical_request
            .contains("/bucket/pre/sha256/ba/ba7816bf"));

        let s = store("s3://bucket/pre", None, None);
        let req = s.sign("GET", &hash(), EMPTY_PAYLOAD_SHA256).unwrap();
        assert!(req
            .url
            .starts_with("https://bucket.s3.us-east-1.amazonaws.com/pre/sha256/ba/"));
        assert!(
            req.canonical_request
                .contains("host:bucket.s3.us-east-1.amazonaws.com"),
            "{}",
            req.canonical_request
        );
    }

    /// A custom endpoint means MinIO, Ceph or R2 far more often than not, and all three want
    /// path style. AWS itself gets virtual-host. An explicit setting beats both.
    #[test]
    fn path_style_defaults_to_whether_an_endpoint_was_named() {
        assert!(store("s3://b", Some("https://h"), None).path_style);
        assert!(!store("s3://b", None, None).path_style);
        assert!(!store("s3://b", Some("https://h"), Some(false)).path_style);
    }

    /// A PUT signs the artifact's own digest as its payload hash. That is what lets the body
    /// stream from disk, and it is the property the whole design turns on.
    #[test]
    fn a_put_signs_the_artifacts_digest_and_a_get_signs_the_empty_body() {
        let s = store("s3://b/p", Some("https://h"), None);
        let put = s.sign("PUT", &hash(), hash().as_str()).unwrap();
        assert!(put
            .canonical_request
            .contains(&format!("x-amz-content-sha256:{}", hash())));
        assert!(put.canonical_request.starts_with("PUT\n"));

        let get = s.sign("GET", &hash(), EMPTY_PAYLOAD_SHA256).unwrap();
        assert!(get
            .canonical_request
            .contains(&format!("x-amz-content-sha256:{EMPTY_PAYLOAD_SHA256}")));
    }

    /// Every header that was signed is handed to the sender. A signature covering a header
    /// the request omits is rejected with no indication of which one.
    #[test]
    fn the_signed_request_carries_authorization_and_every_signed_header() {
        let s = store("s3://b/p", Some("https://h"), None);
        let req = s.sign("GET", &hash(), EMPTY_PAYLOAD_SHA256).unwrap();
        let names: Vec<&str> = req.headers.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"authorization"));
        assert!(names.contains(&"host"));
        assert!(names.contains(&"x-amz-date"));
        assert!(names.contains(&"x-amz-content-sha256"));
    }

    #[test]
    fn an_endpoint_without_a_scheme_is_refused_rather_than_guessed_at() {
        let mut s = store("s3://b", Some("https://h"), None);
        s.endpoint = "minio.local:9000".into();
        let err = s
            .sign("GET", &hash(), EMPTY_PAYLOAD_SHA256)
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("no scheme"), "{err}");
    }

    #[test]
    fn describe_names_the_bucket_and_prefix_and_never_a_credential() {
        let s = store("s3://bucket/pre", Some("https://h"), None);
        assert_eq!(s.describe(), "s3://bucket/pre");
        assert!(!s.describe().contains("AKIDEXAMPLE"));
        assert_eq!(store("s3://bucket", None, None).describe(), "s3://bucket");
    }
}
