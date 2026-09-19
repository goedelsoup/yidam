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

#[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
pub(crate) mod remote;
#[cfg(feature = "vector-read")]
pub(crate) mod vector;

/// The `degraded_reason` an index built in another vector space reports.
///
/// **Not a new string.** `prelude/sdks/parity/mcp/tools.json` froze this value alongside
/// `no_index` and `no_vector_support` — "an index exists and was built with different
/// embedding settings than this server would use, so answering from it would be answering in
/// another vector space" — and says a value outside that set is a divergence. It had no
/// implementation until #536: the contract named the state, and the query path that could
/// reach it never looked. `mcp_serve.rs` records it as "frozen and unforced", which it now
/// is not.
///
/// Here and not in [`vector`], though only that module can produce it: these three strings
/// are a client's whole vocabulary for why retrieval degraded, and one of them living in the
/// module the light build does not compile is how two of them would come to disagree. It is
/// also what lets the light build assert all three against the freeze.
///
/// The light build cannot *produce* it — it never embeds, so it can never find that an index
/// is in another space — but it carries the value anyway, because the vocabulary is the
/// contract and the test asserting all three against the freeze has to run in the build CI
/// compiles on a pull request. Gating the constant would put that assertion in the build
/// nothing checks until main.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
pub(crate) const STALE_CONTRACT: &str = "stale_contract";

/// What to do about it, in the clause shape [`Retrieval::repair`] uses.
///
/// Rebuilding is the only repair: the rows were written by a runtime this binary no longer
/// contains, and nothing on this side can be adjusted to reach them.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
pub(crate) const STALE_CONTRACT_REPAIR: &str =
    "rebuild it with this yidam — `yidam embed && yidam index-build`";

/// The `degraded_reason` a remote index that did not answer reports.
///
/// **A fourth value in a frozen vocabulary, added the way the freeze says to add one.**
/// `prelude/sdks/parity/mcp/tools.json` lists the permitted values and says *"a value outside
/// this set is a divergence; a server needing one should add it here first"* — so it was added
/// there, and the contract version was bumped in all three places that carry it.
///
/// It exists because the three that were there cannot say this. `no_index` is false — the
/// corpus has one and says where. `no_vector_support` is false — this binary can read an
/// index. `stale_contract` is a claim about the vector space, which nothing has established
/// when the service never answered. A 503 reported as any of them would send a reader to a
/// repair that cannot work.
///
/// One string for every kind of failure, deliberately. The *repair* differs — a 403 is a
/// permission and a 503 is a wait — but a client branching on this is deciding whether to
/// trust the ranking, and the answer is the same for all of them. The specific cause goes in
/// the human-readable message beside it, where it is not a contract.
#[cfg_attr(
    not(all(feature = "vector-read", feature = "s3-vectors")),
    allow(dead_code)
)]
pub(crate) const REMOTE_UNAVAILABLE: &str = "remote_unavailable";

/// What to do about it, in the clause shape [`Retrieval::repair`] uses.
#[cfg_attr(
    not(all(feature = "vector-read", feature = "s3-vectors")),
    allow(dead_code)
)]
pub(crate) const REMOTE_UNAVAILABLE_REPAIR: &str =
    "check the index named by `[index.remote]` and the credentials for it — `yidam doctor`";

use anyhow::Result;

use crate::model::DomainModel;

/// What a search found, or why it could not be trusted to find it.
///
/// Not a `Result`: none of the non-`Hits` arms is a failure of the *search*, they are the
/// search being the wrong instrument. Both call sites already carry a keyword arm — `retrieve`
/// falls through to `keyword_retrieve` and an anchored step to `keyword_entries` — so the
/// honest answer is the one they already give when there is no index at all, with its own
/// reason.
#[cfg(feature = "vector-read")]
pub(crate) enum Searched {
    Hits(Vec<Hit>),
    /// This binary embeds into a different space than the index was built in.
    SpaceMismatch,
    /// A remote index did not answer. Carries the service's own words, for the message beside
    /// the frozen reason — never for a client to branch on.
    #[cfg(feature = "s3-vectors")]
    Unavailable(String),
}

