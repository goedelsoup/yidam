//! Credentials a pod is given by its platform rather than handed: web identity (#1234).
//!
//! On EKS, IRSA and Pod Identity project a signed token into the pod and name it, with the
//! role it may assume, in the environment: `AWS_WEB_IDENTITY_TOKEN_FILE` and `AWS_ROLE_ARN`.
//! The token is exchanged with STS's `AssumeRoleWithWebIdentity` for temporary credentials.
//! Without this, a cluster operator mints a long-lived IAM user key and stores it in a secret.
//!
//! # Still "the environment, and nothing else"
//!
//! RFC-0023's amendment of 2026-09-30 settles it: a file *named by* the environment counts as
//! the environment. The platform projects it, nothing commits it, and the variable holds a path,
//! not a key. Which vault may use which role follows `creds.rs`'s rule unchanged.
//!
//! # Unsigned, and minimal
//!
//! `AssumeRoleWithWebIdentity` is the one STS call that takes no signature: the token is the
//! authentication. So this is one form POST through the same `reqwest` the stores use, and a
//! response read for four elements. There is no STS client and no XML parser, because four
//! leaf elements whose values never contain `<` do not need one.
//!
//! # Refreshed, not fetched once
//!
//! The credentials last an hour by default. A `vault push` of a large index, or `serve`, can
//! outlive that, so [`super::creds::Provider`] holds the lease and exchanges again when fewer
//! than [`REFRESH_BEFORE`] seconds remain. The token file is read on every exchange, because
//! the kubelet rotates it.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};

use super::retry::{self, Attempt, Failed, Fault};
use super::sigv4::Credentials;

/// How long before expiry a lease is exchanged again. Long enough that a request signed just
/// before the line still arrives in time, and the margin the AWS SDKs use.
pub const REFRESH_BEFORE: u64 = 300;

/// A role to assume, and the token that proves the pod may.
pub struct WebIdentity {
    pub role_arn: String,
    pub token_file: PathBuf,
    pub session_name: String,
    /// Scheme and authority, e.g. `https://sts.us-east-1.amazonaws.com`.
    pub sts_endpoint: String,
    /// The variable the role came from, for a report: `AWS_ROLE_ARN` or a vault's own.
    pub role_var: String,
}

/// Temporary credentials and when they stop working, in Unix seconds.
pub struct Lease {
    pub creds: Credentials,
    pub expires: u64,
}

impl Lease {
    /// Whether this lease is still worth signing with at `now`.
    pub fn fresh(&self, now: u64) -> bool {
        now.saturating_add(REFRESH_BEFORE) < self.expires
    }
}

/// Where STS is: `AWS_ENDPOINT_URL_STS`, then the regional endpoint for `AWS_REGION` or
/// `AWS_DEFAULT_REGION`, then the global one.
///
/// Read for every scope, a second vault's included. None of these says *who* a pod is, only
/// where to ask, so reading them crosses no boundary. EKS's webhook sets `AWS_REGION`, and a
/// regional endpoint is what an STS interface endpoint, and `[cluster.egress] sts`, serves.
pub fn sts_endpoint(lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let get = |k: &str| lookup(k).filter(|v| !v.trim().is_empty());
    if let Some(e) = get("AWS_ENDPOINT_URL_STS") {
        return e.trim().trim_end_matches('/').to_string();
    }
    match get("AWS_REGION").or_else(|| get("AWS_DEFAULT_REGION")) {
        Some(r) => format!("https://sts.{}.amazonaws.com", r.trim()),
        None => "https://sts.amazonaws.com".to_string(),
    }
}

/// The form body of one `AssumeRoleWithWebIdentity` call. `DurationSeconds` is left to the
/// role's default, an hour, which the role's owner can raise without this changing.
pub fn request_body(role_arn: &str, session_name: &str, token: &str) -> String {
    let enc = super::sigv4::uri_encode;
    format!(
        "Action=AssumeRoleWithWebIdentity&Version=2011-06-15&RoleArn={}&RoleSessionName={}\
         &WebIdentityToken={}",
        enc(role_arn),
        enc(session_name),
        enc(token)
    )
}

