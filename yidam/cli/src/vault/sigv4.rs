//! AWS Signature Version 4, for the three verbs a content-addressed store needs.
//!
//! # Why this is hand-written
//!
//! #412 measured the alternatives against this repository's real dependency graph. Every
//! library brings a second copy of the HTTP and TLS stack into a binary that already has one,
//! and the smallest of them additionally wants `aws-lc-sys`, a C crypto library whose build
//! needs CMake that `release.yml` does not install. Signing here costs one pure-Rust crate.
//!
//! # Why that is a smaller undertaking than it sounds
//!
//! **Nothing here ever emits `STREAMING-AWS4-HMAC-SHA256-PAYLOAD`.** Chunked signing — a
//! second signing surface and a state machine, and where SigV4 implementations usually go
//! wrong — exists for bodies whose hash is not known when the request is built. A
//! content-addressed store always knows: naming the artifact *is* computing its digest, so
//! `x-amz-content-sha256` is already in hand before a single byte is sent, and the body can
//! stream from disk under a signature computed up front.
//!
//! The rest is GET, PUT and HEAD against keys this crate generates — `<prefix>/sha256/<aa>/<64
//! hex>` — whose characters are all unreserved, so canonical-URI encoding is the identity on
//! every key a vault will ever ask for. The encoder below is still written correctly, because
//! a prefix comes from a human and may contain anything.
//!
//! # Everything here is pure
//!
//! No clock, no environment, no I/O. The timestamp arrives as an argument, which is what lets
//! the AWS test-suite vectors be asserted exactly and what lets `vault push --dry-run` print
//! the canonical request a real send would have signed.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub const ALGORITHM: &str = "AWS4-HMAC-SHA256";
/// The S3 service name in the credential scope. Every S3-compatible store uses it.
pub const SERVICE: &str = "s3";
/// The digest of an empty body — what GET, HEAD and DELETE sign as their payload.
pub const EMPTY_PAYLOAD_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// What a request is signed with.
///
/// `Debug` is deliberately **not** derived. A secret that can be printed is a secret that
/// ends up in a log, and this type exists on the path of a command whose failures are
/// reported to a terminal.
#[derive(Clone)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    /// Present for temporary credentials, which additionally sign `x-amz-security-token`.
    pub session_token: Option<String>,
}

/// One request, reduced to what signing needs.
pub struct Signable<'a> {
    pub method: &'a str,
    /// Host header, including a port when the endpoint carries one — the signature covers
    /// the header verbatim, so `host:localhost:9000` must be signed as it is sent.
    pub host: &'a str,
    /// Absolute path, not yet canonically encoded.
    pub path: &'a str,
    /// Canonical query string, or empty. Sorted by key; empty for every request this makes.
    pub query: &'a str,
    /// Hex digest of the body. The artifact's own for a PUT; [`EMPTY_PAYLOAD_SHA256`]
    /// otherwise.
    pub payload_sha256: &'a str,
    /// `YYYYMMDDTHHMMSSZ`, UTC.
    pub timestamp: &'a str,
    pub region: &'a str,
}