/// One row a search returned, owned.
///
/// **Owned, and that is the change a remote backend forced.** Until the S3 Vectors transport
/// arrived, a search borrowed [`crate::model::VectorRow`]s out of an index held in memory and
/// handed back `(&VectorRow, f32)`; a row that arrives over the network is not borrowed from
/// anything this process holds. Rather than two result shapes — one per backend, converging at
/// two call sites that would each have to know which they were looking at — there is one, and
/// the local scan pays a clone per returned row. That is `k` clones, where `k` defaults to 5.
///
/// The fields are exactly what both call sites read: `cmd/serve/tools.rs` renders all five,
/// and `cmd/query/anchor.rs` uses `path` to resolve a node and `score` to rank it.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Repository-relative path, as the index recorded it. The handle both call sites resolve
    /// a node through.
    pub path: String,
    pub class: String,
    pub label: String,
    pub text: String,
    /// Cosine similarity, highest first. The local scan computes it as a dot product over
    /// normalized vectors; the remote backend converts the distance it is given. They are the
    /// same quantity or the remote backend refuses to answer — see
    /// [`crate::s3vectors::response::score_from_distance`].
    pub score: f32,
}

/// What a search may return, in the part of the question a server can be asked.
///
/// Split from the arbitrary predicate it used to be — `keep: impl Fn(&VectorRow) -> bool` —
/// because half of that predicate can be pushed to a remote index and half cannot, and a
/// closure cannot be asked which half it is. `classes` is the pushable half: `retrieve`
/// filters on at most one class, and an anchored step on the classes it narrowed to. The
/// residual — anchor's *"and this path resolves to a node this repository owns"* — stays a
/// closure at the call site, applied to whatever comes back.
///
/// Both backends read this one type, which is what stops them disagreeing about what a filter
/// means: the local scan tests it in Rust, the remote one renders it as a metadata filter, and
/// `s3vectors::filter` asserts the two admit the same rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// `None` admits every class. `Some(&[])` admits none, and is a caller's bug rather than a
    /// state to render — the remote translation refuses it rather than sending `$in: []`,
    /// which S3 Vectors rejects as a validation error.
    pub classes: Option<Vec<String>>,
}

impl Filter {
    /// Every class.
    pub fn any() -> Self {
        Self { classes: None }
    }

    /// One class, or every class when `None` — `retrieve`'s shape exactly.
    pub fn class(name: Option<&str>) -> Self {
        Self {
            classes: name.map(|c| vec![c.to_string()]),
        }
    }

    /// A set of classes — an anchored step's shape.
    pub fn classes(names: &[String]) -> Self {
        Self {
            classes: Some(names.to_vec()),
        }
    }

    /// Whether this filter admits a row of `class`. The local backend's whole reading of it.
    pub fn admits(&self, class: &str) -> bool {
        match &self.classes {
            None => true,
            Some(cs) => cs.iter().any(|c| c == class),
        }
    }
}