/// Exchange the token for a lease. Reads the token file afresh, and repeats a transient failure.
pub fn exchange(w: &WebIdentity) -> Result<Lease> {
    let token = std::fs::read_to_string(&w.token_file)
        .with_context(|| {
            format!(
                "reading the web identity token at {} (named by the environment beside {})",
                w.token_file.display(),
                w.role_var
            )
        })?
        .trim()
        .to_string();
    let body = request_body(&w.role_arn, &w.session_name, &token);
    let url = format!("{}/", w.sts_endpoint);
    retry::Policy::default().run(|| -> Attempt<Lease> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("yidam-vault/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building the HTTP client")?;
        let body = body.clone();
        let (status, text) = crate::runtime::block_on(async {
            let resp = client
                .post(&url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await
                .map_err(|e| {
                    Failed::transient(
                        anyhow::Error::new(e)
                            .context(format!("AssumeRoleWithWebIdentity at {url}")),
                    )
                })?;
            let status = resp.status().as_u16();
            let text = resp.text().await.map_err(|e| {
                Failed::transient(anyhow::Error::new(e).context(format!("reading {url}")))
            })?;
            Ok::<_, Failed>((status, text))
        })?;
        parse_response(status, &text).map_err(|f| {
            Failed::of(
                f.fault,
                f.error
                    .context(format!("assuming {} ({})", w.role_arn, w.role_var)),
            )
        })
    })
}

/// Read a response: a lease, or STS's own error code and message with whether to ask again.
///
/// `IDPCommunicationError` is the one 400 STS documents as worth retrying: the identity
/// provider did not answer it. An `InvalidIdentityToken` or an `AccessDenied` is an answer.
pub fn parse_response(status: u16, body: &str) -> Attempt<Lease> {
    if !(200..300).contains(&status) {
        let code = element(body, "Code").unwrap_or_default();
        let message = element(body, "Message").unwrap_or_default();
        let fault = match (status, code.as_str()) {
            (_, "IDPCommunicationError") | (408 | 429 | 500 | 502 | 503 | 504, _) => {
                Fault::Transient
            }
            _ => Fault::Permanent,
        };
        let detail = match (code.is_empty(), message.is_empty()) {
            (true, _) => body.trim().chars().take(600).collect::<String>(),
            (false, true) => code,
            (false, false) => format!("{code}: {message}"),
        };
        return Err(Failed::of(
            fault,
            anyhow!("STS refused the web identity: HTTP {status}\n  {detail}"),
        ));
    }
    let need = |name: &str| {
        element(body, name).ok_or_else(|| {
            Failed::of(
                Fault::Permanent,
                anyhow!("STS answered without <{name}>, so there are no credentials to sign with"),
            )
        })
    };
    let expiration = need("Expiration")?;
    let expires = unix_from_iso8601(&expiration).ok_or_else(|| {
        Failed::of(
            Fault::Permanent,
            anyhow!("STS gave an <Expiration> of {expiration:?}, which is not a UTC timestamp"),
        )
    })?;
    Ok(Lease {
        creds: Credentials {
            access_key_id: need("AccessKeyId")?,
            secret_access_key: need("SecretAccessKey")?,
            session_token: Some(need("SessionToken")?),
        },
        expires,
    })
}

