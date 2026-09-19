//! The one module here that performs I/O.
//!
//! # It owns its runtime
//!
//! Everything above this file is synchronous, for the reason `vault::store` gives about its
//! own trait: an async signature would spread `tokio` into the ungated half of this module and
//! spend what the split bought. So the runtime is built here and blocked on here, and nothing
//! outside knows there is one.
//!
//! # Deliberately thin
//!
//! Sign, send, read the status, parse the body. There is no paging here, no batching, no retry
//! and no diff — those are in [`super::ops`], because there is no S3 Vectors emulator and
//! anything in this file cannot be exercised in CI. What is left is short enough to be read
//! and checked by eye, which is the only review this code gets until the live smoke test runs.

use anyhow::{Context, Result};
use serde_json::Value;

use super::request::Request;
use super::response::{self, Failure};
use super::{Api, RemoteIndex};
use crate::vault::sigv4::{Credentials, Signable, S3VECTORS_SERVICE};

/// A signed request, and the string that was signed.
///
/// The canonical request is carried for the same reason `vault::s3::SignedRequest` carries it:
/// it is the only artifact of a signing bug a person can actually inspect, and `--dry-run`
/// prints it.
pub struct SignedRequest {
    pub canonical_request: String,
    pub headers: Vec<(String, String)>,
    pub url: String,
    pub body: Vec<u8>,
}

pub struct Client {
    index: RemoteIndex,
    creds: Credentials,
    runtime: tokio::runtime::Runtime,
    http: reqwest::Client,
    /// A clock, injected so the signing path is exercisable at a fixed time.
    now: fn() -> u64,
}

impl Client {
    pub fn new(index: RemoteIndex, creds: Credentials) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("building the runtime for the S3 Vectors transport")?;
        let http = reqwest::Client::builder()
            .user_agent(concat!("yidam-s3vectors/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building the HTTP client")?;
        Ok(Self {
            index,
            creds,
            runtime,
            http,
            now: unix_now,
        })
    }

    /// Everything the request needs, signed. Also what `--dry-run` renders.
    ///
    /// The body is serialized once, here: the digest in `x-amz-content-sha256` covers these
    /// exact bytes, and serializing again to send would be signing one string and sending
    /// another.
    pub fn sign(&self, req: &Request) -> Result<SignedRequest> {
        let body = req.bytes()?;
        let payload_sha256 = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&body));
        let timestamp = crate::dates::amz_datetime((self.now)());
        let signable = Signable {
            method: "POST",
            host: self.index.host(),
            path: req.op.path(),
            query: "",
            payload_sha256: &payload_sha256,
            timestamp: &timestamp,
            region: &self.index.region,
            service: S3VECTORS_SERVICE,
        };
        Ok(SignedRequest {
            canonical_request: signable.canonical_request(&self.creds),
            headers: signable.headers_to_send(&self.creds),
            url: self.index.url(req.op),
            body,
        })
    }
}

impl Api for Client {
    fn call(&self, request: &Request) -> Result<Value, Failure> {
        let signed = self
            .sign(request)
            .map_err(|e| Failure::invalid(e.to_string()))?;
        self.runtime.block_on(async {
            let mut sent = self
                .http
                .post(&signed.url)
                // Not signed, and it does not need to be: the server recomputes the signature
                // from the headers the request declares it signed, and this is not among them.
                .header("content-type", "application/json")
                .body(signed.body);
            for (k, v) in &signed.headers {
                sent = sent.header(k, v);
            }

            let resp = sent.send().await.map_err(|e| {
                Failure::local(format!("{} {}: {e}", request.op.path(), signed.url))
            })?;

            let status = resp.status().as_u16();
            let body = resp
                .bytes()
                .await
                .map_err(|e| Failure::local(format!("reading the response body: {e}")))?;

            if !(200..300).contains(&status) {
                return Err(Failure {
                    fault: response::classify(status),
                    message: format!(
                        "{}: {}",
                        request.op.path().trim_start_matches('/'),
                        response::fault_message(status, &body)
                    ),
                });
            }

            // `PutVectors` answers 200 with an empty body, and `serde_json` will not parse
            // nothing. An empty success is a success.
            if body.is_empty() {
                return Ok(Value::Null);
            }
            serde_json::from_slice(&body).map_err(|e| {
                Failure::invalid(format!(
                    "{} answered {status} with a body that is not JSON: {e}",
                    request.op.path()
                ))
            })
        })
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s3vectors::{request, RemoteIndexConfig, KIND};

    fn client() -> Client {
        let index = RemoteIndex::resolve(&RemoteIndexConfig {
            kind: KIND.to_string(),
            bucket: "yidam-corpora".to_string(),
            index: "yidam-main".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
        })
        .unwrap();
        let mut c = Client::new(
            index,
            Credentials {
                access_key_id: "AKIDEXAMPLE".to_string(),
                secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".to_string(),
                session_token: None,
            },
        )
        .unwrap();
        // A fixed clock, so the signature below is a constant rather than a shape.
        c.now = || 1_440_938_160;
        c
    }

    /// The scope on the wire names `s3vectors`, and the request is signed for the host it is
    /// sent to. Both are what a wrong answer here looks like: a plausible signature the service
    /// rejects with no explanation of which half is wrong.
    #[test]
    fn a_request_is_signed_for_s3vectors_at_the_host_it_is_sent_to() {
        let c = client();
        let signed = c.sign(&request::get_index(&c.index)).unwrap();
        let auth = signed
            .headers
            .iter()
            .find(|(k, _)| k == "authorization")
            .map(|(_, v)| v.as_str())
            .expect("authorization is always sent");
        assert!(auth.contains("/us-east-1/s3vectors/aws4_request"), "{auth}");
        assert!(!auth.contains("/s3/aws4_request"), "{auth}");

        assert_eq!(signed.url, "https://s3vectors.us-east-1.api.aws/GetIndex");
        let host = signed
            .headers
            .iter()
            .find(|(k, _)| k == "host")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(
            signed.url.contains(host),
            "signed for {host}, sent to {}",
            signed.url
        );
    }

    /// The digest covers the bytes that are sent, and the canonical request names it.
    #[test]
    fn the_payload_digest_covers_the_body_that_goes_on_the_wire() {
        let c = client();
        let req = request::get_index(&c.index);
        let signed = c.sign(&req).unwrap();
        let digest = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&signed.body));
        assert!(signed.canonical_request.ends_with(&digest));
        assert!(signed
            .headers
            .iter()
            .any(|(k, v)| k == "x-amz-content-sha256" && v == &digest));
        assert_eq!(signed.body, req.bytes().unwrap());
    }

    #[test]
    fn the_canonical_request_is_a_post_to_the_operations_path() {
        let c = client();
        let signed = c.sign(&request::list_vectors(&c.index, None)).unwrap();
        let mut lines = signed.canonical_request.lines();
        assert_eq!(lines.next(), Some("POST"));
        assert_eq!(lines.next(), Some("/ListVectors"));
        assert_eq!(lines.next(), Some(""), "there is never a query string");
    }
}
