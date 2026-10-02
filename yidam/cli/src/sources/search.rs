//! A pack's `[search]`: what its endpoint answered, read as candidate identifiers (#1316).
//!
//! Read by path and not by a transform, so `source search` works in every build. A transform
//! needs the gluon engine, and asking a publisher what it holds under a name is the first thing
//! a corpus does with a pack, before anyone has decided to build with `source-transforms`.
//!
//! The paths are the ones a transform sees in [`crate::gluon_arm::transform::Parsed`]: names
//! from the root down joined by `/`, a JSON array's items named by index, an XML attribute as
//! `@name` and its text as `#text`. JSON is read here in every build, by the same walk
//! [`super::parsed`] makes. XML and CSV are read by that module, which needs the feature, and a
//! build without it says so by name.

use serde::Serialize;

use super::manifest::Search;

/// One result the endpoint answered with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// `scheme:local-id`, the value `source add` takes.
    pub identifier: String,
    /// The result's title as the publisher wrote it. `None` when the pack names no `title`
    /// path, or this result has nothing there.
    pub title: Option<String>,
}

/// Every candidate in `bytes`, in the order the endpoint answered.
///
/// A result with no value at the `id` path is passed over. That is a publisher listing
/// something it holds no identifier for, which is not a failure of the search.
pub fn candidates(search: &Search, bytes: &[u8]) -> Result<Vec<Candidate>, String> {
    let leaves = flatten(&search.media, bytes)?;
    let items: Vec<&str> = split(&search.items);
    let mut out = Vec::new();
    for leaf in &leaves {
        if !matches(&items, &leaf.path) {
            continue;
        }
        let below = |rel: &str| -> Option<String> {
            let want = format!("{}/{rel}", leaf.path);
            leaves
                .iter()
                .find(|l| l.path == want)
                .and_then(|l| l.text.clone())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        };
        let Some(id) = below(&search.id) else {
            continue;
        };
        out.push(Candidate {
            identifier: format!("{}:{id}", search.scheme),
            title: search.title.as_deref().and_then(below),
        });
    }
    Ok(out)
}

/// One field of a parsed response.
#[derive(Debug, Clone, PartialEq)]
struct Leaf {
    path: String,
    /// The field's value as text. `None` for a container.
    text: Option<String>,
}

fn split(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Whether `path` is `pattern`, with `*` standing for any one segment.
fn matches(pattern: &[&str], path: &str) -> bool {
    let segments = split(path);
    segments.len() == pattern.len()
        && pattern
            .iter()
            .zip(&segments)
            .all(|(p, s)| *p == "*" || p == s)
}

/// Whether a response in `media` is read in every build: JSON, by the names
/// [`super::parsed`] gives it.
pub fn is_json(media: &str) -> bool {
    let essence = media
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(essence.as_str(), "application/json" | "text/json") || essence.ends_with("+json")
}

fn flatten(media: &str, bytes: &[u8]) -> Result<Vec<Leaf>, String> {
    if is_json(media) {
        let v: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|e| format!("the search response is not JSON: {e}"))?;
        let mut out = Vec::new();
        json("", &v, &mut out);
        return Ok(out);
    }
    other(media, bytes)
}

/// Walk a JSON value the way [`super::parsed`] does: the document is not a field, so its
/// members are the roots.
fn json(path: &str, v: &serde_json::Value, out: &mut Vec<Leaf>) {
    use serde_json::Value as J;
    let child = |name: &str| {
        if path.is_empty() {
            name.to_string()
        } else {
            format!("{path}/{name}")
        }
    };
    let text = match v {
        J::Null => None,
        J::Bool(b) => Some(b.to_string()),
        J::Number(n) => Some(n.to_string()),
        J::String(s) => Some(s.clone()),
        J::Array(_) | J::Object(_) => None,
    };
    if !path.is_empty() {
        out.push(Leaf {
            path: path.to_string(),
            text,
        });
    }
    match v {
        J::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                json(&child(&i.to_string()), item, out);
            }
        }
        J::Object(map) => {
            for (k, item) in map {
                json(&child(k), item, out);
            }
        }
        _ => {}
    }
}

#[cfg(feature = "source-transforms")]
fn other(media: &str, bytes: &[u8]) -> Result<Vec<Leaf>, String> {
    use crate::gluon_arm::marshal::Value;
    let parsed =
        super::parsed::parse(media, bytes).map_err(|e| format!("the search response {e}"))?;
    Ok(parsed
        .fields
        .into_iter()
        .map(|f| Leaf {
            path: f.path,
            text: match f.value {
                Value::Text(s) => Some(s),
                Value::Int(i) => Some(i.to_string()),
                Value::Number(n) => Some(n.to_string()),
                Value::Flag(b) => Some(b.to_string()),
                Value::Empty | Value::Unrepresentable => None,
            },
        })
        .collect())
}

#[cfg(not(feature = "source-transforms"))]
fn other(media: &str, _: &[u8]) -> Result<Vec<Leaf>, String> {
    Err(format!(
        "a search answering in `{media}` is read in a build with `source-transforms`; this \
         build reads JSON"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(items: &str, id: &str, title: Option<&str>) -> Search {
        Search {
            scheme: "doi".into(),
            template: "https://e.org/?q={query}".into(),
            media: "application/json".into(),
            items: items.into(),
            id: id.into(),
            title: title.map(str::to_string),
            fixtures: Default::default(),
        }
    }

    const CROSSREF: &str = r#"{"message": {"items": [
        {"DOI": "10.1167/tvst.8.5.14", "title": ["Retinal Imaging"]},
        {"title": ["No identifier"]},
        {"DOI": " 10.1/b ", "title": []}
    ]}}"#;

    #[test]
    fn each_item_with_an_id_is_a_candidate_in_the_order_answered() {
        let got = candidates(
            &search("message/items/*", "DOI", Some("title/0")),
            CROSSREF.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            got,
            vec![
                Candidate {
                    identifier: "doi:10.1167/tvst.8.5.14".into(),
                    title: Some("Retinal Imaging".into()),
                },
                Candidate {
                    identifier: "doi:10.1/b".into(),
                    title: None,
                },
            ]
        );
    }

    #[test]
    fn a_path_that_matches_nothing_is_no_candidates_and_not_an_error() {
        let got = candidates(&search("message/hits/*", "DOI", None), CROSSREF.as_bytes());
        assert_eq!(got, Ok(vec![]));
    }

    #[test]
    fn a_response_that_is_not_its_media_type_says_so() {
        let err = candidates(&search("a/*", "b", None), b"<xml/>").unwrap_err();
        assert!(err.contains("not JSON"), "{err}");
    }

    /// The paths a pack author writes are the paths a transform sees, so a pack has one
    /// vocabulary for both. Held against the transform's own walk where the build has it.
    #[cfg(feature = "source-transforms")]
    #[test]
    fn json_paths_are_the_ones_a_transform_reads() {
        let mine: Vec<String> = flatten("application/json", CROSSREF.as_bytes())
            .unwrap()
            .into_iter()
            .map(|l| l.path)
            .collect();
        let theirs: Vec<String> =
            super::super::parsed::parse("application/json", CROSSREF.as_bytes())
                .unwrap()
                .fields
                .into_iter()
                .map(|f| f.path)
                .collect();
        assert_eq!(mine, theirs);
    }
}