/// The text of the first `<name>…</name>` in `xml`, with the five predefined entities undone.
fn element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&format!("</{name}>"))?;
    Some(
        xml[start..end]
            .trim()
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

/// `2026-09-30T12:34:56Z`, with or without fractional seconds, as Unix seconds.
fn unix_from_iso8601(s: &str) -> Option<u64> {
    let s = s.trim().strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let days = crate::dates::days_from_civil_str(date)?;
    let time = time.split('.').next()?;
    let mut hms = time.split(':').map(|p| p.parse::<u64>().ok());
    let (h, m, sec) = (hms.next()??, hms.next()??, hms.next()??);
    if hms.next().is_some() || h > 23 || m > 59 || sec > 60 {
        return None;
    }
    u64::try_from(days)
        .ok()
        .map(|d| d * 86400 + h * 3600 + m * 60 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| m.get(k).cloned()
    }

    const OK: &str = r#"<AssumeRoleWithWebIdentityResponse xmlns="https://sts.amazonaws.com/doc/2011-06-15/">
  <AssumeRoleWithWebIdentityResult>
    <Credentials>
      <AccessKeyId>ASIAEXAMPLE</AccessKeyId>
      <SecretAccessKey>wJalr/K7MDENG+bPxRfi&amp;KEY</SecretAccessKey>
      <SessionToken>FwoGZXIvYXdzEJr//////////wEaDM</SessionToken>
      <Expiration>2026-09-30T13:00:00Z</Expiration>
    </Credentials>
  </AssumeRoleWithWebIdentityResult>
</AssumeRoleWithWebIdentityResponse>"#;

    #[test]
    fn a_success_reads_four_elements_into_a_lease() {
        let lease = parse_response(200, OK).map_err(|f| f.error).unwrap();
        assert_eq!(lease.creds.access_key_id, "ASIAEXAMPLE");
        assert_eq!(lease.creds.secret_access_key, "wJalr/K7MDENG+bPxRfi&KEY");
        assert_eq!(
            lease.creds.session_token.as_deref(),
            Some("FwoGZXIvYXdzEJr//////////wEaDM")
        );
        assert_eq!(
            lease.expires,
            unix_from_iso8601("2026-09-30T13:00:00Z").unwrap()
        );
    }

    #[test]
    fn an_expiration_is_read_as_utc_with_or_without_fractions() {
        // 2015-08-30T12:36:00Z is the instant sigv4's tests sign at.
        assert_eq!(unix_from_iso8601("2015-08-30T12:36:00Z"), Some(1440938160));
        assert_eq!(
            unix_from_iso8601("2015-08-30T12:36:00.123Z"),
            Some(1440938160)
        );
        assert_eq!(unix_from_iso8601("2015-08-30T12:36:00"), None, "no zone");
        assert_eq!(unix_from_iso8601("2015-08-30T25:00:00Z"), None);
    }

    /// An `InvalidIdentityToken` is an answer, and asking again gets the same one. STS's own
    /// code and message are what an operator can act on, so they are what is shown.
    #[test]
    fn a_refusal_names_stss_code_and_is_not_retried() {
        let body = "<ErrorResponse><Error><Type>Sender</Type><Code>InvalidIdentityToken</Code>\
                    <Message>No OpenIDConnect provider found</Message></Error></ErrorResponse>";
        let f = parse_response(400, body).map(|_| ()).unwrap_err();
        assert_eq!(f.fault, Fault::Permanent);
        let msg = f.error.to_string();
        assert!(
            msg.contains("InvalidIdentityToken: No OpenIDConnect"),
            "{msg}"
        );
    }

    #[test]
    fn an_unreachable_identity_provider_is_asked_again() {
        let body =
            "<ErrorResponse><Error><Code>IDPCommunicationError</Code></Error></ErrorResponse>";
        assert_eq!(
            parse_response(400, body).map(|_| ()).unwrap_err().fault,
            Fault::Transient
        );
        assert_eq!(
            parse_response(503, "").map(|_| ()).unwrap_err().fault,
            Fault::Transient
        );
    }

    #[test]
    fn a_success_missing_an_element_is_refused_by_name() {
        let body = OK.replace(
            "<SessionToken>FwoGZXIvYXdzEJr//////////wEaDM</SessionToken>",
            "",
        );
        let msg = parse_response(200, &body)
            .map(|_| ())
            .unwrap_err()
            .error
            .to_string();
        assert!(msg.contains("<SessionToken>"), "{msg}");
    }

    /// The token is a JWT, and the role ARN has a colon and a slash in it. Both must arrive as
    /// the bytes they are, so everything outside the unreserved set is escaped.
    #[test]
    fn the_request_body_escapes_what_a_form_would_misread() {
        let b = request_body("arn:aws:iam::1:role/r", "yidam-1", "a.b+c/d=");
        assert!(b.starts_with("Action=AssumeRoleWithWebIdentity&Version=2011-06-15&"));
        assert!(
            b.contains("RoleArn=arn%3Aaws%3Aiam%3A%3A1%3Arole%2Fr"),
            "{b}"
        );
        assert!(b.contains("WebIdentityToken=a.b%2Bc%2Fd%3D"), "{b}");
    }

    #[test]
    fn sts_is_found_by_override_then_region_then_globally() {
        assert_eq!(
            sts_endpoint(&env(&[
                ("AWS_ENDPOINT_URL_STS", "http://127.0.0.1:9/"),
                ("AWS_REGION", "eu-west-1")
            ])),
            "http://127.0.0.1:9"
        );
        assert_eq!(
            sts_endpoint(&env(&[("AWS_DEFAULT_REGION", "us-east-2")])),
            "https://sts.us-east-2.amazonaws.com"
        );
        assert_eq!(
            sts_endpoint(&env(&[
                ("AWS_REGION", "eu-west-1"),
                ("AWS_DEFAULT_REGION", "x")
            ])),
            "https://sts.eu-west-1.amazonaws.com"
        );
        assert_eq!(sts_endpoint(&env(&[])), "https://sts.amazonaws.com");
    }

    #[test]
    fn a_lease_is_stale_inside_the_refresh_margin() {
        let lease = Lease {
            creds: Credentials {
                access_key_id: "A".into(),
                secret_access_key: "S".into(),
                session_token: None,
            },
            expires: 10_000,
        };
        assert!(lease.fresh(10_000 - REFRESH_BEFORE - 1));
        assert!(!lease.fresh(10_000 - REFRESH_BEFORE));
        assert!(!lease.fresh(20_000));
    }

    /// The whole exchange against a listener that answers like STS: the form arrives with the
    /// token read from the file, and the lease comes back. The token is read at exchange time,
    /// so a rotated file is what the next exchange sends.
    #[test]
    fn an_exchange_posts_the_token_file_and_reads_the_lease() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            // Read until the declared body has arrived.
            loop {
                let n = sock.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let len = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if body.len() >= len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/xml\r\ncontent-length: {}\r\n\
                 connection: close\r\n\r\n{OK}",
                OK.len()
            );
            sock.write_all(resp.as_bytes()).unwrap();
            String::from_utf8(got).unwrap()
        });

        let dir = tempfile::tempdir().unwrap();
        let token = dir.path().join("token");
        std::fs::write(&token, "eyJ.projected.token\n").unwrap();
        let lease = exchange(&WebIdentity {
            role_arn: "arn:aws:iam::123456789012:role/yidam".into(),
            token_file: token,
            session_name: "yidam-test".into(),
            sts_endpoint: format!("http://127.0.0.1:{port}"),
            role_var: "AWS_ROLE_ARN".into(),
        })
        .unwrap();
        assert_eq!(lease.creds.access_key_id, "ASIAEXAMPLE");

        let request = server.join().unwrap();
        assert!(request.starts_with("POST / "), "{request}");
        assert!(
            request.contains("WebIdentityToken=eyJ.projected.token"),
            "the trailing newline is not part of the token: {request}"
        );
        assert!(
            !request.to_ascii_lowercase().contains("authorization:"),
            "unsigned"
        );
    }

    #[test]
    fn a_missing_token_file_names_the_file_and_the_variable() {
        let msg = exchange(&WebIdentity {
            role_arn: "arn:aws:iam::1:role/r".into(),
            token_file: "/nonexistent/yidam/token".into(),
            session_name: "s".into(),
            sts_endpoint: "http://127.0.0.1:9".into(),
            role_var: "YIDAM_VAULT_SOURCES_ROLE_ARN".into(),
        })
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(msg.contains("/nonexistent/yidam/token"), "{msg}");
        assert!(msg.contains("YIDAM_VAULT_SOURCES_ROLE_ARN"), "{msg}");
    }
}
