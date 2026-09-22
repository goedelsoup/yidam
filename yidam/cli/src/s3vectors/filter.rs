//! A [`crate::retrieval::Filter`], rendered as an S3 Vectors metadata filter.
//!
//! # Why the corpus predicate is always here
//!
//! Every filter this module emits names a corpus, including the ones whose caller asked for no
//! filter at all. Two things ride on that:
//!
//! - The witness record ([`super::WITNESS_KEY`]) lives in the same index as the corpus's own
//!   vectors and would otherwise be a search result. Excluding it with a *positive* predicate
//!   on every real query is what avoids `$ne`, whose behaviour against a vector that lacks the
//!   key this code would be guessing at.
//! - An index holding more than one corpus is what a shared bucket is for. Writing the
//!   predicate from the first push meant that phase is a filter change rather than a re-push
//!   of every corpus, and #835 is that filter change: one corpus renders `$eq`, several render
//!   `$in`, and the witness fails both by construction.
//!
//! **There is no "every corpus in this index" filter, and that is a decision.** It would have
//! to be written `$nin: ["__yidam_meta"]`, and what S3 Vectors does with a negative predicate
//! against a vector that lacks the key is not documented — a wrong guess leaks an internal
//! record into somebody's search results. The set a caller names is a set this code can render
//! positively, and a caller who wants every corpus in an index can list them: RFC-0033's phase
//! list is unchanged about that, and enumerating the index is
//! [`super::ops::list_keys`]'s to answer, not a query's.
//!
//! # Filtering happens during the search, not after it
//!
//! AWS: *"S3 Vectors searches through candidate vectors in the index to find the top K similar
//! vectors while simultaneously validating if each candidate vector matches your metadata
//! filter conditions."* So a pushed class filter is not an optimisation of a local one — it
//! changes which vectors are candidates, and pushing it is what makes `k` count rows the
//! caller could actually use. The residual half of a filter, which cannot be pushed, is why
//! the read path over-fetches.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::{META_KEY_CLASS, META_KEY_CORPUS};
use crate::retrieval::{Corpora, Filter};

