//! A listing or a catalog, read for the file an identifier names (#1342).
//!
//! Some publishers have no address a template reaches. NOAA dates each climdiv file by its
//! release, FHFA moved its HPI files, and CMS writes a fresh `<uuid>` path for each release.
//! What they do keep current is a page naming the files: a directory listing, or the
//! DCAT-US catalog every federal agency publishes at `https://<host>/data.json`.
//!
//! Reading the page is `source add`'s, through its [`super::draft::Asker`]. The name it picks
//! is written into the identifier as a pin, `scheme:local-id@<pin>`, and a fetch follows only
//! the pin. An entry is then the file somebody read, not whichever file the page lists today.
//!
//! Pure and offline: this module turns a page's bytes into candidate names, matches and picks
//! one, and joins a pin to the address it names. Every pin stays on the host the pack declared,
//! so a page cannot send a fetch, or the pack's credentials, anywhere else.

use std::cmp::Ordering;

use super::manifest::{Pick, Transport};

/// What a lookup scheme reads, and how it picks from what it reads.
#[derive(Debug, Clone)]
pub struct Lookup {
    /// `<pack>@<version>`.
    pub pack: String,
    pub scheme: String,
    /// `scheme:local-id`, without a pin.
    pub identifier: String,
    /// The page read: the bound listing, or `https://<host>/data.json`.
    pub url: String,
    pub transport: Transport,
    pub(super) page: Page,
    pub(super) base: Base,
    /// The bound `match`, which a name must match whole.
    pub(super) matches: Option<regex::Regex>,
    pub(super) pick: Pick,
    pub(super) media: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum Page {
    Listing,
    /// The local id, which names a dataset.
    Dcat {
        local: String,
    },
}

/// Where a pin is joined: the page's origin and directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Base {
    /// `https://host`, lowercased, without a trailing `/`.
    origin: String,
    /// The page's directory: `/`, or a path ending in `/`.
    dir: String,
}

impl Base {
    /// `url`'s origin and directory, or `None` when it is not `http(s)://host…`.
    pub(super) fn of(url: &str) -> Option<Self> {
        let (scheme, rest) = url.split_once("://")?;
        if !matches!(scheme, "http" | "https") {
            return None;
        }
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let host = &rest[..end];
        if host.is_empty() {
            return None;
        }
        let path = &rest[end..];
        let path = &path[..path.find(['?', '#']).unwrap_or(path.len())];
        let dir = match path.rfind('/') {
            Some(i) => &path[..=i],
            None => "/",
        };
        Some(Self {
            origin: format!("{scheme}://{}", host.to_ascii_lowercase()),
            dir: dir.to_string(),
        })
    }

    /// The address `pin` names: an absolute path on the origin, or a name in the directory.
    pub(super) fn join(&self, pin: &str) -> String {
        if pin.starts_with('/') {
            format!("{}{pin}", self.origin)
        } else {
            format!("{}{}{pin}", self.origin, self.dir)
        }
    }

    /// A link as the pin would write it, or `None` when it leaves the origin or names nothing.
    ///
    /// A name under the page's directory is written relative to it, so a listing's own links
    /// read as they are listed. Anything else on the origin is its absolute path.
    fn pin_for(&self, href: &str) -> Option<String> {
        let href = href.trim().replace("&amp;", "&");
        let href = &href[..href.find('#').unwrap_or(href.len())];
        if href.is_empty() || href.starts_with('?') {
            return None;
        }
        let path = if let Some(rest) = href.strip_prefix("//") {
            self.on_origin(&format!("{}//{rest}", self.scheme()))?
        } else if names_scheme(href) {
            self.on_origin(href)?
        } else if href.starts_with('/') {
            href.to_string()
        } else {
            format!("{}{href}", self.dir)
        };
        let path = normalised(&path)?;
        match path.strip_prefix(&self.dir) {
            Some("") => None,
            Some(rest) => Some(rest.to_string()),
            None if path == "/" => None,
            None => Some(path),
        }
    }

    /// `https:` or `http:`, which a scheme-relative `//host/…` link takes.
    fn scheme(&self) -> &str {
        &self.origin[..self.origin.find("://").map_or(0, |i| i + 1)]
    }

    /// The path and query of an absolute `url` on this origin.
    fn on_origin(&self, url: &str) -> Option<String> {
        let (scheme, rest) = url.split_once("://")?;
        let end = rest.find(['/', '?']).unwrap_or(rest.len());
        // http and https of one host are one publisher; a pin is joined to the page's scheme.
        let host = self.origin.split_once("://").map_or("", |(_, h)| h);
        if !matches!(scheme, "http" | "https") || !rest[..end].eq_ignore_ascii_case(host) {
            return None;
        }
        let tail = &rest[end..];
        Some(if tail.starts_with('/') {
            tail.to_string()
        } else {
            format!("/{tail}")
        })
    }
}

