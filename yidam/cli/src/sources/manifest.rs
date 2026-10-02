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
//! ttl_days = 365
//!
//! [fixtures]
//! "doi:10.1167/tvst.8.5.14" = "crossref-tvst.json"
//!
//! [search]
//! scheme   = "doi"
//! template = "https://api.crossref.org/works?query={query}&rows={limit}"
//! media    = "application/json"
//! items    = "message/items/*"
//! id       = "DOI"
//! title    = "title/0"
//! fixtures = { "retinal imaging" = "crossref-search.json" }
//! ```
//!
//! Every table refuses a key it does not know. A misspelt `min_intreval` would otherwise be a
//! pack that fetches with no spacing and says nothing.
//!
//! `[fixtures]` maps an identifier to a file under `fixtures/`. It is how a fixture is claimed
//! by a scheme: the identifier names the scheme, and its local id has to match that scheme's
//! pattern. A file under `fixtures/` that no identifier names belongs to nothing.
//!
//! `[search]` is the endpoint `source search` asks (#1316). It is one per pack, because the
//! command names a pack and not a scheme. What it answers is read by path rather than by a
//! transform, so a search works in a build without `source-transforms`: each field matching
//! `items` is one candidate, and `id` and `title` are paths below it. Its fixtures are its own,
//! keyed by query, since a query is not an identifier any scheme could claim.
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
    pub search: Option<Search>,
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
    /// How a mutable local id becomes the pinned one `pattern` admits (#1343).
    #[serde(default)]
    pub pin: Option<PinRead>,
    /// How the address is read past its first page (#1341).
    #[serde(default)]
    pub paginate: Option<Paginate>,
}

/// How a paged service is read to its end (#1341):
///
/// ```toml
/// paginate = { offset = "resultOffset", items = "features/*", until = "!exceededTransferLimit" }
/// paginate = { offset = "$offset", limit = "$limit", size = 1000, items = "*" }
/// ```
///
/// A paged service answers a single `GET` with its first page and HTTP 200, so the page looks
/// like the whole. With this declared, a fetch asks page after page, each at the offset the
/// records already received reach, and records one artifact: the first page's document with
/// every page's records at `items`. The offset advances by what arrived and not by `size`,
/// because a server whose own cap is lower than the size asked returns fewer and says nothing
/// else.
///
/// The fetch stops where `until` says, or else on a page shorter than `size`. Only JSON is
/// paged.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paginate {
    /// The query parameter carrying how many records to skip.
    pub offset: String,
    /// The query parameter carrying how many records to ask for. Paired with `size`.
    #[serde(default)]
    pub limit: Option<String>,
    /// How many records to ask for in each page. Paired with `limit`.
    #[serde(default)]
    pub size: Option<u32>,
    /// The path of each record in a page, as `[search]` writes `items`: `features/*`, or `*`
    /// for a page that is one array.
    pub items: String,
    /// The path of a boolean that ends the paging when true, or with a leading `!` when not
    /// true: `!exceededTransferLimit`. Absent, a page shorter than `size` is the last.
    #[serde(default)]
    pub until: Option<String>,
}

/// A read `source add` makes to fix a mutable identifier to one version (#1343):
///
/// ```toml
/// [scheme.wikipedia.pin]
/// mutable = '^(?P<lang>[a-z][a-z-]*)/(?P<title>[^\s@#]+)$'
/// read    = "https://{lang}.wikipedia.org/w/api.php?action=query&prop=revisions&titles={title}"
/// media   = "application/json"
/// value   = "query/pages/0/revisions/0/revid"
/// pinned  = "{lang}/{title}@{pin}"
/// ```
///
/// A title names whatever the article says today, and a location names bytes that stay put.
/// The scheme's `pattern` admits only the pinned form, and its group `(?P<pin>…)` is the value
/// read. A fetch follows only the pinned form, so it never asks `read`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRead {
    /// A regular expression, anchored `^…$`, the mutable local id matches. Its named groups
    /// are the slots `read` and `pinned` bind.
    pub mutable: String,
    /// An `http(s)` address. Each slot is bound percent-encoded, so a URL lands in a query.
    pub read: String,
    /// The media type `read` answers in.
    pub media: String,
    /// The path of the value in the answer, as `[search]` writes paths.
    pub value: String,
    /// The pinned local id: `mutable`'s slots, and `{pin}` for the value read.
    pub pinned: String,
}

