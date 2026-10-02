//! Reading a paged service to its end, and refusing one page of it as the whole (#1341).
//!
//! A paged service answers a single `GET` with its first page and HTTP 200. ArcGIS REST stops at
//! the layer's `maxRecordCount` and says `exceededTransferLimit: true`; Socrata stops at its
//! default `$limit` and says nothing. Recorded as it arrives, the page's digest would stand for
//! the dataset.
//!
//! A scheme that declares `paginate` is read page by page, and the pages make one document. A
//! response that says it was cut short, from an address nothing declares paging for, is refused.
//!
//! Pure, like [`super::location`]: it takes the bytes each page answered and says what to ask
//! next. The requests are [`super::fetch`]'s.

use std::path::Path;

use serde_json::Value;

use crate::sources::manifest::Paginate;

/// The key ArcGIS REST sets on a page its record cap cut short.
pub(crate) const TRUNCATED: &str = "exceededTransferLimit";

/// The most pages one address is read in. A service that never says it is done would otherwise
/// be asked forever.
pub(crate) const MAX_PAGES: usize = 10_000;

/// Whether the reader asks another page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    More,
    Done,
}

/// The pages of one address, as they arrive.
pub(crate) struct Pager<'a> {
    p: &'a Paginate,
    base: &'a str,
    /// Records received so far, which is the next page's offset.
    received: u64,
    pages: usize,
    /// The first page's bytes, kept while it is the only page.
    first: Vec<u8>,
    /// The first page's document, with every page's records at `items`.
    doc: Option<Value>,
}

impl<'a> Pager<'a> {
    pub(crate) fn new(p: &'a Paginate, base: &'a str) -> Self {
        Self {
            p,
            base,
            received: 0,
            pages: 0,
            first: Vec::new(),
            doc: None,
        }
    }

    /// How many pages have arrived.
    pub(crate) fn pages(&self) -> usize {
        self.pages
    }

    /// The address of the next page: the base with its offset, and its size when one is
    /// declared.
    pub(crate) fn url(&self) -> String {
        let mut url = set_param(self.base, &self.p.offset, &self.received.to_string());
        if let (Some(limit), Some(size)) = (&self.p.limit, self.p.size) {
            url = set_param(&url, limit, &size.to_string());
        }
        url
    }

    /// Take the page the last [`Self::url`] answered, and say whether another follows.
    pub(crate) fn take(&mut self, bytes: Vec<u8>) -> Result<Step, String> {
        let n = self.pages + 1;
        let mut page: Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("page {n} is not JSON, and only JSON is paged: {e}"))?;
        let at = array_path(&self.p.items).ok_or_else(|| {
            format!(
                "`items = \"{}\"` does not end in `*`; `yidam source check` reports the pack",
                self.p.items
            )
        })?;
        let records = match at_mut(&mut page, &at) {
            Some(Value::Array(a)) => std::mem::take(a),
            _ => {
                return Err(format!(
                    "page {n} holds no array at `{}`",
                    self.p.items.trim_end_matches('*').trim_end_matches('/')
                ))
            }
        };
        let count = records.len();
        let done = match &self.p.until {
            Some(until) => {
                let (negated, path) = match until.strip_prefix('!') {
                    Some(p) => (true, p),
                    None => (false, until.as_str()),
                };
                let set = matches!(at_mut(&mut page, &segments(path)), Some(Value::Bool(true)));
                set != negated
            }
            None => (count as u64) < u64::from(self.p.size.unwrap_or(u32::MAX)),
        };
        if !done && count == 0 {
            return Err(format!(
                "page {n} holds no records and does not say it is the last, so the next page \
                 would ask the same offset again"
            ));
        }

        match &mut self.doc {
            None => {
                let mut doc = page;
                if let Some(Value::Array(a)) = at_mut(&mut doc, &at) {
                    *a = records;
                }
                self.doc = Some(doc);
                self.first = bytes;
            }
            Some(doc) => {
                if let Some(Value::Array(a)) = at_mut(doc, &at) {
                    a.extend(records);
                }
                // The flag that ends the paging says what the last page said, which is that
                // nothing was left out.
                if let Some(until) = &self.p.until {
                    let path = segments(until.trim_start_matches('!'));
                    let last = at_mut(&mut page, &path).map(std::mem::take);
                    set_at(doc, &path, last);
                }
                self.first = Vec::new();
            }
        }
        self.pages = n;
        self.received += count as u64;
        if !done && n >= MAX_PAGES {
            return Err(format!(
                "{MAX_PAGES} pages arrived and the last did not say it was the last"
            ));
        }
        Ok(if done { Step::Done } else { Step::More })
    }

    /// What is recorded: the page as it arrived when there was one, and otherwise the first
    /// page's document holding every page's records.
    pub(crate) fn into_bytes(self) -> Result<Vec<u8>, String> {
        match self.doc {
            Some(_) if self.pages == 1 => Ok(self.first),
            Some(doc) => serde_json::to_vec(&doc).map_err(|e| e.to_string()),
            None => Err("no page arrived".into()),
        }
    }
}

