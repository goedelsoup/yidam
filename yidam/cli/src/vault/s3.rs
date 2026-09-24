//! The S3 backend — three verbs, signed, over the `reqwest` already in the default build.
//!
//! # It blocks on the runtime, and does not own one
//!
//! [`super::Store`] is synchronous, for the reason `store.rs` gives: an async trait would put
//! `tokio` in the signature of every caller and therefore in the ungated half of this module,
//! spending what the split bought for operations that are one command invocation rather than
//! a loop. So each verb is a future run to completion through [`crate::runtime::block_on`].
//! Nothing above this file knows there is a runtime, and — since #930 — neither does this
//! file: it once owned one in a field, and an owned runtime's `block_on` panics on a thread
//! that is already inside another, which the HTTP server's is.
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
//!
//! # Every verb retries a transport failure
//!
//! S3 resets long-lived connections as a matter of course, and a corpus large enough to be
//! worth a vault is large enough that this happens during a push rather than between them.
//! All three verbs are idempotent — a HEAD, a GET into a temporary file, a PUT of bytes named
//! by their own digest — so an attempt that failed on the wire can simply be made again.
//! [`super::retry`] holds the schedule and the reason it is per request.
//!
//! Each attempt re-signs and builds a fresh client, which is not incidental: the connection
//! pool is what held the reset socket, and a retry that reused it would reuse the fault.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::cas::ContentHash;
use super::config::VaultConfig;
use super::retry::{self, Attempt, Failed, Fault};
use super::sigv4::{self, Credentials, Signable, EMPTY_PAYLOAD_SHA256};
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
        Ok(Self {
            location,
            region,
            endpoint,
            path_style,
            creds,
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
            service: sigv4::S3_SERVICE,
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
async fn check(resp: reqwest::Response, what: &str) -> Attempt<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let fault = status_fault(status.as_u16());
    let body = resp.text().await.unwrap_or_default();
    let body = body.trim();
    let shown: String = body.chars().take(600).collect();
    Err(Failed::of(
        fault,
        anyhow::anyhow!(
            "{what} failed: HTTP {status}{}",
            if shown.is_empty() {
                String::new()
            } else {
                format!("\n  {shown}")
            }
        ),
    ))
}

/// Which HTTP statuses are the store being busy rather than the request being wrong.
///
/// 429 and 503 are S3's `SlowDown`, 500 its `InternalError`, and 408 a request it stopped
/// waiting for; 502 and 504 are whatever sits in front of it. A 403 or a 404 is an answer,
/// and asking again produces the same one.
fn status_fault(status: u16) -> Fault {
    match status {
        408 | 429 | 500 | 502 | 503 | 504 => Fault::Transient,
        _ => Fault::Permanent,
    }
}

/// Which `reqwest` failures are the connection and which are the answer.
///
/// Taken as the three predicates rather than the error itself because a `reqwest::Error`
/// cannot be constructed outside its crate: as booleans the decision is testable here, and
/// the reading of it at the call site is one line.
///
/// The default is transient, which is the opposite of [`Failed`]'s. It is the right way round
/// for this question: `send` fails when a request does not complete, and a request that does
/// not complete is overwhelmingly the wire. The three exceptions are the cases where it is
/// not — a client that would not build, a status the caller asked to be an error, a body that
/// would not decode — and none of those is repairable by sending it again.
fn send_fault(is_builder: bool, is_status: bool, is_decode: bool) -> Fault {
    match is_builder || is_status || is_decode {
        true => Fault::Permanent,
        false => Fault::Transient,
    }
}

/// Send a signed request, classifying a failure to complete it.
async fn send(r: reqwest::RequestBuilder, what: &str) -> Attempt<reqwest::Response> {
    r.send().await.map_err(|e| {
        let fault = send_fault(e.is_builder(), e.is_status(), e.is_decode());
        Failed::of(fault, anyhow::Error::new(e).context(what.to_string()))
    })
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
        retry::Policy::default().run(|| -> Attempt<bool> {
            let req = self.sign("HEAD", hash, EMPTY_PAYLOAD_SHA256)?;
            let client = self.client()?;
            crate::runtime::block_on(async {
                let mut r = client.head(&req.url);
                for (k, v) in &req.headers {
                    r = r.header(k, v);
                }
                let what = format!("HEAD {}", req.url);
                let resp = send(r, &what).await?;
                if resp.status() == reqwest::StatusCode::NOT_FOUND {
                    return Ok(false);
                }
                check(resp, &what).await?;
                Ok(true)
            })
        })
    }

    fn get(&self, hash: &ContentHash, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let tmp = temp_beside(dest, hash);
        // The partial file is cleaned up per attempt rather than per call, so the invariant
        // the module header states — a reader never finds a partial artifact — holds between
        // two attempts as well as after the last one.
        retry::Policy::default().run(|| -> Attempt<()> {
            let req = self.sign("GET", hash, EMPTY_PAYLOAD_SHA256)?;
            let client = self.client()?;
            let attempt =
                crate::runtime::block_on(async {
                    let mut r = client.get(&req.url);
                    for (k, v) in &req.headers {
                        r = r.header(k, v);
                    }
                    let what = format!("GET {}", req.url);
                    let resp = send(r, &what).await?;
                    let mut resp = check(resp, &what).await?;
                    // Streamed rather than `bytes()`: the whole point of a vault is that it holds
                    // things too big to want in memory.
                    let mut file = std::fs::File::create(&tmp)
                        .with_context(|| format!("creating {}", tmp.display()))?;
                    // A body that stops arriving is the same fault as a request that never left.
                    while let Some(chunk) = resp.chunk().await.map_err(|e| {
                        Failed::transient(anyhow::Error::new(e).context(what.clone()))
                    })? {
                        use std::io::Write;
                        file.write_all(&chunk)
                            .with_context(|| format!("writing {}", tmp.display()))?;
                    }
                    file.sync_all().ok();
                    Ok(())
                });
            if attempt.is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            attempt
        })?;
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
        retry::Policy::default().run(|| -> Attempt<()> {
            // The payload hash is the artifact's own name, which is the property that makes a
            // streamed body signable at all — see the module header. It is also what makes a
            // retry safe: the same digest is the same key holding the same bytes.
            let req = self.sign("PUT", hash, hash.as_str())?;
            let client = self.client()?;
            let src = src.to_path_buf();
            crate::runtime::block_on(async move {
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
                let what = format!("PUT {}", req.url);
                let resp = send(r, &what).await?;
                check(resp, &what).await?;
                Ok(())
            })
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

    #[test]
    fn a_busy_store_is_asked_again_and_a_refusal_is_not() {
        for busy in [408, 429, 500, 502, 503, 504] {
            assert_eq!(status_fault(busy), Fault::Transient, "HTTP {busy}");
        }
        // 403 is the signing failure this store is most likely to hit, and the one where a
        // retry is worst: three identical rejections before the same message.
        for answered in [301, 400, 403, 404, 405, 409, 412, 501] {
            assert_eq!(status_fault(answered), Fault::Permanent, "HTTP {answered}");
        }
    }

    #[test]
    fn a_request_that_did_not_complete_is_the_wire_unless_it_is_not() {
        assert_eq!(send_fault(false, false, false), Fault::Transient);
        assert_eq!(send_fault(true, false, false), Fault::Permanent);
        assert_eq!(send_fault(false, true, false), Fault::Permanent);
        assert_eq!(send_fault(false, false, true), Fault::Permanent);
    }
}