/// How a scheme's identifier becomes an address. Exactly one of `template`, `listing` and
/// `dcat` (#1342):
///
/// ```toml
/// resolve = { template = "https://api.crossref.org/works/{id}" }
/// resolve = { listing = "https://www.ncei.noaa.gov/pub/data/cirs/climdiv/",
///             match = '^climdiv-{id}-v1\.0\.0-\d{8}$', pick = "latest" }
/// resolve = { dcat = "data.cms.gov", media = "text/csv" }
/// ```
///
/// A template is bound offline. A listing or a catalog is a page the publisher keeps current,
/// read once by `source add`, which writes the name it picked into the identifier as a pin:
/// `climdiv:pcpndv@climdiv-pcpndv-v1.0.0-20261001`. A fetch follows only the pin, so an entry
/// stays the file that was read. See [`super::lookup`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolve {
    /// An `http(s)` address with `{id}` and named-group slots.
    #[serde(default)]
    pub template: Option<String>,
    /// An `http(s)` page whose links name the publisher's files, with the same slots.
    #[serde(default)]
    pub listing: Option<String>,
    /// A host whose DCAT-US catalog, `https://<host>/data.json`, names each dataset's files.
    #[serde(default)]
    pub dcat: Option<String>,
    /// A regular expression, anchored `^…$`, a listed name must match to be picked. Its
    /// `{id}` and named-group slots are bound with the values escaped.
    #[serde(default, rename = "match")]
    pub matches: Option<String>,
    /// Which matching name is picked. `only` when unsaid.
    #[serde(default)]
    pub pick: Option<Pick>,
    /// The media type the publisher answers in. For `dcat`, also the `mediaType` a
    /// distribution must declare to be picked.
    #[serde(default)]
    pub media: Option<String>,
}

/// Which of several matching names a listing or catalog resolves to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pick {
    /// The greatest, comparing runs of digits as numbers: the newest release date.
    Latest,
    /// The one name that matches. Several is a refusal that names them.
    #[default]
    Only,
}

/// A [`Resolve`] once [`Resolve::form`] has accepted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form<'a> {
    Template(&'a str),
    Listing {
        listing: &'a str,
        matches: &'a str,
        pick: Pick,
    },
    Dcat {
        host: &'a str,
        matches: Option<&'a str>,
        pick: Pick,
        media: Option<&'a str>,
    },
}

impl Form<'_> {
    /// Whether the address is read off a page rather than bound, so an identifier needs a pin.
    pub fn looks_up(&self) -> bool {
        !matches!(self, Self::Template(_))
    }
}

impl Resolve {
    /// Which form this is, or why the keys written do not make one.
    pub fn form(&self) -> Result<Form<'_>, String> {
        let given: Vec<&str> = [
            ("template", self.template.is_some()),
            ("listing", self.listing.is_some()),
            ("dcat", self.dcat.is_some()),
        ]
        .into_iter()
        .filter_map(|(k, set)| set.then_some(k))
        .collect();
        let pick = self.pick.unwrap_or_default();
        match (given.as_slice(), &self.template, &self.listing, &self.dcat) {
            (["template"], Some(t), _, _) => {
                if self.matches.is_some() || self.pick.is_some() {
                    return Err(
                        "a template is bound, not picked; `match` and `pick` belong to a `listing` or `dcat`"
                            .into(),
                    );
                }
                Ok(Form::Template(t))
            }
            (["listing"], _, Some(listing), _) => match &self.matches {
                Some(m) => Ok(Form::Listing {
                    listing,
                    matches: m,
                    pick,
                }),
                None => Err(
                    "a `listing` needs `match`, or every link on the page would be a candidate"
                        .into(),
                ),
            },
            (["dcat"], _, _, Some(host)) => Ok(Form::Dcat {
                host,
                matches: self.matches.as_deref(),
                pick,
                media: self.media.as_deref(),
            }),
            ([], ..) => Err("names no `template`, `listing` or `dcat`".into()),
            (many, ..) => Err(format!(
                "names {}; a scheme resolves one way",
                many.iter()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            )),
        }
    }
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

