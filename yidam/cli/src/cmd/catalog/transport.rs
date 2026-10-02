//! Bytes from an address — the one part of a fetch that needs a network.
//!
//! # How small this is, on purpose
//!
//! Everything interesting about following a catalog address happens in
//! [`super::location`], which is ungated: deciding what would be fetched, refusing a path
//! that leaves the repository, refusing a template nothing bound. What is left here is a
//! function from a resolved URL to a file on disk.
//!
//! That split is #460's stated mitigation for the failure mode it calls the *feature-gate
//! blind spot* — "anything gated takes gated facts as arguments". The gated half takes a
//! `String` and a `Path` and returns bytes, which is about as little judgement as a function
//! can carry, so the code a pull request does not compile is also the code a pull request has
//! least reason to.
//!
//! # Why it is a feature at all, and why that feature is in `default`
//!
//! `vault-s3` settled both halves of this and the reasoning transfers unchanged. It is a
//! feature because `--no-default-features --features reports` must keep meaning something and
//! a reports-only binary has no business making outbound requests. It is in the **default
//! set** because anything outside `default` ships code no pull request has built — which is
//! precisely how a `forbid(unsafe_code)` change went green through review and red on a job
//! nobody was watching.
//!
//! `reqwest` and `tokio` are already in the default build through `tonpa` and `vault-s3`, so
//! the marginal cost of this feature on the released binary is zero crates.
//!
//! # What every request does (RFC-0048 §5)
//!
//! Three corpora wrote connectors, and each wrote these again. They live here once, for a
//! plain `url` location and for every pack:
//!
//! - **contact**: the User-Agent is `yidam-catalog/<ver>`, with `(+$YIDAM_CONTACT)` when the
//!   variable is set. A pack that says `contact = "required"` is refused at planning time
//!   without it. It is never a browser string, and a pack cannot set it.
//! - **auth**: each credential is read from the variable the pack names and sent where the pack
//!   says. A query credential is added to the URL as it is sent and to no URL this reports, so
//!   neither the report nor the `refresh:` commit can carry it. Redirects with a credential
//!   stay on the host it was sent to.
//! - **spacing**: `min_interval` per host, across one run.
//! - **atomic write**: the body streams into `<dest>.partial`, and is renamed into place only
//!   once it has all arrived.
//! - **non-200**: returned as a [`Refused`], which the caller records on the entry. It is not
//!   retried. A 403 from a publisher is its answer, and the host takes no step around it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::sources::manifest::Place;

/// The variable a fetch's contact is read from.
pub const CONTACT_VAR: &str = "YIDAM_CONTACT";

/// The Wayback Machine's availability API: a read, which finds the capture nearest a time.
pub const AVAILABILITY: &str = "https://archive.org/wayback/available";

/// What a transport learned about the bytes beyond the bytes themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fetched {
    /// The server's `Content-Type`, with any `; charset=…` parameter dropped.
    ///
    /// Recorded so a reader knows what an artifact is without fetching it, which is the
    /// reason `CatalogArtifact::media_type` exists. `None` where the server said nothing —
    /// guessing from the URL's extension would put a claim in a committed record that no
    /// server ever made.
    pub media_type: Option<String>,
}

/// A publisher's answer that was not a 2xx. Data for the entry, not an error for the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub status: u16,
    pub reason: String,
}

/// One credential, read from the environment.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    /// The variable it was read from. Reported; the value never is.
    pub var: String,
    pub value: String,
    pub place: Place,
}

/// Printed with its value withheld, so a `{:?}` in an error or a test failure cannot leak it.
impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("var", &self.var)
            .field("value", &"<withheld>")
            .field("place", &self.place)
            .finish()
    }
}

/// How one request is to be made.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    pub contact: Option<String>,
    pub credentials: Vec<Credential>,
    pub min_interval: Option<Duration>,
}