/// How text will be resolved to nodes, and — when it will be resolved badly — why.
///
/// Three states rather than a bare `Option`, because "this corpus has no index" and "this
/// binary cannot read the index this corpus has" are different diagnoses with different
/// repairs, and a lone `degraded: true` collapses them into one. The first is fixed by
/// `yidam embed && yidam index-build`; the second by reinstalling with `--features vector-read`,
/// which reads an index and needs no protoc — building one is a separate, heavier feature.
/// A client told only that retrieval was degraded cannot tell which it is looking at.
pub(crate) enum Retrieval {
    /// Semantic search, over a loaded index.
    ///
    /// Boxed: it carries the decoded rows and a lazily-loaded embedder, and an unboxed
    /// variant would make every `Retrieval` — including the two empty ones the light build
    /// uses exclusively — as large as the heaviest.
    #[cfg(feature = "vector-read")]
    Vector(Box<vector::IndexState>),
    /// Semantic search, over a vector bucket.
    ///
    /// A fourth state rather than a flavour of [`Self::Vector`], because what can go wrong is
    /// different in kind: a local index is present or it is not, and a remote one can be
    /// declared, reachable, forbidden or throttled. Collapsing them would mean one arm whose
    /// failures a caller cannot tell apart.
    #[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
    Remote(Box<remote::RemoteState>),
    /// Keyword search: the corpus has no vector index.
    NoIndex,
    /// Keyword search: the corpus *has* an index and this build cannot read it.
    ///
    /// Compiled only into the build that can actually be in this state. A binary carrying
    /// `index` reads any index it finds or fails to start, so the variant would be
    /// unreachable there — and an unreachable state that still appears in a match is one a
    /// reader has to rule out by hand every time.
    #[cfg(not(feature = "vector-read"))]
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
            #[cfg(feature = "vector-read")]
            Self::Vector(_) => None,
            // Not degraded. A declared remote index that answers is semantic search; the
            // reason a *call* against it degrades is discovered per search, not here, and is
            // spliced in at the call site the way `stale_contract` already is.
            #[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
            Self::Remote(_) => None,
            Self::NoIndex => Some("no_index"),
            #[cfg(not(feature = "vector-read"))]
            Self::NoVectorSupport => Some("no_vector_support"),
        }
    }

    /// What to do about it, in one clause. Present tense, no leading capital — callers
    /// splice it into a sentence of their own.
    pub(crate) fn repair(&self) -> Option<&'static str> {
        match self {
            #[cfg(feature = "vector-read")]
            Self::Vector(_) => None,
            #[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
            Self::Remote(_) => None,
            Self::NoIndex => Some("run `yidam embed && yidam index-build` to build one"),
            #[cfg(not(feature = "vector-read"))]
            Self::NoVectorSupport => {
                Some("reinstall with `--features vector-read` to read the index this corpus has")
            }
        }
    }
}

/// Decide how text will be resolved, and read the indexed commit either way.
///
/// Three bodies, one signature. The split is what lets the light build compile: decoding
/// `index/corpus.arrow` needs `arrow-ipc` and embedding a query needs `fastembed`, and
/// neither is in the default dependency set. What *is* in it is the raw `index/meta.json`
/// that `load_domain_model` already read — enough to know an index exists and which commit
/// it was built at, which is exactly the two facts a degraded caller should still report.
///
/// # A declared remote index wins
///
/// A corpus that writes `[index.remote]` is queried out of it, even when a local
/// `.yidam/index/` is also present. Falling back to the local one when the service is
/// unreachable was considered and rejected: two indexes that can disagree, switched between
/// silently, is a ranking whose provenance nobody can state. What happens instead is what
/// happens when there is no index at all — keyword search, under [`REMOTE_UNAVAILABLE`].
#[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
pub(crate) fn load(
    root: &std::path::Path,
    model: &DomainModel,
) -> Result<(Retrieval, Option<String>)> {
    if let Some(state) = remote_state(root, model)? {
        // `None`, not a guess. The commit a remote index was built at is on the records
        // themselves and reading it would cost a round trip at startup, on a path that may
        // never search. The MCP contract's `stale` is a tri-state for exactly this: the
        // honest answer to "is it behind?" here is "cannot tell".
        let local = model.index.as_ref().and_then(indexed_commit);
        return Ok((Retrieval::Remote(Box::new(state)), local));
    }
    load_local(model)
}