impl Signable<'_> {
    /// `YYYYMMDD` — the date half of the credential scope.
    fn date(&self) -> &str {
        &self.timestamp[..8]
    }

    fn scope(&self) -> String {
        format!("{}/{}/{SERVICE}/aws4_request", self.date(), self.region)
    }

    /// The headers that are signed, in canonical order.
    ///
    /// Exactly the three (or four) this signer sets. Signing more than is sent, or fewer, is
    /// the same failure in either direction — the server recomputes from what arrived.
    fn headers(&self, creds: &Credentials) -> Vec<(String, String)> {
        let mut h = vec![
            ("host".to_string(), self.host.to_string()),
            (
                "x-amz-content-sha256".to_string(),
                self.payload_sha256.to_string(),
            ),
            ("x-amz-date".to_string(), self.timestamp.to_string()),
        ];
        if let Some(t) = &creds.session_token {
            h.push(("x-amz-security-token".to_string(), t.clone()));
        }
        h.sort_by(|a, b| a.0.cmp(&b.0));
        h
    }

    /// The canonical request, verbatim — the string `--dry-run` prints.
    pub fn canonical_request(&self, creds: &Credentials) -> String {
        let headers = self.headers(creds);
        let canonical_headers: String = headers
            .iter()
            .map(|(k, v)| format!("{k}:{}\n", v.trim()))
            .collect();
        let signed_headers = headers
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "{}\n{}\n{}\n{canonical_headers}\n{signed_headers}\n{}",
            self.method,
            canonical_uri(self.path),
            self.query,
            self.payload_sha256
        )
    }

    fn signed_headers(&self, creds: &Credentials) -> String {
        self.headers(creds)
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(";")
    }

    /// The string that is actually signed.
    pub fn string_to_sign(&self, creds: &Credentials) -> String {
        format!(
            "{ALGORITHM}\n{}\n{}\n{}",
            self.timestamp,
            self.scope(),
            hex::encode(Sha256::digest(self.canonical_request(creds).as_bytes()))
        )
    }

    /// The signature, as lowercase hex.
    pub fn signature(&self, creds: &Credentials) -> String {
        let key = signing_key(&creds.secret_access_key, self.date(), self.region);
        hex::encode(hmac(&key, self.string_to_sign(creds).as_bytes()))
    }

    /// Every header the request must carry, including `Authorization`.
    ///
    /// Returned together rather than as an `Authorization` alone, because the signature
    /// covers the others: a caller that sets the header and forgets `x-amz-date` has built a
    /// request the server will reject and will have no idea why.
    pub fn headers_to_send(&self, creds: &Credentials) -> Vec<(String, String)> {
        let mut out = self.headers(creds);
        out.push((
            "authorization".to_string(),
            format!(
                "{ALGORITHM} Credential={}/{}, SignedHeaders={}, Signature={}",
                creds.access_key_id,
                self.scope(),
                self.signed_headers(creds),
                self.signature(creds)
            ),
        ));
        out
    }
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).expect("hmac takes a key of any length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

/// The four-step derived key. Each step narrows the scope, so a leaked signing key is good
/// for one day, one region and one service.
fn signing_key(secret: &str, date: &str, region: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, SERVICE.as_bytes());
    hmac(&k_service, b"aws4_request")
}

/// The canonical URI: each path segment percent-encoded, `/` preserved.
///
/// **Not double-encoded.** S3 is the documented exception to SigV4's normal rule, and getting
/// this wrong produces a signature that is valid-looking and rejected. Every key this crate
/// generates is unreserved characters and `/`, so this is the identity on them; a
/// human-written prefix is why it is written properly anyway.
fn canonical_uri(path: &str) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    let mut out = String::with_capacity(path.len());
    for segment in path.split('/') {
        if !out.is_empty() || path.starts_with('/') {
            // `split` on a leading `/` yields an empty first segment, which reproduces it.
        }
        out.push_str(&uri_encode(segment));
        out.push('/');
    }
    out.pop();
    out
}