/// Whether `href` starts `<scheme>:`, so is not a path.
fn names_scheme(href: &str) -> bool {
    let first = &href[..href.find(['/', '?']).unwrap_or(href.len())];
    first.contains(':')
}

/// `path` with `.` and `..` segments resolved, or `None` when `..` climbs above the root.
fn normalised(path: &str) -> Option<String> {
    let (path, query) = match path.find('?') {
        Some(i) => (&path[..i], &path[i..]),
        None => (path, ""),
    };
    let mut out: Vec<&str> = Vec::new();
    let segments: Vec<&str> = path.split('/').skip(1).collect();
    let last = segments.len().saturating_sub(1);
    for (i, seg) in segments.iter().enumerate() {
        match *seg {
            "." => {
                if i == last {
                    out.push("");
                }
            }
            ".." => {
                out.pop()?;
                if i == last {
                    out.push("");
                }
            }
            s => out.push(s),
        }
    }
    Some(format!("/{}{query}", out.join("/")))
}

/// Why a pin names nothing a fetch should follow, or `None` when it is safe.
///
/// A pin is joined to the declared host, so all it can do wrong is name something other than
/// one file there: another host, a parent directory, or a member inside the file.
pub fn unsafe_pin(pin: &str) -> Option<&'static str> {
    if pin.is_empty() {
        return Some("is empty");
    }
    if pin.contains('#') {
        return Some(
            "carries a `#`. A location names the bytes fetched, never a member inside them",
        );
    }
    if pin
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return Some("carries a space, a control character or a backslash");
    }
    if pin.starts_with("//") || names_scheme(pin) {
        return Some("names a host, and a pin only names a file on the pack's");
    }
    let path = &pin[..pin.find('?').unwrap_or(pin.len())];
    if path.split('/').any(|s| s == "." || s == "..") {
        return Some("carries a `.` or `..` segment");
    }
    None
}

/// `pattern` with `{id}` and each named group bound, every value escaped, so the regex's own
/// braces (`\d{8}`) are left as they are.
pub fn bind_match(pattern: &str, bindings: &[(String, String)]) -> String {
    let mut out = pattern.to_string();
    for (k, v) in bindings {
        out = out.replace(&format!("{{{k}}}"), &regex::escape(v));
    }
    out
}

/// Why reading the page picked nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unpicked {
    /// The catalog is not a DCAT-US `data.json`.
    NotCatalog(String),
    /// No dataset's identifier names the local id.
    NoDataset,
    /// Several do, and nothing says which.
    Datasets(Vec<String>),
    /// Nothing listed matches. `listed` is how many names were read, `elsewhere` how many
    /// were dropped for naming another host.
    NoMatch { listed: usize, elsewhere: usize },
    /// Several match, and the scheme picks `only`.
    Several(Vec<String>),
}

impl Unpicked {
    pub fn message(&self, l: &Lookup) -> String {
        let page = &l.url;
        match self {
            Self::NotCatalog(why) => format!("`{page}` is not a DCAT-US catalog: {why}"),
            Self::NoDataset => {
                let local = match &l.page {
                    Page::Dcat { local } => local.as_str(),
                    Page::Listing => "",
                };
                format!("no dataset in `{page}` has an identifier naming `{local}`")
            }
            Self::Datasets(ids) => format!(
                "{} datasets in `{page}` name it: {}",
                ids.len(),
                ids.join(", ")
            ),
            Self::NoMatch { listed, elsewhere } => {
                let re = l
                    .matches
                    .as_ref()
                    .map(|re| format!(" `{re}`"))
                    .unwrap_or_default();
                let media = match (&l.media, &l.page) {
                    (Some(m), Page::Dcat { .. }) => format!(" with media type `{m}`"),
                    _ => String::new(),
                };
                let off = if *elsewhere > 0 {
                    format!("; {elsewhere} more are on another host, which a pin cannot name")
                } else {
                    String::new()
                };
                format!("`{page}` lists {listed} names, and none matches{re}{media}{off}")
            }
            Self::Several(names) => format!(
                "`{page}` lists {} names that match, and [scheme.{}] picks `only`: {}",
                names.len(),
                l.scheme,
                names.join(", ")
            ),
        }
    }
}