/// Build the remote state, or `None` when this corpus declares no remote index.
#[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
fn remote_state(
    root: &std::path::Path,
    model: &DomainModel,
) -> Result<Option<remote::RemoteState>> {
    use anyhow::Context;

    let config = crate::config::load_yidam_config(root)?;
    let Some(declared) = config.index.remote.as_ref() else {
        return Ok(None);
    };
    let index = crate::s3vectors::RemoteIndex::resolve(declared)?;

    // Same refusal `index-push` makes, for the same reason: every key is prefixed with the
    // corpus, so a repository that cannot see its own root commit would be asking about a
    // corpus that is not the one it is in.
    let corpus = model
        .provenance
        .genesis_hash
        .as_deref()
        .map(|h| h.chars().take(12).collect::<String>())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "this repository cannot see its own genesis commit, so it cannot say which \
                 corpus in the remote index is its own.\n  \
                 A shallow clone is the usual cause — `git fetch --unshallow`."
            )
        })?;

    // The model to embed queries with. A local `embed.config.json` is authoritative when one
    // is present; otherwise the default, checked against the index's own witness on the first
    // search. It is not read from the witness here because that is a round trip at startup.
    let model_id = model
        .index
        .as_ref()
        .and_then(|i| i.embed_config.as_ref())
        .map(|c| c.model_id.clone())
        .unwrap_or_else(|| crate::embedding::DEFAULT_MODEL.to_string());

    let creds = crate::vault::creds::resolve_scope(&crate::vault::creds::index_scope(), |k| {
        std::env::var(k).ok()
    })
    .context("the remote index declared in `[index.remote]` needs credentials")?;

    Ok(Some(remote::RemoteState {
        index: index.clone(),
        corpus,
        model_id,
        client: crate::s3vectors::transport::Client::new(index, creds)?,
        embedder: std::cell::RefCell::new(None),
        space: std::cell::RefCell::new(None),
    }))
}

#[cfg(all(feature = "vector-read", not(feature = "s3-vectors")))]
pub(crate) fn load(
    _root: &std::path::Path,
    model: &DomainModel,
) -> Result<(Retrieval, Option<String>)> {
    load_local(model)
}

#[cfg(feature = "vector-read")]
fn load_local(model: &DomainModel) -> Result<(Retrieval, Option<String>)> {
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
                    embed_config: idx.embed_config.clone(),
                    embedder: std::cell::RefCell::new(None),
                    space: std::cell::RefCell::new(None),
                })),
                indexed_commit(idx),
            ))
        }
        None => Ok((Retrieval::NoIndex, None)),
    }
}

