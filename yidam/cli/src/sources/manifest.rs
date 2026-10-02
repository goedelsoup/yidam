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

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transport {
    /// Whether a fetch needs `YIDAM_CONTACT` in its User-Agent.
    #[serde(default)]
    pub contact: Contact,
    /// The least time between two requests to this pack's publisher: `250ms`, `2s`.
    #[serde(default)]
    pub min_interval: Option<String>,
    /// Credentials, each named by its environment variable and never by its value.
    #[serde(default)]
    pub auth: Vec<Auth>,
    #[serde(default)]
    pub archive: Option<Archive>,
    /// Why this publisher cannot be fetched from at all.
    #[serde(default)]
    pub blocked: Option<String>,
}

/// One credential a fetch sends, as `pack.toml` declares it.
///
/// ```toml
/// auth = [
///   { env = "COURTLISTENER_TOKEN", header = "Authorization", prefix = "Token " },
///   { env = "BLS_KEY", query = "registrationkey" },
/// ]
/// ```
///
/// A bare name (`auth = ["COURTLISTENER_TOKEN"]`) parses, and is refused by [`Auth::spec`]: it
/// says which variable holds the credential and not how the publisher wants it sent, and a
/// default would be a guess that fails as a 401 on the publisher's side.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Auth {
    Name(String),
    Declared(AuthSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthSpec {
    /// The environment variable holding the credential.
    pub env: String,
    /// Send it in this request header.
    #[serde(default)]
    pub header: Option<String>,
    /// Written before the value in the header: `"Token "`, `"Bearer "`.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Send it as this query parameter.
    #[serde(default)]
    pub query: Option<String>,
}

/// Where a credential goes, once [`Auth::spec`] has accepted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Header { name: String, prefix: String },
    Query { param: String },
}

impl Auth {
    /// The variable this credential is read from, however it was written.
    pub fn env(&self) -> &str {
        match self {
            Self::Name(n) => n,
            Self::Declared(s) => &s.env,
        }
    }

    /// The variable and where its value is sent, or why the declaration does not say.
    pub fn spec(&self) -> Result<(&str, Place), String> {
        let s = match self {
            Self::Name(n) => {
                return Err(format!(
                    "`{n}` names a variable and not how it is sent; write \
                     {{ env = \"{n}\", header = \"Authorization\", prefix = \"Bearer \" }} \
                     or {{ env = \"{n}\", query = \"<param>\" }}"
                ))
            }
            Self::Declared(s) => s,
        };
        let env = s.env.as_str();
        if !is_env_name(env) {
            return Err(format!(
                "`{env}` is not an environment variable name; auth names a variable, never a value"
            ));
        }
        match (&s.header, &s.query) {
            (Some(name), None) => {
                if !is_header_name(name) {
                    return Err(format!("`{env}`: `{name}` is not a header name"));
                }
                // The User-Agent is the contact line and nothing else (RFC-0048 §5). A pack that
                // could set it could send a browser string.
                if name.eq_ignore_ascii_case("user-agent") {
                    return Err(format!(
                        "`{env}` is sent as the User-Agent; that header carries the contact, and a pack does not set it"
                    ));
                }
                let prefix = s.prefix.clone().unwrap_or_default();
                if prefix.chars().any(char::is_control) {
                    return Err(format!("`{env}`: the prefix holds a control character"));
                }
                Ok((
                    env,
                    Place::Header {
                        name: name.clone(),
                        prefix,
                    },
                ))
            }
            (None, Some(param)) => {
                if s.prefix.is_some() {
                    return Err(format!(
                        "`{env}` is sent as a query parameter, which takes no prefix"
                    ));
                }
                if param.is_empty() || !param.bytes().all(is_unreserved) {
                    return Err(format!("`{env}`: `{param}` is not a query parameter name"));
                }
                Ok((
                    env,
                    Place::Query {
                        param: param.clone(),
                    },
                ))
            }
            (Some(_), Some(_)) => Err(format!(
                "`{env}` declares both a header and a query parameter; it is sent one way"
            )),
            (None, None) => Err(format!(
                "`{env}` declares neither `header` nor `query`, so nothing says how it is sent"
            )),
        }
    }
}

pub(crate) fn is_env_name(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_uppercase() || b == b'_')
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// An HTTP token, narrowed to what a header a publisher documents is actually called.
fn is_header_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// RFC 3986's unreserved set, which needs no encoding in a query.
pub(crate) fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
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

    fn auth(toml_src: &str) -> Auth {
        #[derive(Deserialize)]
        struct T {
            auth: Vec<Auth>,
        }
        toml::from_str::<T>(toml_src).unwrap().auth.remove(0)
    }

    #[test]
    fn a_credential_is_sent_as_its_declaration_says() {
        assert_eq!(
            auth(r#"auth = [{ env = "CL_TOKEN", header = "Authorization", prefix = "Token " }]"#)
                .spec(),
            Ok((
                "CL_TOKEN",
                Place::Header {
                    name: "Authorization".into(),
                    prefix: "Token ".into()
                }
            ))
        );
        assert_eq!(
            auth(r#"auth = [{ env = "BLS_KEY", query = "registrationkey" }]"#).spec(),
            Ok((
                "BLS_KEY",
                Place::Query {
                    param: "registrationkey".into()
                }
            ))
        );
    }

    /// A bare name still parses, so `source check` can say what to write instead of a TOML
    /// type error. It is refused, because it does not say how the credential is sent.
    #[test]
    fn a_bare_name_parses_and_is_refused_for_saying_too_little() {
        let a = auth(r#"auth = ["CL_TOKEN"]"#);
        assert_eq!(a.env(), "CL_TOKEN");
        let err = a.spec().unwrap_err();
        assert!(err.contains("not how it is sent"), "{err}");
    }

    #[test]
    fn a_credential_declared_wrongly_is_refused_with_the_reason() {
        for (src, says) in [
            (
                r#"auth = [{ env = "K", header = "X-Key", query = "k" }]"#,
                "both",
            ),
            (r#"auth = [{ env = "K" }]"#, "neither"),
            (
                r#"auth = [{ env = "K", header = "User-Agent" }]"#,
                "contact",
            ),
            (
                r#"auth = [{ env = "K", header = "Bad Header" }]"#,
                "not a header name",
            ),
            (
                r#"auth = [{ env = "K", query = "a&b" }]"#,
                "not a query parameter",
            ),
            (
                r#"auth = [{ env = "K", query = "k", prefix = "x" }]"#,
                "no prefix",
            ),
            (
                r#"auth = [{ env = "lower", header = "X-Key" }]"#,
                "not an environment",
            ),
        ] {
            let err = auth(src).spec().unwrap_err();
            assert!(err.contains(says), "{src}: {err}");
        }
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
