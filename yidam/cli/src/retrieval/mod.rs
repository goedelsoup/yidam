//! How text becomes entry nodes, and — when it becomes them badly — why.
//!
//! This lived inside `cmd/serve` until #263, which is where it was written and not where it
//! belongs: `yidam query`'s similarity anchor enters the graph the same way `retrieve` does,
//! and two copies of "is retrieval degraded, and why" is two answers to one question. The
//! reason strings are a frozen contract (`prelude/sdks/parity/mcp/tools.json`) that a client
//! branches on, so the copy that drifted would drift silently, in whichever surface was read
//! less often.
//!
//! # What the `index` feature gates
//!
//! One path's *quality*, not any command. [`vector`] is the only module that names
//! `fastembed`; everything here compiles in the default build. Without it both `retrieve`
//! and an anchored query fall through to [`keyword_score`] and say so.

#[cfg(feature = "index")]
pub(crate) mod vector;

use anyhow::Result;

use crate::model::DomainModel;

/// How text will be resolved to nodes, and — when it will be resolved badly — why.
///
/// Three states rather than a bare `Option`, because "this corpus has no index" and "this
/// binary cannot read the index this corpus has" are different diagnoses with different
/// repairs, and a lone `degraded: true` collapses them into one. The first is fixed by
/// `yidam embed && yidam index-build`; the second by reinstalling with `--features index`.
/// A client told only that retrieval was degraded cannot tell which it is looking at.
pub(crate) enum Retrieval {
    /// Semantic search, over a loaded index.
    ///
    /// Boxed: it carries the decoded rows and a lazily-loaded embedder, and an unboxed
    /// variant would make every `Retrieval` — including the two empty ones the light build
    /// uses exclusively — as large as the heaviest.
    #[cfg(feature = "index")]
    Vector(Box<vector::IndexState>),
    /// Keyword search: the corpus has no vector index.
    NoIndex,
    /// Keyword search: the corpus *has* an index and this build cannot read it.
    ///
    /// Compiled only into the build that can actually be in this state. A binary carrying
    /// `index` reads any index it finds or fails to start, so the variant would be
    /// unreachable there — and an unreachable state that still appears in a match is one a
    /// reader has to rule out by hand every time.
    #[cfg(not(feature = "index"))]
    NoVectorSupport,
}

impl Retrieval {
    /// The machine-readable reason retrieval is degraded, or `None` when it is not.
    ///
    /// Stable strings, not prose: a client branches on these, and the MCP banner, the
    /// capability block, every `retrieve` call and every anchored query are all rendered
    /// from this one source so they cannot disagree.
    pub(crate) fn degraded_reason(&self) -> Option<&'static str> {
        match self {
            #[cfg(feature = "index")]
            Retrieval::Vector(_) => None,
            Retrieval::NoIndex => Some("no_index"),
            #[cfg(not(feature = "index"))]
            Retrieval::NoVectorSupport => Some("no_vector_support"),
        }
    }

    /// What to do about it, in one clause. Present tense, no leading capital — callers
    /// splice it into a sentence of their own.
    pub(crate) fn repair(&self) -> Option<&'static str> {
        match self {
            #[cfg(feature = "index")]
            Retrieval::Vector(_) => None,
            Retrieval::NoIndex => Some("run `yidam embed && yidam index-build` to build one"),
            #[cfg(not(feature = "index"))]
            Retrieval::NoVectorSupport => {
                Some("reinstall with `--features index` to read the index this corpus has")
            }
        }
    }
}

/// Decide how text will be resolved, and read the indexed commit either way.
///
/// Two bodies, one signature. The split is what lets the light build compile: decoding
/// `index/corpus.arrow` needs `arrow-ipc` and embedding a query needs `fastembed`, and
/// neither is in the default dependency set. What *is* in it is the raw `index/meta.json`
/// that `load_domain_model` already read — enough to know an index exists and which commit
/// it was built at, which is exactly the two facts a degraded caller should still report.
#[cfg(feature = "index")]
pub(crate) fn load(model: &DomainModel) -> Result<(Retrieval, Option<String>)> {
    use crate::embed_config::EmbedConfig;
    use crate::model::index_rows;

    match &model.index {
        Some(idx) => {
            let rows = index_rows(idx)?;
            // The reproducibility contract is authoritative for the model;
            // fall back to meta.json for indexes built before it existed.
            let model_id = idx
                .embed_config
                .as_ref()
                .map(|c: &EmbedConfig| c.model_id.clone())
                .or_else(|| idx.meta["model_name"].as_str().map(str::to_string))
                .unwrap_or_default();
            Ok((
                Retrieval::Vector(Box::new(vector::IndexState {
                    rows,
                    model_id,
                    embedder: std::cell::RefCell::new(None),
                })),
                indexed_commit(idx),
            ))
        }
        None => Ok((Retrieval::NoIndex, None)),
    }
}