/// Whether the file at `path` is a JSON object saying it was cut short:
/// `"exceededTransferLimit": true` at its top level.
///
/// Only a file whose first character is `{` and that names the key is parsed, so a CSV, a PDF or
/// a JSON document that says nothing about its length costs a peek.
pub(crate) fn truncated(path: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut head = [0u8; 512];
    let n = std::fs::File::open(path)?.read(&mut head)?;
    if head[..n].iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
        return Ok(false);
    }
    let bytes = std::fs::read(path)?;
    let key = format!("\"{TRUNCATED}\"");
    if !bytes.windows(key.len()).any(|w| w == key.as_bytes()) {
        return Ok(false);
    }
    Ok(serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|v| v.get(TRUNCATED).cloned())
        == Some(Value::Bool(true)))
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// The path of the array `items` names: `features/*` is `features`, and `*` the document.
/// `None` when `items` does not end in `*`, or has one anywhere else.
pub(crate) fn array_path(items: &str) -> Option<Vec<&str>> {
    let mut s = segments(items);
    (s.pop() == Some("*") && !s.contains(&"*")).then_some(s)
}

fn at_mut<'v>(v: &'v mut Value, path: &[&str]) -> Option<&'v mut Value> {
    path.iter().try_fold(v, |v, seg| match v {
        Value::Object(m) => m.get_mut(*seg),
        Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
        _ => None,
    })
}

/// Set the value at `path`, or remove it with `None`. A parent that is not there is left so.
fn set_at(v: &mut Value, path: &[&str], to: Option<Value>) {
    let Some((last, parent)) = path.split_last() else {
        return;
    };
    let Some(Value::Object(m)) = at_mut(v, parent) else {
        return;
    };
    match to {
        Some(to) => {
            m.insert((*last).to_string(), to);
        }
        None => {
            m.remove(*last);
        }
    }
}