impl Lookup {
    /// The pin `body` names: its candidates, matched, then picked.
    pub fn pick(&self, body: &[u8]) -> Result<String, Unpicked> {
        let text = String::from_utf8_lossy(body);
        let (listed, elsewhere) = match &self.page {
            Page::Listing => {
                let links = hrefs(&text);
                let total = links.len();
                let on: Vec<String> = links.iter().filter_map(|h| self.base.pin_for(h)).collect();
                let elsewhere = total - on.len();
                (on, elsewhere)
            }
            Page::Dcat { local } => self.distributions(&text, local)?,
        };
        let mut names: Vec<String> = Vec::new();
        for n in &listed {
            if !names.contains(n) {
                names.push(n.clone());
            }
        }
        let count = names.len();
        if let Some(re) = &self.matches {
            names.retain(|n| re.is_match(n));
        }
        names.retain(|n| unsafe_pin(n).is_none());
        match self.pick {
            Pick::Latest => names.into_iter().max_by(|a, b| natural(a, b)),
            Pick::Only if names.len() > 1 => return Err(Unpicked::Several(names)),
            Pick::Only => names.pop(),
        }
        .ok_or(Unpicked::NoMatch {
            listed: count,
            elsewhere,
        })
    }

    /// `scheme:local-id@<pin>`.
    pub fn pinned(&self, pin: &str) -> String {
        format!("{}@{pin}", self.identifier)
    }

    /// The `downloadURL` of each distribution of the one dataset naming `local`, as pins.
    fn distributions(&self, text: &str, local: &str) -> Result<(Vec<String>, usize), Unpicked> {
        let doc: serde_json::Value =
            serde_json::from_str(text).map_err(|e| Unpicked::NotCatalog(e.to_string()))?;
        let Some(datasets) = doc.get("dataset").and_then(|d| d.as_array()) else {
            return Err(Unpicked::NotCatalog("it has no `dataset` array".into()));
        };
        let named: Vec<&serde_json::Value> = datasets
            .iter()
            .filter(|d| {
                d.get("identifier")
                    .and_then(|i| i.as_str())
                    .is_some_and(|i| i == local || i.split('/').any(|s| s == local))
            })
            .collect();
        let dataset = match named.as_slice() {
            [] => return Err(Unpicked::NoDataset),
            [one] => *one,
            many => {
                return Err(Unpicked::Datasets(
                    many.iter()
                        .filter_map(|d| d.get("identifier").and_then(|i| i.as_str()))
                        .map(str::to_string)
                        .collect(),
                ))
            }
        };
        let urls: Vec<&str> = dataset
            .get("distribution")
            .and_then(|d| d.as_array())
            .into_iter()
            .flatten()
            .filter(|d| match &self.media {
                None => true,
                Some(want) => d
                    .get("mediaType")
                    .and_then(|m| m.as_str())
                    .is_some_and(|m| media_eq(m, want)),
            })
            .filter_map(|d| d.get("downloadURL").and_then(|u| u.as_str()))
            .collect();
        let pins: Vec<String> = urls.iter().filter_map(|u| self.base.pin_for(u)).collect();
        let elsewhere = urls.len() - pins.len();
        Ok((pins, elsewhere))
    }
}

/// Media types equal up to case and parameters: `text/csv; charset=utf-8` is `text/csv`.
fn media_eq(a: &str, b: &str) -> bool {
    let bare = |m: &str| {
        m.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
    };
    bare(a) == bare(b)
}

/// The value of every `href` attribute in `html`, in order.
fn hrefs(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find("href") {
        let at = from + i + 4;
        from = at;
        // `href` as an attribute: preceded by space, followed by `=`.
        let before = lower[..at - 4].chars().next_back();
        if !before.is_some_and(char::is_whitespace) {
            continue;
        }
        let rest = html[at..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or(""),
            Some(_) => rest
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()
                .unwrap_or(""),
            None => "",
        };
        out.push(value.to_string());
    }
    out
}

/// Names in natural order: runs of digits compare as numbers, so `v10` follows `v9`, and a
/// release date compares as a date.
pub fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        match (a.is_empty(), b.is_empty()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
        let (ra, ta) = run(a);
        let (rb, tb) = run(b);
        let digits = |s: &str| s.starts_with(|c: char| c.is_ascii_digit());
        let ord = if digits(ra) && digits(rb) {
            let (za, zb) = (ra.trim_start_matches('0'), rb.trim_start_matches('0'));
            za.len()
                .cmp(&zb.len())
                .then_with(|| za.cmp(zb))
                .then_with(|| ra.len().cmp(&rb.len()))
        } else {
            ra.cmp(rb)
        };
        if ord != Ordering::Equal {
            return ord;
        }
        a = ta;
        b = tb;
    }
}