/// Render `filter`, scoped to the corpora being asked about.
///
/// A corpus is the short genesis hash — the one identity every corpus has and no two share.
pub fn to_json(corpora: &Corpora, filter: &Filter) -> Result<Value> {
    let ids = corpora.ids();
    if ids.iter().any(|c| c.is_empty()) {
        bail!("a remote query must name the corpus it is searching");
    }
    // One corpus is an equality and several are a set, which is the shape the class clause
    // below already uses. `$in` with one element would be equivalent and is not emitted: two
    // renderings of one predicate is two things a reader of a canonical request has to know.
    let corpus_clause = if ids.len() == 1 {
        json!({ META_KEY_CORPUS: { "$eq": ids[0] } })
    } else {
        json!({ META_KEY_CORPUS: { "$in": ids } })
    };

    let classes = match &filter.classes {
        None => return Ok(corpus_clause),
        Some(cs) => cs,
    };
    // `$in` and `$nin` take a *non-empty* array; an empty one is a `ValidationException`. It is
    // also a caller's bug rather than a query — a filter admitting no class can have no result
    // — so it is refused here, where the message can say that, rather than at the service.
    if classes.is_empty() {
        bail!("a class filter admitting no class cannot match anything — pass `Filter::any()` to search every class");
    }

    let class_clause = if classes.len() == 1 {
        json!({ META_KEY_CLASS: { "$eq": classes[0] } })
    } else {
        json!({ META_KEY_CLASS: { "$in": classes } })
    };
    Ok(json!({ "$and": [corpus_clause, class_clause] }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(classes: Option<&[&str]>) -> Filter {
        Filter {
            classes: classes.map(|cs| cs.iter().map(|c| c.to_string()).collect()),
            // Rendering is downstream of `Filter::as_applied`: by the time a filter reaches
            // this module a backend has already decided what it may push, so what a filter
            // was a claim *about* no longer changes a byte of the JSON.
            about_nodes: false,
        }
    }

    fn own() -> Corpora {
        Corpora::own("abc123")
    }

    fn across(also: &[&str]) -> Corpora {
        Corpora::across(
            "abc123",
            &also.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn no_class_filter_still_scopes_to_the_corpus() {
        let v = to_json(&own(), &f(None)).unwrap();
        assert_eq!(v, json!({"corpus": {"$eq": "abc123"}}));
    }

    #[test]
    fn one_class_is_an_equality_and_several_are_a_set() {
        assert_eq!(
            to_json(&own(), &f(Some(&["concept"]))).unwrap(),
            json!({"$and": [
                {"corpus": {"$eq": "abc123"}},
                {"class": {"$eq": "concept"}},
            ]})
        );
        assert_eq!(
            to_json(&own(), &f(Some(&["concept", "source"]))).unwrap(),
            json!({"$and": [
                {"corpus": {"$eq": "abc123"}},
                {"class": {"$in": ["concept", "source"]}},
            ]})
        );
    }

    /// The whole of RFC-0033 phase 3 on the wire: a set where there was a scalar, positive
    /// either way.
    #[test]
    fn several_corpora_are_a_set_and_the_asking_one_leads_it() {
        assert_eq!(
            to_json(&across(&["other9"]), &f(None)).unwrap(),
            json!({"corpus": {"$in": ["abc123", "other9"]}})
        );
        assert_eq!(
            to_json(&across(&["other9"]), &f(Some(&["concept"]))).unwrap(),
            json!({"$and": [
                {"corpus": {"$in": ["abc123", "other9"]}},
                {"class": {"$eq": "concept"}},
            ]})
        );
    }

    /// Naming your own corpus among the others changes nothing, so a caller pasting the
    /// index's whole roster does not send `$in ["x", "x"]`.
    #[test]
    fn naming_the_asking_corpus_again_is_not_a_second_element() {
        assert_eq!(
            to_json(&across(&["abc123", "other9", "other9"]), &f(None)).unwrap(),
            json!({"corpus": {"$in": ["abc123", "other9"]}})
        );
    }

    #[test]
    fn an_empty_class_set_is_refused_rather_than_sent_as_an_empty_in() {
        let e = to_json(&own(), &f(Some(&[]))).unwrap_err().to_string();
        assert!(e.contains("no class"), "{e}");
    }

    #[test]
    fn a_query_that_names_no_corpus_is_refused() {
        assert!(to_json(&Corpora::own(""), &f(None)).is_err());
        assert!(to_json(&Corpora::across("abc123", &[String::new()]), &f(None)).is_err());
    }

    // ── the equivalence guard ─────────────────────────────────────────────────

    /// An interpreter for the subset of the filter language this module emits.
    ///
    /// It exists so the two backends can be held to one reading of a [`Filter`]: the local scan
    /// tests `Filter::admits` in Rust, the remote one sends this JSON, and nothing else in the
    /// process compares them. Written as a real evaluator rather than a string match, because a
    /// string match would pass for a filter that means the opposite of what it says.
    fn evaluate(f: &Value, corpus: &str, class: &str) -> bool {
        if let Some(clauses) = f.get("$and").and_then(Value::as_array) {
            return clauses.iter().all(|c| evaluate(c, corpus, class));
        }
        let field = |name: &str, actual: &str| -> Option<bool> {
            let clause = f.get(name)?;
            if let Some(want) = clause.get("$eq").and_then(Value::as_str) {
                return Some(actual == want);
            }
            if let Some(set) = clause.get("$in").and_then(Value::as_array) {
                return Some(set.iter().any(|v| v.as_str() == Some(actual)));
            }
            panic!("unsupported operator in {clause}");
        };
        if let Some(v) = field(META_KEY_CORPUS, corpus) {
            return v;
        }
        if let Some(v) = field(META_KEY_CLASS, class) {
            return v;
        }
        panic!("filter names no field this evaluator knows: {f}");
    }

    /// The pushed filter and the local predicate admit the same rows.
    ///
    /// Over a matrix rather than a case, and including rows of a corpus that was **not** asked
    /// for — which `Filter` itself cannot express, because a local index is one corpus by
    /// construction. [`Corpora`] is where that half of the question lives now, and the two
    /// halves are checked together because a filter that admitted the right classes of the
    /// wrong corpus would pass either one alone.
    #[test]
    fn the_pushed_filter_and_the_local_predicate_admit_the_same_rows() {
        let rows = [
            ("abc123", "concept"),
            ("abc123", "source"),
            ("abc123", "question"),
            ("other9", "concept"),
            ("third4", "concept"),
            (super::super::META_CORPUS, "concept"),
        ];
        for corpora in [own(), across(&["other9"]), across(&["other9", "third4"])] {
            for filter in [
                f(None),
                f(Some(&["concept"])),
                f(Some(&["concept", "source"])),
            ] {
                let json = to_json(&corpora, &filter).unwrap();
                for (corpus, class) in rows {
                    let remote = evaluate(&json, corpus, class);
                    let local = corpora.admits(corpus) && filter.admits(class);
                    assert_eq!(
                        remote, local,
                        "filter {json} disagrees about ({corpus}, {class}): remote {remote}, local {local}"
                    );
                }
            }
        }
    }

    /// The witness is excluded by every filter, including the one that filters on nothing and
    /// the one that spans several corpora.
    ///
    /// This is the assertion that would go red if the corpus clause were ever dropped for the
    /// unfiltered case as an optimisation — which is exactly the shape of change someone makes
    /// while reading `retrieve` and not this file — or if a spanning filter were written as a
    /// negative predicate.
    #[test]
    fn the_witness_record_is_excluded_by_every_filter() {
        for corpora in [own(), across(&["other9"])] {
            for filter in [f(None), f(Some(&["concept"]))] {
                let json = to_json(&corpora, &filter).unwrap();
                assert!(
                    !evaluate(&json, super::super::META_CORPUS, "concept"),
                    "the witness would be returned by {json}"
                );
            }
        }
    }
}