/// `url` with `name=value` in its query: the pair's value replaced where the query has one, and
/// the pair appended where it does not.
pub(crate) fn set_param(url: &str, name: &str, value: &str) -> String {
    let (rest, fragment) = match url.split_once('#') {
        Some((r, f)) => (r, Some(f)),
        None => (url, None),
    };
    let (path, query) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };
    let mut pairs: Vec<String> = query
        .map(|q| {
            q.split('&')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let pair = format!("{name}={value}");
    match pairs.iter_mut().find(|p| p.split('=').next() == Some(name)) {
        Some(p) => *p = pair,
        None => pairs.push(pair),
    }
    let mut out = format!("{path}?{}", pairs.join("&"));
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arcgis() -> Paginate {
        Paginate {
            offset: "resultOffset".into(),
            limit: None,
            size: None,
            items: "features/*".into(),
            until: Some("!exceededTransferLimit".into()),
        }
    }

    fn socrata(size: u32) -> Paginate {
        Paginate {
            offset: "$offset".into(),
            limit: Some("$limit".into()),
            size: Some(size),
            items: "*".into(),
            until: None,
        }
    }

    fn page(features: &[u32], more: bool) -> Vec<u8> {
        let mut v = serde_json::json!({
            "geometryType": "esriGeometryPoint",
            "features": features.iter().map(|i| serde_json::json!({"attributes": {"id": i}})).collect::<Vec<_>>(),
        });
        if more {
            v["exceededTransferLimit"] = Value::Bool(true);
        }
        serde_json::to_vec(&v).unwrap()
    }

    #[test]
    fn a_param_is_replaced_where_the_query_has_it_and_appended_where_it_does_not() {
        assert_eq!(
            set_param("https://x/q?where=1%3D1&f=json", "resultOffset", "0"),
            "https://x/q?where=1%3D1&f=json&resultOffset=0"
        );
        assert_eq!(
            set_param("https://x/r.json?$limit=5000&a=b", "$limit", "1000"),
            "https://x/r.json?$limit=1000&a=b"
        );
        assert_eq!(set_param("https://x/r#t", "o", "2"), "https://x/r?o=2#t");
        assert_eq!(
            set_param("https://x/r?offsets=1", "offset", "2"),
            "https://x/r?offsets=1&offset=2",
            "a parameter whose name begins with another's is not that one"
        );
    }

    /// The acceptance case's shape: two ArcGIS pages make one document holding both pages'
    /// features, and it says nothing was left out.
    #[test]
    fn two_pages_make_one_document_that_is_not_truncated() {
        let p = arcgis();
        let mut pager = Pager::new(&p, "https://x/FeatureServer/0/query?where=1%3D1&f=json");
        assert!(pager.url().ends_with("&resultOffset=0"), "{}", pager.url());
        assert_eq!(pager.take(page(&[1, 2], true)), Ok(Step::More));
        assert!(pager.url().ends_with("&resultOffset=2"), "{}", pager.url());
        assert_eq!(pager.take(page(&[3], false)), Ok(Step::Done));
        assert_eq!(pager.pages(), 2);

        let doc: Value = serde_json::from_slice(&pager.into_bytes().unwrap()).unwrap();
        let ids: Vec<u64> = doc["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["attributes"]["id"].as_u64().unwrap())
            .collect();
        assert_eq!(ids, [1, 2, 3]);
        assert_eq!(doc["geometryType"], "esriGeometryPoint");
        assert!(doc.get(TRUNCATED).is_none(), "{doc}");
    }

    /// One page that is the whole answer is recorded as the publisher sent it, byte for byte.
    #[test]
    fn a_single_page_is_kept_as_it_arrived() {
        let p = arcgis();
        let mut pager = Pager::new(&p, "https://x/q");
        let bytes = b"{ \"features\" : [ {\"a\":1} ] }".to_vec();
        assert_eq!(pager.take(bytes.clone()), Ok(Step::Done));
        assert_eq!(pager.into_bytes().unwrap(), bytes);
    }

    /// Socrata's form says nothing about its length, so a short page is the last. The offset
    /// follows what arrived, and each page asks for `size`.
    #[test]
    fn without_until_a_short_page_is_the_last() {
        let p = socrata(2);
        let mut pager = Pager::new(&p, "https://data.cdc.gov/resource/abcd-1234.json");
        assert_eq!(
            pager.url(),
            "https://data.cdc.gov/resource/abcd-1234.json?$offset=0&$limit=2"
        );
        assert_eq!(
            pager.take(b"[{\"a\":1},{\"a\":2}]".to_vec()),
            Ok(Step::More)
        );
        assert_eq!(
            pager.url(),
            "https://data.cdc.gov/resource/abcd-1234.json?$offset=2&$limit=2"
        );
        assert_eq!(pager.take(b"[{\"a\":3}]".to_vec()), Ok(Step::Done));
        assert_eq!(
            pager.into_bytes().unwrap(),
            b"[{\"a\":1},{\"a\":2},{\"a\":3}]".to_vec()
        );
    }

    /// A full last page is followed by an empty one, which ends the paging rather than failing.
    #[test]
    fn an_empty_page_after_a_full_one_is_the_end() {
        let p = socrata(1);
        let mut pager = Pager::new(&p, "https://x/r.json");
        assert_eq!(pager.take(b"[1]".to_vec()), Ok(Step::More));
        assert_eq!(pager.take(b"[]".to_vec()), Ok(Step::Done));
        assert_eq!(pager.into_bytes().unwrap(), b"[1]".to_vec());
    }

    /// A page that says there is more and brings nothing would be asked again at the same
    /// offset, forever.
    #[test]
    fn a_page_with_nothing_in_it_that_says_there_is_more_is_refused() {
        let p = arcgis();
        let mut pager = Pager::new(&p, "https://x/q");
        let err = pager.take(page(&[], true)).unwrap_err();
        assert!(err.contains("same offset"), "{err}");
    }

    #[test]
    fn a_page_that_is_not_the_declared_shape_says_what_it_lacks() {
        let p = arcgis();
        let err = Pager::new(&p, "https://x/q")
            .take(b"a,b\n1,2\n".to_vec())
            .unwrap_err();
        assert!(err.contains("not JSON"), "{err}");
        let err = Pager::new(&p, "https://x/q")
            .take(b"{\"rows\":[]}".to_vec())
            .unwrap_err();
        assert!(err.contains("no array at `features`"), "{err}");
    }

    #[test]
    fn items_names_an_array_by_a_trailing_star() {
        assert_eq!(array_path("features/*"), Some(vec!["features"]));
        assert_eq!(array_path("*"), Some(vec![]));
        assert_eq!(array_path("features"), None);
        assert_eq!(array_path("*/rows/*"), None);
    }

    #[test]
    fn a_response_saying_it_was_cut_short_is_told_apart_from_one_that_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let at = |name: &str, body: &[u8]| {
            let p = dir.path().join(name);
            std::fs::write(&p, body).unwrap();
            truncated(&p).unwrap()
        };
        assert!(at("cut", &page(&[1], true)));
        assert!(!at("whole", &page(&[1], false)));
        assert!(!at(
            "false",
            b"{\"exceededTransferLimit\": false, \"features\": []}"
        ));
        assert!(
            !at("nested", b"{\"a\": {\"exceededTransferLimit\": true}}"),
            "only the top level is ArcGIS's"
        );
        assert!(!at("csv", b"exceededTransferLimit,true\n"));
    }
}