/// The leading run of digits, or of non-digits, and what follows it.
fn run(s: &str) -> (&str, &str) {
    let digit = s.starts_with(|c: char| c.is_ascii_digit());
    let end = s
        .find(|c: char| c.is_ascii_digit() != digit)
        .unwrap_or(s.len());
    s.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(url: &str) -> Base {
        Base::of(url).unwrap()
    }

    #[test]
    fn a_link_is_pinned_relative_to_the_listing_or_by_its_path() {
        let b = base("https://www.ncei.noaa.gov/pub/data/cirs/climdiv/");
        assert_eq!(
            b.pin_for("climdiv-pcpndv-v1.0.0-20261001").as_deref(),
            Some("climdiv-pcpndv-v1.0.0-20261001")
        );
        assert_eq!(
            b.pin_for("/pub/data/cirs/climdiv/a.txt").as_deref(),
            Some("a.txt")
        );
        assert_eq!(
            b.pin_for("https://WWW.ncei.noaa.gov/pub/other/b.txt")
                .as_deref(),
            Some("/pub/other/b.txt")
        );
        assert_eq!(b.pin_for("../").as_deref(), Some("/pub/data/cirs/"));
        assert_eq!(b.pin_for("https://example.org/c.txt"), None);
        assert_eq!(b.pin_for("//example.org/c.txt"), None);
        assert_eq!(b.pin_for("mailto:x@y.org"), None);
        assert_eq!(b.pin_for("?C=M;O=A"), None);
        assert_eq!(b.pin_for("#top"), None);
        assert_eq!(b.pin_for("a.zip#member.txt").as_deref(), Some("a.zip"));
    }

    #[test]
    fn a_pin_joins_to_the_declared_host_only() {
        let b = base("https://www2.census.gov/programs-surveys/saipe/datasets/index.html?x=1");
        assert_eq!(
            b.join("2022/"),
            "https://www2.census.gov/programs-surveys/saipe/datasets/2022/"
        );
        assert_eq!(b.join("/x/y.csv"), "https://www2.census.gov/x/y.csv");
    }

    #[test]
    fn an_unsafe_pin_says_why() {
        for pin in [
            "",
            "a.zip#packinglist.txt",
            "a b",
            "//evil.org/x",
            "https://evil.org/x",
            "../secret",
            "a/./b",
            "a\\b",
        ] {
            assert!(unsafe_pin(pin).is_some(), "{pin:?}");
        }
        for pin in ["a.csv", "/data/2026/a.csv?format=csv", "2022/", "a..b.csv"] {
            assert_eq!(unsafe_pin(pin), None, "{pin:?}");
        }
        assert!(unsafe_pin("a.zip#m").unwrap().contains("member"));
    }

    #[test]
    fn a_match_binds_escaped_values_and_keeps_its_quantifiers() {
        let bound = bind_match(
            r"^climdiv-{id}-v1\.0\.0-\d{8}$",
            &[("id".into(), "a.b".into())],
        );
        assert_eq!(bound, r"^climdiv-a\.b-v1\.0\.0-\d{8}$");
    }

    #[test]
    fn hrefs_are_read_in_every_quoting() {
        let html = r#"<a href="a.txt">a</a> <A HREF='b.txt'> <a href=c.txt>c</a>
                      <link rel=x href = "d&amp;e"> <a data-href="no">"#;
        assert_eq!(hrefs(html), vec!["a.txt", "b.txt", "c.txt", "d&amp;e"]);
    }

    #[test]
    fn latest_compares_digit_runs_as_numbers() {
        let mut v = vec!["v10", "v9", "v09a", "a-20260901", "a-20261001"];
        v.sort_by(|a, b| natural(a, b));
        assert_eq!(v, vec!["a-20260901", "a-20261001", "v9", "v09a", "v10"]);
    }

    #[test]
    fn media_ignores_case_and_parameters() {
        assert!(media_eq("Text/CSV; charset=utf-8", "text/csv"));
        assert!(!media_eq("application/json", "text/csv"));
    }

    #[test]
    fn a_dot_segment_is_resolved_and_one_above_the_root_is_nothing() {
        assert_eq!(normalised("/a/b/../c?x=1").as_deref(), Some("/a/c?x=1"));
        assert_eq!(normalised("/a/./").as_deref(), Some("/a/"));
        assert_eq!(normalised("/../a"), None);
    }
}