/// What a new entry from this pack starts with.
///
/// **No `redistributable`** (#1339). It is a licence, and it belongs on an artifact's record,
/// where the operator writes it about one source. A pack-wide `true` would assert a licence for
/// every DOI a scheme resolves. A `false` would change nothing, because `vault push` already
/// refuses an artifact that does not say. The key is refused rather than ignored, so a pack
/// that sets it fails to load and does not appear to grant anything.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    #[serde(default)]
    pub ttl_days: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    /// The scheme a candidate's `id` is a local id in. One this pack declares.
    pub scheme: String,
    /// An `http(s)` address with a `{query}` slot, and optionally `{limit}`.
    pub template: String,
    /// The media type the endpoint answers in.
    pub media: String,
    /// The path of each result, with `*` for any one segment: `message/items/*`.
    pub items: String,
    /// The path of a result's local id, below the result.
    pub id: String,
    /// The path of a result's title, below the result.
    #[serde(default)]
    pub title: Option<String>,
    /// A query to the file under `fixtures/` that records what the endpoint answered.
    #[serde(default)]
    pub fixtures: BTreeMap<String, String>,
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
ttl_days = 365
"#,
        )
        .unwrap();
        assert_eq!(m.scheme.len(), 2);
        assert_eq!(m.transport.contact, Contact::Required);
        assert_eq!(m.scheme["doi"].then[0].scheme, "pmc");
        assert!(m.search.is_none());
    }

    #[test]
    fn a_search_table_parses_and_refuses_a_key_it_does_not_know() {
        let head = "[pack]\nname = \"a\"\nversion = \"0.1.0\"\n[search]\nscheme = \"doi\"\n\
                    template = \"https://e.org/?q={query}\"\nmedia = \"application/json\"\n\
                    items = \"items/*\"\nid = \"DOI\"\n";
        let m: Manifest = toml::from_str(head).unwrap();
        let s = m.search.unwrap();
        assert_eq!((s.items.as_str(), s.title), ("items/*", None));
        let err = toml::from_str::<Manifest>(&format!("{head}titel = \"t\"\n")).unwrap_err();
        assert!(err.to_string().contains("titel"), "{err}");
    }

    fn resolve(src: &str) -> Resolve {
        #[derive(Deserialize)]
        struct T {
            resolve: Resolve,
        }
        toml::from_str::<T>(src).unwrap().resolve
    }

    #[test]
    fn each_resolve_form_parses_to_its_form() {
        assert_eq!(
            resolve(r#"resolve = { template = "https://e.org/{id}" }"#).form(),
            Ok(Form::Template("https://e.org/{id}"))
        );
        assert_eq!(
            resolve(r#"resolve = { listing = "https://e.org/d/", match = '^{id}-\d{8}$', pick = "latest" }"#)
                .form(),
            Ok(Form::Listing {
                listing: "https://e.org/d/",
                matches: r"^{id}-\d{8}$",
                pick: Pick::Latest
            })
        );
        assert_eq!(
            resolve(r#"resolve = { dcat = "data.cms.gov", media = "text/csv" }"#).form(),
            Ok(Form::Dcat {
                host: "data.cms.gov",
                matches: None,
                pick: Pick::Only,
                media: Some("text/csv")
            })
        );
    }

    #[test]
    fn a_resolve_that_is_not_one_form_says_why() {
        for (src, says) in [
            (r#"resolve = { media = "text/csv" }"#, "names no"),
            (
                r#"resolve = { template = "https://e.org/{id}", dcat = "e.org" }"#,
                "`template` and `dcat`",
            ),
            (
                r#"resolve = { template = "https://e.org/{id}", pick = "latest" }"#,
                "bound, not picked",
            ),
            (
                r#"resolve = { listing = "https://e.org/" }"#,
                "needs `match`",
            ),
        ] {
            let err = resolve(src).form().unwrap_err();
            assert!(err.contains(says), "{src}: {err}");
        }
    }

    #[test]
    fn a_misspelt_key_is_refused_rather_than_ignored() {
        let err = toml::from_str::<Manifest>(
            "[pack]\nname = \"a\"\nversion = \"0.1.0\"\n[transport]\nmin_intreval = \"1s\"\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("min_intreval"), "{err}");
    }

    /// A pack cannot set a licence (#1339). A pack written to RFC-0048's first example still
    /// carries the key, and it fails to load rather than seeming to grant anything.
    #[test]
    fn a_pack_cannot_default_redistributable() {
        let err = toml::from_str::<Manifest>(
            "[pack]\nname = \"a\"\nversion = \"0.1.0\"\n[defaults]\nredistributable = true\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("redistributable"), "{err}");
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
