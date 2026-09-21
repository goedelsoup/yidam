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
//! - An index holding more than one corpus is the phase after this one. Writing the predicate
//!   now means that phase is a filter change rather than a re-push of every corpus.
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
use crate::retrieval::Filter;

/// Render `filter`, scoped to `corpus`.
///
/// `corpus` is the short genesis hash — the one identity every corpus has and no two share.
pub fn to_json(corpus: &str, filter: &Filter) -> Result<Value> {
    if corpus.is_empty() {
        bail!("a remote query must name the corpus it is searching");
    }
    let corpus_clause = json!({ META_KEY_CORPUS: { "$eq": corpus } });

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

    #[test]
    fn no_class_filter_still_scopes_to_the_corpus() {
        let v = to_json("abc123", &f(None)).unwrap();
        assert_eq!(v, json!({"corpus": {"$eq": "abc123"}}));
    }

    #[test]
    fn one_class_is_an_equality_and_several_are_a_set() {
        assert_eq!(
            to_json("abc123", &f(Some(&["concept"]))).unwrap(),
            json!({"$and": [
                {"corpus": {"$eq": "abc123"}},
                {"class": {"$eq": "concept"}},
            ]})
        );
        assert_eq!(
            to_json("abc123", &f(Some(&["concept", "source"]))).unwrap(),
            json!({"$and": [
                {"corpus": {"$eq": "abc123"}},
                {"class": {"$in": ["concept", "source"]}},
            ]})
        );
    }

    #[test]
    fn an_empty_class_set_is_refused_rather_than_sent_as_an_empty_in() {
        let e = to_json("abc123", &f(Some(&[]))).unwrap_err().to_string();
        assert!(e.contains("no class"), "{e}");
    }

    #[test]
    fn a_query_that_names_no_corpus_is_refused() {
        assert!(to_json("", &f(None)).is_err());
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
    /// Over a matrix rather than a case, and including rows of the *wrong corpus* — which
    /// `Filter` itself cannot express, because a local index is one corpus by construction. The
    /// remote one is not, and the corpus predicate is the whole of that difference.
    #[test]
    fn the_pushed_filter_and_the_local_predicate_admit_the_same_rows() {
        let ours = "abc123";
        let rows = [
            (ours, "concept"),
            (ours, "source"),
            (ours, "question"),
            (super::super::META_CORPUS, "concept"),
            ("other9", "concept"),
        ];
        for filter in [
            f(None),
            f(Some(&["concept"])),
            f(Some(&["concept", "source"])),
        ] {
            let json = to_json(ours, &filter).unwrap();
            for (corpus, class) in rows {
                let remote = evaluate(&json, corpus, class);
                let local = corpus == ours && filter.admits(class);
                assert_eq!(
                    remote, local,
                    "filter {json} disagrees about ({corpus}, {class}): remote {remote}, local {local}"
                );
            }
        }
    }

    /// The witness is excluded by every filter, including the one that filters on nothing.
    ///
    /// This is the assertion that would go red if the corpus clause were ever dropped for the
    /// unfiltered case as an optimisation — which is exactly the shape of change someone makes
    /// while reading `retrieve` and not this file.
    #[test]
    fn the_witness_record_is_excluded_by_every_filter() {
        for filter in [f(None), f(Some(&["concept"]))] {
            let json = to_json("abc123", &filter).unwrap();
            assert!(
                !evaluate(&json, super::super::META_CORPUS, "concept"),
                "the witness would be returned by {json}"
            );
        }
    }
}
