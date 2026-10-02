//! `pack.toml`, as written (RFC-0048 §3).
//!
//! ```toml
//! [pack]
//! name    = "scholarly"
//! version = "0.1.0"
//!
//! [scheme.doi]
//! pattern  = '^10\.\d{4,9}/\S+$'
//! type     = "paper"
//! resolve  = { template = "https://api.crossref.org/works/{id}", media = "application/json" }
//! describe = "transforms/crossref.glu"
//! then     = [{ scheme = "pmc", from = "describe.pmcid" }]
//!
//! [transport]
//! contact      = "required"
//! min_interval = "100ms"
//! auth         = []
//! archive      = "wayback"
//!
//! [defaults]
//! ttl_days        = 365
//! redistributable = false
//!
//! [fixtures]
//! "doi:10.1167/tvst.8.5.14" = "crossref-tvst.json"
//! ```
//!
//! Every table refuses a key it does not know. A misspelt `min_intreval` would otherwise be a
//! pack that fetches with no spacing and says nothing.
//!
//! `[fixtures]` maps an identifier to a file under `fixtures/`. It is how a fixture is claimed
//! by a scheme: the identifier names the scheme, and its local id has to match that scheme's
//! pattern. A file under `fixtures/` that no identifier names belongs to nothing.
//!
//! `[vendored]` is written by `yidam-vendor-update` onto the copy it makes, and never by an
//! author. It records the pin the copy satisfied and where it came from.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub pack: Header,
    #[serde(default)]
    pub scheme: BTreeMap<String, Scheme>,
    #[serde(default)]
    pub transport: Transport,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub fixtures: BTreeMap<String, String>,
    #[serde(default)]
    pub vendored: Option<Vendored>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub name: String,
    /// The pack's own semver, `MAJOR.MINOR.PATCH`. See [`super::version`].
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scheme {
    /// A regular expression the local id must match. Its named groups are slots the resolve
    /// template may use beside `{id}`.
    pub pattern: String,
    /// One of [`crate::parse::CATALOG_TYPES`].
    #[serde(rename = "type")]
    pub kind: String,
    pub resolve: Resolve,
    /// `transforms/<name>.glu`: publisher metadata to entry fields (§6, #1318).
    #[serde(default)]
    pub describe: Option<String>,
    /// `transforms/<name>.glu`: a fetched artifact to a derived reading (§6, #1318).
    #[serde(default)]
    pub extract: Option<String>,
    /// The identifiers this one leads to.
    #[serde(default)]
    pub then: Vec<Then>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolve {
    /// An `http(s)` address with `{id}` and named-group slots.
    pub template: String,
    /// The media type the publisher answers in.
    #[serde(default)]
    pub media: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Then {
    /// A scheme in this pack.
    pub scheme: String,
    /// Where the next local id comes from: `describe.<field>`.
    pub from: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transport {
    /// Whether a fetch needs `YIDAM_CONTACT` in its User-Agent.
    #[serde(default)]
    pub contact: Contact,
    /// The least time between two requests to this pack's publisher: `250ms`, `2s`.
    #[serde(default)]
    pub min_interval: Option<String>,
    /// Environment variable names, never values.
    #[serde(default)]
    pub auth: Vec<String>,
    #[serde(default)]
    pub archive: Option<Archive>,
    /// Why this publisher cannot be fetched from at all.
    #[serde(default)]
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Contact {
    Required,
    #[default]
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Archive {
    Wayback,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    #[serde(default)]
    pub ttl_days: Option<u32>,
    #[serde(default)]
    pub redistributable: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vendored {
    /// The `prelude_sources` entry this copy satisfied, verbatim.
    pub pin: String,
    /// The repository it was copied from: the template's origin, or the peer a `from` names.
    pub from: String,
    pub commit: String,
}

/// A duration as a pack writes one: digits, then `ms` or `s`.
pub fn parse_interval(raw: &str) -> Option<std::time::Duration> {
    let (digits, unit) = match raw.strip_suffix("ms") {
        Some(d) => (d, 1),
        None => (raw.strip_suffix('s')?, 1000),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u64 = digits.parse().ok()?;
    Some(std::time::Duration::from_millis(n.checked_mul(unit)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rfc_example_parses() {
        let m: Manifest = toml::from_str(
            r#"
[pack]
name    = "scholarly"
version = "0.1.0"

[scheme.doi]
pattern  = '^10\.\d{4,9}/\S+$'
type     = "paper"
resolve  = { template = "https://api.crossref.org/works/{id}", media = "application/json" }
describe = "transforms/crossref.glu"
then     = [{ scheme = "pmc", from = "describe.pmcid" }]

[scheme.pmc]
pattern = '^PMC\d+$'
type    = "paper"
resolve = { template = "https://www.ebi.ac.uk/europepmc/webservices/rest/{id}/fullTextXML", media = "application/xml" }

[transport]
contact       = "required"
min_interval  = "100ms"
auth          = []
archive       = "wayback"

[defaults]
ttl_days        = 365
redistributable = false
"#,
        )
        .unwrap();
        assert_eq!(m.scheme.len(), 2);
        assert_eq!(m.transport.contact, Contact::Required);
        assert_eq!(m.scheme["doi"].then[0].scheme, "pmc");
    }

    #[test]
    fn a_misspelt_key_is_refused_rather_than_ignored() {
        let err = toml::from_str::<Manifest>(
            "[pack]\nname = \"a\"\nversion = \"0.1.0\"\n[transport]\nmin_intreval = \"1s\"\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("min_intreval"), "{err}");
    }

    #[test]
    fn intervals_are_milliseconds_or_seconds() {
        assert_eq!(
            parse_interval("100ms"),
            Some(std::time::Duration::from_millis(100))
        );
        assert_eq!(
            parse_interval("2s"),
            Some(std::time::Duration::from_secs(2))
        );
        for bad in ["", "ms", "1.5s", "10", "-1s", "1m"] {
            assert_eq!(parse_interval(bad), None, "{bad}");
        }
    }
}