#[cfg(not(feature = "vector-read"))]
pub(crate) fn load(
    _root: &std::path::Path,
    model: &DomainModel,
) -> Result<(Retrieval, Option<String>)> {
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
    ///
    /// All four. `stale_contract` got an implementation in #536 — it was in the frozen set
    /// from the start and nothing produced it — and `remote_unavailable` went the other way
    /// round: the state existed first, and it was added to the freeze before it was produced
    /// here, which is the order the contract itself prescribes for a new value.
    ///
    /// This pins the spellings. `degraded_reason_freeze.rs` is what holds them to
    /// `prelude/sdks/parity/mcp/tools.json`, because a constant that agrees with a second
    /// constant proves nothing about the document a client reads.
    #[test]
    fn the_degraded_reasons_are_distinct_and_stable() {
        assert_eq!(Retrieval::NoIndex.degraded_reason(), Some("no_index"));
        #[cfg(not(feature = "vector-read"))]
        assert_eq!(
            Retrieval::NoVectorSupport.degraded_reason(),
            Some("no_vector_support")
        );
        assert_eq!(super::STALE_CONTRACT, "stale_contract");
        assert_eq!(super::REMOTE_UNAVAILABLE, "remote_unavailable");

        // Distinct, because each carries a different repair and a collision would collapse
        // two of them into one.
        let all = [
            "no_index",
            "no_vector_support",
            super::STALE_CONTRACT,
            super::REMOTE_UNAVAILABLE,
        ];
        let mut unique = all.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            all.len(),
            "two degraded reasons share a string"
        );
    }

    /// Every degraded state owes a repair, or the reason string is a diagnosis with no
    /// treatment — and the two states exist precisely because their treatments differ.
    #[test]
    fn a_reason_and_a_repair_are_present_together_or_not_at_all() {
        #[cfg(not(feature = "vector-read"))]
        let states = [Retrieval::NoIndex, Retrieval::NoVectorSupport];
        #[cfg(feature = "vector-read")]
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

    /// **What `vector-read` exists to make true.** A build with no `lancedb` and no protoc
    /// decodes a real index and reports itself *not* degraded.
    ///
    /// The arrow buffer is written here rather than fetched from a fixture, against the same
    /// schema `cmd/index_build.rs` writes — which is the point, since that module does not
    /// compile in this build. If the two schemas ever diverge this test decodes something the
    /// real writer would not have produced, so it is pinned field-for-field.
    ///
    /// It stops at decoding. Embedding a *query* loads ONNX weights over the network, so the
    /// last step of the round trip is not something a hermetic suite can run; `yidam vault
    /// pull --index` followed by `serve --mcp` is where a person sees it.
    #[cfg(feature = "vector-read")]
    #[test]
    fn a_build_that_can_read_but_not_build_an_index_is_not_degraded() {
        use crate::model::IndexData;
        use std::sync::Arc;

        let dim = 3i32;
        let schema = Arc::new(arrow_schema::Schema::new(vec![
            arrow_schema::Field::new("path", arrow_schema::DataType::Utf8, false),
            arrow_schema::Field::new("class", arrow_schema::DataType::Utf8, false),
            arrow_schema::Field::new("label", arrow_schema::DataType::Utf8, false),
            arrow_schema::Field::new("text", arrow_schema::DataType::Utf8, false),
            arrow_schema::Field::new(
                "vector",
                arrow_schema::DataType::FixedSizeList(
                    Arc::new(arrow_schema::Field::new(
                        "item",
                        arrow_schema::DataType::Float32,
                        true,
                    )),
                    dim,
                ),
                false,
            ),
        ]));
        let item = Arc::new(arrow_schema::Field::new(
            "item",
            arrow_schema::DataType::Float32,
            true,
        ));
        let batch = arrow_array::RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(arrow_array::StringArray::from(vec!["corpus/gauge/a.md"])),
                Arc::new(arrow_array::StringArray::from(vec!["gauge"])),
                Arc::new(arrow_array::StringArray::from(vec!["Gauge A"])),
                Arc::new(arrow_array::StringArray::from(vec!["a gauge on a river"])),
                Arc::new(arrow_array::FixedSizeListArray::new(
                    item,
                    dim,
                    Arc::new(arrow_array::Float32Array::from(vec![1.0f32, 0.0, 0.0])),
                    None,
                )),
            ],
        )
        .unwrap();

        let mut ipc: Vec<u8> = Vec::new();
        {
            let mut w = arrow_ipc::writer::FileWriter::try_new(&mut ipc, &schema).unwrap();
            w.write(&batch).unwrap();
            w.finish().unwrap();
        }

        let index = IndexData {
            arrow_ipc: ipc,
            meta_raw: br#"{"indexed_commit":"abc123"}"#.to_vec(),
            meta: serde_json::json!({"indexed_commit": "abc123", "model_name": "m"}),
            embed_config: None,
        };
        let rows = crate::model::index_rows(&index).expect("a vector-read build decodes an index");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "a gauge on a river");
        assert_eq!(rows[0].vector, vec![1.0, 0.0, 0.0]);

        // And the state built from it reports no degradation — which is the sentence #417
        // could not make true and this build can.
        let state = Retrieval::Vector(Box::new(vector::IndexState {
            rows,
            model_id: "m".to_string(),
            // This index carries no contract, which is `Verdict::Unverifiable` and not a
            // degradation: an index built before the block existed gave the query path
            // nothing to check against, and failing those closed would break all of them.
            embed_config: None,
            embedder: std::cell::RefCell::new(None),
            space: std::cell::RefCell::new(None),
        }));
        assert_eq!(state.degraded_reason(), None);
        assert_eq!(state.repair(), None);
    }

    /// An empty query matches nothing rather than everything. `retrieve` relied on this
    /// through a guard of its own; folding the guard into the scorer is what keeps the
    /// anchored path from having to remember it separately.
    #[test]
    fn an_empty_query_matches_nothing() {
        assert_eq!(keyword_score(&[], "anything at all"), None);
    }
}
