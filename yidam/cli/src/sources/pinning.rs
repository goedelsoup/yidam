//! A mutable identifier, read once for the version it names today (#1343).
//!
//! Some identifiers name a moving target. `wayback:<url>` is whichever capture the archive
//! made last, `github:<owner>/<repo>@main/<path>` is wherever the branch points, and
//! `wikipedia:en/<title>` is the article as it reads now. A location names bytes that stay
//! put, so such an identifier is not written as one. `source add` asks the scheme's
//! [`PinRead`] for the version it names today, and writes the pinned form the scheme's
//! `pattern` admits: a capture timestamp, a commit SHA, a revision id.
//!
//! The read is `source add`'s, through its [`super::draft::Asker`], as a lookup scheme's page
//! is (#1342). A fetch follows only the pinned form, and never asks the read. Pure and
//! offline: this module binds the read's address, and turns its answer into the identifier.

use std::fmt::Write as _;

use super::manifest::PinRead;
use super::search;
use crate::cmd::catalog::location::bind;

/// What a mutable identifier's pin read asks, and how its answer becomes the pinned identifier.
#[derive(Debug, Clone)]
pub struct Pinning {
    /// `<pack>@<version>`.
    pub pack: String,
    pub scheme: String,
    /// `scheme:local-id`, as asked.
    pub identifier: String,
    /// The bound read.
    pub url: String,
    pub(super) media: String,
    pub(super) value: String,
    /// The scheme's `pinned` template.
    pub(super) pinned: String,
    /// The pattern the pinned local id has to match.
    pub(super) pattern: regex::Regex,
    /// `{id}` and each of `mutable`'s groups, unencoded.
    pub(super) bindings: Vec<(String, String)>,
}

impl Pinning {
    /// Bind `pin`'s read for `local`, a local id `mutable` captured as `caps`.
    pub(super) fn of(
        pack: &str,
        scheme: &str,
        local: &str,
        pin: &PinRead,
        mutable: &regex::Regex,
        pattern: &regex::Regex,
    ) -> Option<Result<Self, Vec<String>>> {
        let caps = mutable.captures(local)?;
        let mut bindings = vec![("id".to_string(), local.to_string())];
        for group in mutable.capture_names().flatten() {
            if let Some(v) = caps.name(group) {
                bindings.push((group.to_string(), v.as_str().to_string()));
            }
        }
        let encoded: Vec<(String, String)> = bindings
            .iter()
            .map(|(k, v)| (k.clone(), encode(v)))
            .collect();
        Some(bind(&pin.read, &encoded).map(|url| Self {
            pack: pack.to_string(),
            scheme: scheme.to_string(),
            identifier: format!("{scheme}:{local}"),
            url,
            media: pin.media.clone(),
            value: pin.value.clone(),
            pinned: pin.pinned.clone(),
            pattern: pattern.clone(),
            bindings,
        }))
    }

    /// The value the read answered with, and the identifier it pins. `Err` says why the answer
    /// pins nothing: no value at the path, or a value the scheme's pattern does not admit.
    pub fn pin(&self, answer: &[u8]) -> Result<(String, String), String> {
        let value = search::value_at(&self.media, answer, &self.value)?.ok_or_else(|| {
            format!(
                "`{}` answered with nothing at `{}`, so there is no version to pin",
                self.url, self.value
            )
        })?;
        // The value last, so nothing the publisher answered is read as a slot.
        let mut bindings = self.bindings.clone();
        bindings.push(("pin".to_string(), value.clone()));
        let local = bind(&self.pinned, &bindings).map_err(|slots| {
            format!(
                "{}'s `pinned` leaves {} unbound. `yidam source check` reports the pack",
                self.pack,
                slots
                    .iter()
                    .map(|s| format!("`{{{s}}}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        if !self.pattern.is_match(&local) {
            return Err(format!(
                "`{}` answered `{value}` at `{}`, which pins `{local}`, and [scheme.{}] pattern \
                 `{}` does not admit it",
                self.url, self.value, self.scheme, self.pattern
            ));
        }
        Ok((value, format!("{}:{local}", self.scheme)))
    }
}

/// `s` percent-encoded but for RFC 3986's unreserved characters, so a bound value is one query
/// value or one path segment: a URL with its own `?` and `&` stays inside the read's `url=`.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wikipedia() -> Pinning {
        let pin = PinRead {
            mutable: r"^(?P<lang>[a-z][a-z-]*)/(?P<title>[^\s@#]+)$".into(),
            read: "https://{lang}.wikipedia.org/w/api.php?titles={title}".into(),
            media: "application/json".into(),
            value: "query/pages/0/revisions/0/revid".into(),
            pinned: "{lang}/{title}@{pin}".into(),
        };
        let mutable = regex::Regex::new(&pin.mutable).unwrap();
        let pattern =
            regex::Regex::new(r"^(?P<lang>[a-z][a-z-]*)/(?P<title>[^\s@#]+)@(?P<pin>\d+)$")
                .unwrap();
        Pinning::of(
            "archive@0.2.0",
            "wikipedia",
            "en/Allen_County,_Ohio",
            &pin,
            &mutable,
            &pattern,
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn the_read_binds_each_slot_encoded() {
        assert_eq!(
            wikipedia().url,
            "https://en.wikipedia.org/w/api.php?titles=Allen_County%2C_Ohio"
        );
        assert_eq!(
            encode("https://x.org/a?b=1&c=é"),
            "https%3A%2F%2Fx.org%2Fa%3Fb%3D1%26c%3D%C3%A9"
        );
    }

    /// The pinned identifier keeps the title as written: only the read's address is encoded.
    #[test]
    fn the_value_read_is_written_into_the_pinned_identifier() {
        let answer = br#"{"query": {"pages": [{"revisions": [{"revid": 1187654321}]}]}}"#;
        assert_eq!(
            wikipedia().pin(answer),
            Ok((
                "1187654321".into(),
                "wikipedia:en/Allen_County,_Ohio@1187654321".into()
            ))
        );
    }

    #[test]
    fn an_answer_with_no_value_or_a_value_the_pattern_refuses_pins_nothing() {
        let missing = wikipedia()
            .pin(br#"{"query": {"pages": [{"missing": true}]}}"#)
            .unwrap_err();
        assert!(
            missing.contains("nothing at `query/pages/0/revisions/0/revid`"),
            "{missing}"
        );
        let odd = wikipedia()
            .pin(br#"{"query": {"pages": [{"revisions": [{"revid": "12 34"}]}]}}"#)
            .unwrap_err();
        assert!(odd.contains("does not admit"), "{odd}");
    }
}