/// Percent-encode everything outside SigV4's unreserved set.
///
/// Uppercase hex, which the specification requires: `%2f` and `%2F` are the same byte and
/// only one of them signs.
fn uri_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The credentials AWS's SigV4 documentation uses in its worked examples.
    ///
    /// **On what these tests are and are not.** The vectors below pin the *derivation steps* —
    /// reorder `date, region, service` and they go red, which is where a transposition hides,
    /// since any order produces 32 plausible bytes. They were computed by this implementation
    /// and cross-checked against an implementation written independently from the
    /// specification in another language; two implementations agreeing is evidence about the
    /// steps and not proof about the scheme.
    ///
    /// **The authority that the whole thing is correct is a real server accepting it.** A
    /// round-trip against MinIO is what establishes that, and it cannot run in hermetic CI —
    /// see `tests/vault_s3.rs` for the harness and the note on running it.
    ///
    /// (An earlier draft of this file asserted a constant recalled as being from the published
    /// `get-vanilla` case. It was wrong, and it failed on the first run — which is the argument
    /// for never writing a cryptographic vector from memory.)
    const KEY: &str = "AKIDEXAMPLE";
    const SECRET: &str = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";

    fn creds() -> Credentials {
        Credentials {
            access_key_id: KEY.to_string(),
            secret_access_key: SECRET.to_string(),
            session_token: None,
        }
    }

    /// The derived signing key, pinned so a reordered step goes red.
    ///
    /// Both scopes, because the two differ only in one HMAC input and a bug that swapped
    /// region for service would still produce a stable answer for either alone.
    #[test]
    fn the_derived_signing_key_is_stable_and_scope_dependent() {
        assert_eq!(
            hex::encode(signing_key_for(SECRET, "20150830", "us-east-1", "service")),
            "938127b5336810ddb6a5d6af445fcac9e371f9ed418ed386b022aed82901be75"
        );
        assert_eq!(
            hex::encode(signing_key_for(SECRET, "20150830", "us-east-1", "s3")),
            "32f78051dcde24c552811d654f4a769112bb834b03975cdd6b1fd7d16248c269"
        );
        // Narrowing any one of the four steps changes the key. Without this, a derivation
        // that ignored `region` entirely would pass the two assertions above.
        assert_ne!(
            signing_key_for(SECRET, "20150830", "us-east-1", "s3"),
            signing_key_for(SECRET, "20150830", "eu-west-2", "s3")
        );
        assert_ne!(
            signing_key_for(SECRET, "20150830", "us-east-1", "s3"),
            signing_key_for(SECRET, "20150831", "us-east-1", "s3")
        );
    }

    /// The same four steps as [`signing_key`], with the service left open so the published
    /// vectors — which sign a service literally called `service` — can be asserted.
    fn signing_key_for(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
        let k_date = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
        let k_region = hmac(&k_date, region.as_bytes());
        let k_service = hmac(&k_region, service.as_bytes());
        hmac(&k_service, b"aws4_request")
    }

    /// `signing_key` must be `signing_key_for` with the service pinned to `s3` — otherwise
    /// the test above is exercising a function production does not use.
    #[test]
    fn the_production_key_is_the_general_one_with_s3_pinned() {
        assert_eq!(
            signing_key(SECRET, "20150830", "us-east-1"),
            signing_key_for(SECRET, "20150830", "us-east-1", "s3")
        );
    }

    #[test]
    fn a_canonical_request_has_the_shape_the_specification_describes() {
        let s = Signable {
            method: "GET",
            host: "example.amazonaws.com",
            path: "/",
            query: "",
            payload_sha256: EMPTY_PAYLOAD_SHA256,
            timestamp: "20150830T123600Z",
            region: "us-east-1",
        };
        let c = s.canonical_request(&creds());
        let lines: Vec<&str> = c.split('\n').collect();
        assert_eq!(lines[0], "GET");
        assert_eq!(lines[1], "/");
        assert_eq!(lines[2], "", "no query");
        // Headers, sorted, one per line, then a blank line, then the signed-header list.
        assert_eq!(lines[3], "host:example.amazonaws.com");
        assert_eq!(
            lines[4],
            format!("x-amz-content-sha256:{EMPTY_PAYLOAD_SHA256}")
        );
        assert_eq!(lines[5], "x-amz-date:20150830T123600Z");
        assert_eq!(lines[6], "");
        assert_eq!(lines[7], "host;x-amz-content-sha256;x-amz-date");
        assert_eq!(lines[8], EMPTY_PAYLOAD_SHA256);
    }

    /// The string to sign is four lines and the last is a digest of the canonical request —
    /// so a change anywhere in the request changes the signature, which is the property the
    /// whole scheme rests on.
    #[test]
    fn the_string_to_sign_binds_the_canonical_request() {
        let s = Signable {
            method: "GET",
            host: "example.amazonaws.com",
            path: "/",
            query: "",
            payload_sha256: EMPTY_PAYLOAD_SHA256,
            timestamp: "20150830T123600Z",
            region: "us-east-1",
        };
        let sts = s.string_to_sign(&creds());
        let lines: Vec<&str> = sts.split('\n').collect();
        assert_eq!(lines[0], ALGORITHM);
        assert_eq!(lines[1], "20150830T123600Z");
        assert_eq!(lines[2], "20150830/us-east-1/s3/aws4_request");
        assert_eq!(
            lines[3],
            hex::encode(Sha256::digest(s.canonical_request(&creds()).as_bytes()))
        );
    }

    /// A temporary credential signs its token, and the header list grows in sorted position.
    #[test]
    fn a_session_token_is_signed_and_sorted_into_place() {
        let mut c = creds();
        c.session_token = Some("TOKEN".into());
        let s = Signable {
            method: "GET",
            host: "h",
            path: "/",
            query: "",
            payload_sha256: EMPTY_PAYLOAD_SHA256,
            timestamp: "20150830T123600Z",
            region: "us-east-1",
        };
        let req = s.canonical_request(&c);
        assert!(req.contains("x-amz-security-token:TOKEN"));
        assert!(req.contains("host;x-amz-content-sha256;x-amz-date;x-amz-security-token"));
    }

    /// Every signed header must actually be sent. Signing a header the request omits and
    /// omitting a header the signature covers are the same failure from opposite sides, and
    /// the server sees both as a bad signature with no further detail.
    #[test]
    fn every_signed_header_is_among_the_headers_to_send() {
        let mut c = creds();
        c.session_token = Some("TOKEN".into());
        let s = Signable {
            method: "PUT",
            host: "h",
            path: "/a/b",
            query: "",
            payload_sha256: EMPTY_PAYLOAD_SHA256,
            timestamp: "20150830T123600Z",
            region: "us-east-1",
        };
        let sent: Vec<String> = s.headers_to_send(&c).into_iter().map(|(k, _)| k).collect();
        for signed in s.signed_headers(&c).split(';') {
            assert!(sent.iter().any(|k| k == signed), "{signed} is not sent");
        }
        assert!(sent.iter().any(|k| k == "authorization"));
    }

    /// The authorization header names the same scope the string to sign does. Two spellings
    /// of the scope is a classic SigV4 bug and the server reports it as a bad signature.
    #[test]
    fn the_authorization_header_and_the_string_to_sign_agree_on_scope() {
        let s = Signable {
            method: "GET",
            host: "h",
            path: "/",
            query: "",
            payload_sha256: EMPTY_PAYLOAD_SHA256,
            timestamp: "20150830T123600Z",
            region: "eu-west-2",
        };
        let auth = s
            .headers_to_send(&creds())
            .into_iter()
            .find(|(k, _)| k == "authorization")
            .map(|(_, v)| v)
            .unwrap();
        assert!(auth.contains("Credential=AKIDEXAMPLE/20150830/eu-west-2/s3/aws4_request"));
        assert!(s
            .string_to_sign(&creds())
            .contains("20150830/eu-west-2/s3/aws4_request"));
        assert!(auth.contains(&s.signature(&creds())));
    }

    /// Every key a vault generates is unreserved characters and `/`, so encoding must not
    /// touch them — a `%2F` where a `/` belongs signs a path the server never saw.
    #[test]
    fn a_vault_key_survives_canonical_encoding_unchanged() {
        let key =
            "/corpus/sha256/ba/ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(canonical_uri(key), key);
    }

    /// A prefix comes from a person and may contain anything, so the encoder is written
    /// properly even though no generated key needs it. Uppercase hex, per the specification.
    #[test]
    fn a_prefix_with_awkward_characters_is_encoded_and_slashes_are_kept() {
        assert_eq!(canonical_uri("/my archive/x"), "/my%20archive/x");
        assert_eq!(canonical_uri("/a+b/c"), "/a%2Bb/c");
        assert_eq!(canonical_uri("/~-._/x"), "/~-._/x");
        assert_eq!(canonical_uri(""), "/");
    }
}