#[cfg(not(feature = "index"))]
pub(crate) fn load(model: &DomainModel) -> Result<(Retrieval, Option<String>)> {
    match &model.index {
        // An index is on disk and this build cannot read it. Not `NoIndex`: the repair is
        // a different one, and telling a user to run `index-build` against an index they
        // already have is the kind of advice that costs an afternoon.
        Some(idx) => Ok((Retrieval::NoVectorSupport, indexed_commit(idx))),
        None => Ok((Retrieval::NoIndex, None)),
    }
}

fn indexed_commit(idx: &crate::model::IndexData) -> Option<String> {
    idx.meta["indexed_commit"].as_str().map(str::to_string)
}

/// A query's terms, lowercased. Whitespace-split — no stemming, no stop list.
pub(crate) fn terms(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// The fraction of `terms` present in `haystack`, or `None` when none are.
///
/// One scorer, shared by `retrieve`'s fallback and an anchored query's. They were the same
/// three lines written twice, and the second copy is the one that would have quietly stopped
/// matching the first — which matters more here than it looks, because `bench` compares an
/// anchored arm against a flat one and a scoring difference between them would be read as a
/// result.
///
/// `haystack` is expected already lowercased; the caller builds it once per node and this is
/// called once per node, so lowercasing here would be the same work in a worse place.
pub(crate) fn keyword_score(terms: &[String], haystack: &str) -> Option<f32> {
    if terms.is_empty() {
        return None;
    }
    let hits = terms
        .iter()
        .filter(|t| haystack.contains(t.as_str()))
        .count();
    match hits {
        0 => None,
        n => Some(n as f32 / terms.len() as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The strings are a contract (`prelude/sdks/parity/mcp/tools.json`), not a diagnostic —
    /// a client branches on them. Pinning them here means a rename has to be a deliberate
    /// act that also touches the freeze.
    #[test]
    #[cfg(not(feature = "index"))]
    fn the_two_degraded_reasons_are_distinct_and_stable() {
        assert_eq!(Retrieval::NoIndex.degraded_reason(), Some("no_index"));
        assert_eq!(
            Retrieval::NoVectorSupport.degraded_reason(),
            Some("no_vector_support")
        );
    }

    /// Every degraded state owes a repair, or the reason string is a diagnosis with no
    /// treatment — and the two states exist precisely because their treatments differ.
    #[test]
    fn a_reason_and_a_repair_are_present_together_or_not_at_all() {
        #[cfg(not(feature = "index"))]
        let states = [Retrieval::NoIndex, Retrieval::NoVectorSupport];
        #[cfg(feature = "index")]
        let states = [Retrieval::NoIndex];
        for state in states {
            assert_eq!(state.degraded_reason().is_some(), state.repair().is_some());
        }
    }

    #[test]
    fn a_score_is_the_fraction_of_terms_hit() {
        let t = terms("Knowledge Graph");
        assert_eq!(
            keyword_score(&t, "a knowledge graph of typed nodes"),
            Some(1.0)
        );
        assert_eq!(keyword_score(&t, "a graph of typed nodes"), Some(0.5));
        assert_eq!(keyword_score(&t, "nothing in common"), None);
    }

    /// An empty query matches nothing rather than everything. `retrieve` relied on this
    /// through a guard of its own; folding the guard into the scorer is what keeps the
    /// anchored path from having to remember it separately.
    #[test]
    fn an_empty_query_matches_nothing() {
        assert_eq!(keyword_score(&[], "anything at all"), None);
    }
}