/// `YIDAM_CONTACT`, if set, or why it cannot go in a header.
pub fn contact(env: &dyn Fn(&str) -> Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = env(CONTACT_VAR) else {
        return Ok(None);
    };
    let v = raw.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v.chars().any(char::is_control) || !v.is_ascii() {
        return Err(format!(
            "{CONTACT_VAR} holds a character a User-Agent header cannot carry; set it to an \
             email address or a URL"
        ));
    }
    Ok(Some(v.to_string()))
}

/// The User-Agent every request sends.
#[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
pub fn user_agent(contact: Option<&str>) -> String {
    let base = concat!("yidam-catalog/", env!("CARGO_PKG_VERSION"));
    match contact {
        Some(c) => format!("{base} (+{c})"),
        None => base.to_string(),
    }
}

/// The host a URL names, lowercased, without user info or port.
#[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = if let Some(v6) = host.strip_prefix('[') {
        v6.split(']').next()?
    } else {
        host.split(':').next()?
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Percent-encode everything outside RFC 3986's unreserved set.
pub fn encode_component(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if crate::sources::manifest::is_unreserved(b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// The URL as sent: `url` with each query credential appended.
///
/// Separate from the URL a caller reports, which never carries one.
#[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
pub fn with_query_credentials(url: &str, credentials: &[Credential]) -> String {
    let mut out = url.to_string();
    for c in credentials {
        if let Place::Query { param } = &c.place {
            let (base, fragment) = match out.split_once('#') {
                Some((b, f)) => (b.to_string(), Some(f.to_string())),
                None => (out.clone(), None),
            };
            let sep = if base.contains('?') { '&' } else { '?' };
            out = format!("{base}{sep}{param}={}", encode_component(&c.value));
            if let Some(f) = fragment {
                out = format!("{out}#{f}");
            }
        }
    }
    out
}

/// When each host was last asked, across one run.
#[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
#[derive(Debug, Default)]
pub struct Spacing {
    last: HashMap<String, Instant>,
}

#[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
impl Spacing {
    /// How long to wait before asking `host` again, given this request's interval.
    pub fn wait(&self, host: &str, interval: Option<Duration>, now: Instant) -> Duration {
        match (interval, self.last.get(host)) {
            (Some(iv), Some(at)) => iv.saturating_sub(now.saturating_duration_since(*at)),
            _ => Duration::ZERO,
        }
    }

    pub fn mark(&mut self, host: &str, now: Instant) {
        self.last.insert(host.to_string(), now);
    }
}

/// The availability API's request for the capture nearest now.
pub fn availability_url(api: &str, url: &str) -> String {
    format!("{api}?url={}", encode_component(url))
}

/// The timestamp of the nearest capture the availability API reports, if it captured a 200.
///
/// A capture of a refusal is not a copy of the source. RFC-0048 §5 names a publisher whose
/// captures from 2024 on are its block page, and pinning one would record the block as the
/// source.
pub fn closest_capture(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let c = v.get("archived_snapshots")?.get("closest")?;
    if c.get("available").and_then(|a| a.as_bool()) != Some(true)
        || c.get("status").and_then(|s| s.as_str()) != Some("200")
    {
        return None;
    }
    let ts = c.get("timestamp")?.as_str()?;
    (ts.len() == 14 && ts.bytes().all(|b| b.is_ascii_digit())).then(|| ts.to_string())
}

/// `<dest>.partial`: where a body streams before it is renamed into place.
#[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
fn partial(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    dest.with_file_name(name)
}

/// The requests of one run, which share their spacing.
#[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
#[derive(Debug, Default)]
pub struct Session {
    spacing: Spacing,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg_attr(not(feature = "catalog-fetch"), allow(dead_code))]
    /// Wait out `url`'s host interval, then mark the host as asked.
    fn space(&mut self, url: &str, policy: &Policy) {
        let Some(host) = host_of(url) else { return };
        let wait = self
            .spacing
            .wait(&host, policy.min_interval, Instant::now());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        self.spacing.mark(&host, Instant::now());
    }

    /// `GET` a URL into `dest`, or the publisher's refusal.
    ///
    /// Writes `dest` only on success, so a caller finding a file there can rely on it being
    /// complete — the same contract `vault::Store::get` states, for the same reason.
    #[cfg(feature = "catalog-fetch")]
    pub fn get(
        &mut self,
        url: &str,
        policy: &Policy,
        dest: &Path,
    ) -> Result<std::result::Result<Fetched, Refused>> {
        use anyhow::Context;
        use std::io::Write;

        self.space(url, policy);
        let sent = with_query_credentials(url, &policy.credentials);
        let credentialed = !policy.credentials.is_empty();
        let client = client(credentialed)?;
        crate::runtime::block_on(async {
            let mut req = client.get(&sent).header(
                reqwest::header::USER_AGENT,
                user_agent(policy.contact.as_deref()),
            );
            for c in &policy.credentials {
                if let Place::Header { name, prefix } = &c.place {
                    req = req.header(name.as_str(), format!("{prefix}{}", c.value));
                }
            }
            // `without_url`: the URL sent may carry a query credential, and reqwest's message
            // would print it. The context names the URL as reported, which carries none.
            let mut resp = req
                .send()
                .await
                .map_err(|e| anyhow::anyhow!(e.without_url()))
                .with_context(|| format!("GET {url}"))?;
            let status = resp.status();
            if !status.is_success() {
                let reason = if credentialed && status.is_redirection() {
                    "redirected to another host, and this request carries a credential".into()
                } else {
                    status.canonical_reason().unwrap_or("").to_string()
                };
                return Ok(Err(Refused {
                    status: status.as_u16(),
                    reason,
                }));
            }
            let media_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.split(';').next().unwrap_or(v).trim().to_string())
                .filter(|v| !v.is_empty());

            // Streamed into `<dest>.partial`, then renamed. The reasoning is
            // `vault::Cache::put_file`'s: `rename` is atomic only within a filesystem, and a
            // half-written file under a name a digest asserts is the one failure content
            // addressing exists to make impossible. Streamed, because a 56 MB source held in
            // memory to be written once is the cost one corpus recorded paying.
            let dir = dest.parent().unwrap_or(Path::new("."));
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            let tmp = partial(dest);
            let written: Result<()> = async {
                let mut f = std::fs::File::create(&tmp)
                    .with_context(|| format!("creating {}", tmp.display()))?;
                while let Some(chunk) = resp
                    .chunk()
                    .await
                    .map_err(|e| anyhow::anyhow!(e.without_url()))
                    .with_context(|| format!("reading the response body from {url}"))?
                {
                    f.write_all(&chunk)
                        .with_context(|| format!("writing {}", tmp.display()))?;
                }
                Ok(())
            }
            .await;
            if let Err(e) = written {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
            std::fs::rename(&tmp, dest).with_context(|| {
                format!("moving {} into place at {}", tmp.display(), dest.display())
            })?;
            Ok(Ok(Fetched { media_type }))
        })
    }

    /// The refusal a build without the feature gives.
    ///
    /// It names the feature and the command that installs it, because a message saying only
    /// "not supported" sends a reader to the issue tracker for something a flag fixes.
    /// `cmd/export.rs` words its three the same way.
    #[cfg(not(feature = "catalog-fetch"))]
    pub fn get(
        &mut self,
        url: &str,
        _policy: &Policy,
        _dest: &Path,
    ) -> Result<std::result::Result<Fetched, Refused>> {
        anyhow::bail!(
            "this build cannot fetch {url} — `catalog-fetch` is not compiled in.\n  \
             It is in the default feature set, so this is a `--no-default-features` build: \
             reinstall with `cargo install yidam --features catalog-fetch`.\n  \
             `kind: file` locations need no network and work in every build."
        )
    }

    /// The Wayback capture nearest now of `url`, through the availability API at `api`.
    ///
    /// A read. It never asks the archive to capture anything: that is a write to a service the
    /// corpus does not own, and stays a person's act.
    pub fn closest_capture(
        &mut self,
        api: &str,
        url: &str,
        policy: &Policy,
        scratch: &Path,
    ) -> Result<std::result::Result<Option<String>, Refused>> {
        use anyhow::Context;
        let asked = availability_url(api, url);
        let answer = self.get(&asked, policy, scratch)?;
        let found = match answer {
            Ok(_) => {
                let body = std::fs::read_to_string(scratch)
                    .with_context(|| format!("reading {}", scratch.display()))?;
                let _ = std::fs::remove_file(scratch);
                Ok(closest_capture(&body))
            }
            Err(r) => Err(r),
        };
        Ok(found)
    }
}

/// A client whose redirects stay on one host when the request carries a credential.
///
/// reqwest drops `Authorization` on a cross-host redirect, and a header a pack names is not
/// one it knows to drop. Stopping returns the 3xx, which the caller records as a refusal.
#[cfg(feature = "catalog-fetch")]
fn client(credentialed: bool) -> Result<reqwest::Client> {
    use anyhow::Context;
    let mut b = reqwest::Client::builder();
    if credentialed {
        b = b.redirect(reqwest::redirect::Policy::custom(|attempt| {
            let from = attempt
                .previous()
                .first()
                .and_then(|u| u.host_str().map(str::to_string));
            if attempt.previous().len() > 10 {
                attempt.error("too many redirects")
            } else if from.as_deref() == attempt.url().host_str() {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }));
    }
    b.build().context("building HTTP client")
}

/// Copy a file the repository already holds into `dest`.
///
/// Ungated, and the whole of what a `kind: file` location needs. It is separate from a plain
/// `fs::copy` at the call site so that both arms of a fetch — the local one and the network
/// one — land bytes the same way and report a missing source in the same words.
pub fn read_local(src: &Path, dest: &Path) -> Result<Fetched> {
    use anyhow::Context;
    if !src.is_file() {
        anyhow::bail!(
            "{} does not exist — the entry declares it as a `kind: file` location, and \
             nothing has put it there",
            src.display()
        );
    }
    let dir = dest.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::copy(src, dest)
        .with_context(|| format!("copying {} to {}", src.display(), dest.display()))?;
    Ok(Fetched { media_type: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_file_is_copied_and_claims_no_media_type() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("registry.csv");
        std::fs::write(&src, b"a,b\n1,2\n").unwrap();
        let dest = tmp.path().join("out/staged");

        assert_eq!(
            read_local(&src, &dest).unwrap(),
            Fetched { media_type: None }
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"a,b\n1,2\n");
    }

    /// A declared location that is not there is an ordinary state — a corpus can catalogue a
    /// source before obtaining it — so it must report the path rather than panic.
    #[test]
    fn a_missing_local_file_names_the_path_it_looked_for() {
        let tmp = tempfile::tempdir().unwrap();
        let err = read_local(&tmp.path().join("absent.csv"), &tmp.path().join("out")).unwrap_err();
        assert!(err.to_string().contains("absent.csv"), "{err}");
        assert!(err.to_string().contains("does not exist"), "{err}");
    }

    /// The refusal a `--no-default-features` build gives has to name the flag that fixes it.
    #[cfg(not(feature = "catalog-fetch"))]
    #[test]
    fn a_build_without_the_feature_names_it() {
        let err = Session::new()
            .get("https://x/y", &Policy::default(), Path::new("/tmp/x"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("catalog-fetch"), "{err}");
        assert!(err.contains("kind: file"), "{err}");
    }

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn the_user_agent_names_the_contact_when_there_is_one() {
        let ua = user_agent(Some("ops@example.org"));
        assert!(ua.starts_with("yidam-catalog/"), "{ua}");
        assert!(ua.ends_with(" (+ops@example.org)"), "{ua}");
        assert!(!user_agent(None).contains('('));
    }

    #[test]
    fn a_contact_is_read_trimmed_and_refused_if_a_header_cannot_carry_it() {
        assert_eq!(
            contact(&env(&[(CONTACT_VAR, " ops@example.org ")])),
            Ok(Some("ops@example.org".into()))
        );
        assert_eq!(contact(&env(&[(CONTACT_VAR, "  ")])), Ok(None));
        assert_eq!(contact(&env(&[])), Ok(None));
        assert!(contact(&env(&[(CONTACT_VAR, "a\r\nX-Injected: 1")])).is_err());
    }

    #[test]
    fn a_host_is_read_without_user_info_or_port() {
        assert_eq!(
            host_of("https://u:p@API.Example.org:8443/x?y#z").as_deref(),
            Some("api.example.org")
        );
        assert_eq!(host_of("http://[::1]:80/").as_deref(), Some("::1"));
        assert_eq!(host_of("not a url"), None);
    }

    fn query_key(v: &str) -> Credential {
        Credential {
            var: "BLS_KEY".into(),
            value: v.into(),
            place: Place::Query {
                param: "registrationkey".into(),
            },
        }
    }

    #[test]
    fn a_query_credential_is_appended_encoded_and_before_any_fragment() {
        assert_eq!(
            with_query_credentials("https://x/a", &[query_key("k&1")]),
            "https://x/a?registrationkey=k%261"
        );
        assert_eq!(
            with_query_credentials("https://x/a?s=1#top", &[query_key("k")]),
            "https://x/a?s=1&registrationkey=k#top"
        );
    }

    /// A `{:?}` of a policy lands in test failures and error chains.
    #[test]
    fn a_credential_debug_prints_withhold_the_value() {
        let shown = format!("{:?}", query_key("s3cret"));
        assert!(!shown.contains("s3cret"), "{shown}");
        assert!(shown.contains("BLS_KEY"), "{shown}");
    }

    #[test]
    fn spacing_waits_out_the_interval_per_host() {
        let mut s = Spacing::default();
        let t0 = Instant::now();
        let iv = Some(Duration::from_millis(100));
        assert_eq!(s.wait("a", iv, t0), Duration::ZERO, "a host never asked");
        s.mark("a", t0);
        assert_eq!(
            s.wait("a", iv, t0 + Duration::from_millis(30)),
            Duration::from_millis(70)
        );
        assert_eq!(
            s.wait("a", iv, t0 + Duration::from_millis(150)),
            Duration::ZERO
        );
        assert_eq!(s.wait("b", iv, t0), Duration::ZERO, "another host");
        assert_eq!(
            s.wait("a", None, t0),
            Duration::ZERO,
            "no interval declared"
        );
    }

    #[test]
    fn the_closest_capture_is_taken_only_when_it_captured_a_200() {
        let body = |status: &str| {
            format!(
                r#"{{"archived_snapshots":{{"closest":{{"status":"{status}","available":true,
                "url":"http://web.archive.org/web/20240102030405/https://x/","timestamp":"20240102030405"}}}}}}"#
            )
        };
        assert_eq!(
            closest_capture(&body("200")).as_deref(),
            Some("20240102030405")
        );
        assert_eq!(
            closest_capture(&body("403")),
            None,
            "a capture of the refusal"
        );
        assert_eq!(closest_capture(r#"{"archived_snapshots":{}}"#), None);
        assert_eq!(closest_capture("not json"), None);
        assert_eq!(
            availability_url(AVAILABILITY, "https://x/a?b=1"),
            "https://archive.org/wayback/available?url=https%3A%2F%2Fx%2Fa%3Fb%3D1"
        );
    }

    /// The request as a publisher sees it, against a listener on loopback.
    #[cfg(feature = "catalog-fetch")]
    mod http {
        use super::*;
        use std::io::{Read, Write};

        /// Answer each of `answers` in turn, and return what each request said.
        fn serve(answers: Vec<String>) -> (u16, std::thread::JoinHandle<Vec<String>>) {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let handle = std::thread::spawn(move || {
                let mut seen = Vec::new();
                for answer in answers {
                    let (mut sock, _) = listener.accept().unwrap();
                    let mut got = Vec::new();
                    let mut buf = [0u8; 4096];
                    while !String::from_utf8_lossy(&got).contains("\r\n\r\n") {
                        let n = sock.read(&mut buf).unwrap();
                        if n == 0 {
                            break;
                        }
                        got.extend_from_slice(&buf[..n]);
                    }
                    sock.write_all(answer.as_bytes()).unwrap();
                    seen.push(String::from_utf8_lossy(&got).to_string());
                }
                seen
            });
            (port, handle)
        }

        fn ok(body: &str) -> String {
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/csv; charset=utf-8\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
        }

        #[test]
        fn a_request_carries_the_contact_and_the_credentials_and_lands_atomically() {
            let (port, server) = serve(vec![ok("a,b\n1,2\n")]);
            let dir = tempfile::tempdir().unwrap();
            let dest = dir.path().join("staged");
            let policy = Policy {
                contact: Some("ops@example.org".into()),
                credentials: vec![
                    Credential {
                        var: "CL_TOKEN".into(),
                        value: "tok".into(),
                        place: Place::Header {
                            name: "Authorization".into(),
                            prefix: "Token ".into(),
                        },
                    },
                    query_key("k1"),
                ],
                min_interval: None,
            };
            let got = Session::new()
                .get(&format!("http://127.0.0.1:{port}/data.csv"), &policy, &dest)
                .unwrap();
            assert_eq!(
                got,
                Ok(Fetched {
                    media_type: Some("text/csv".into())
                })
            );
            assert_eq!(std::fs::read(&dest).unwrap(), b"a,b\n1,2\n");
            assert!(!partial(&dest).exists(), "the .partial is renamed away");

            let seen = server.join().unwrap().remove(0).to_ascii_lowercase();
            assert!(
                seen.starts_with("get /data.csv?registrationkey=k1 "),
                "{seen}"
            );
            assert!(seen.contains("authorization: token tok"), "{seen}");
            assert!(
                seen.contains("(+ops@example.org)"),
                "the contact is in the user-agent: {seen}"
            );
        }

        /// A 403 is the publisher's answer. It comes back as data, nothing is written, and it
        /// is asked once: the listener answers one request and would hang a retry.
        #[test]
        fn a_refusal_is_returned_once_and_writes_nothing() {
            let (port, server) = serve(vec![
                "HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".into(),
            ]);
            let dir = tempfile::tempdir().unwrap();
            let dest = dir.path().join("staged");
            let got = Session::new()
                .get(
                    &format!("http://127.0.0.1:{port}/x"),
                    &Policy::default(),
                    &dest,
                )
                .unwrap();
            assert_eq!(
                got,
                Err(Refused {
                    status: 403,
                    reason: "Forbidden".into()
                })
            );
            assert!(!dest.exists() && !partial(&dest).exists());
            assert_eq!(server.join().unwrap().len(), 1);
        }

        #[test]
        fn the_availability_api_is_read_and_its_closest_capture_returned() {
            let (port, server) = serve(vec![ok(
                r#"{"archived_snapshots":{"closest":{"status":"200","available":true,"timestamp":"20240102030405"}}}"#,
            )]);
            let dir = tempfile::tempdir().unwrap();
            let got = Session::new()
                .closest_capture(
                    &format!("http://127.0.0.1:{port}/wayback/available"),
                    "https://x/a",
                    &Policy::default(),
                    &dir.path().join("probe"),
                )
                .unwrap();
            assert_eq!(got, Ok(Some("20240102030405".into())));
            let seen = server.join().unwrap().remove(0);
            assert!(
                seen.starts_with("GET /wayback/available?url=https%3A%2F%2Fx%2Fa "),
                "{seen}"
            );
            assert!(!dir.path().join("probe").exists());
        }
    }
}
