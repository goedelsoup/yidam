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
//! and an anchored query fall through to [`Bm25`] and say so.

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
///
/// Ungated on `vector-read`, though only that build can produce one. The shape of a search's
/// outcome — and everything `cmd/query/anchor.rs` decides from it — carries no dependency on
/// the ML stack, and the build CI compiles on a pull request is the light one. Gating it put
/// the anchored step's whole control flow in the build nothing checks until main.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
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
/// The fields are exactly what both call sites read: `cmd/serve/tools.rs` renders all six,
/// and `cmd/query/anchor.rs` uses `path` to resolve a node and `score` to rank it. An anchored
/// step reads neither `text` nor `truncated` on purpose — it resolves the row to a node and
/// reads that node's own text out of this repository, so a cut carried by the index is not a
/// cut in what it renders.
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
    /// Whether [`Self::text`] is a cut of what the corpus holds rather than all of it.
    ///
    /// **A remote index is the only place this can be true, and it is not a defect there.**
    /// S3 Vectors caps per-vector metadata at 40 KB, so a row whose text exceeds it is pushed
    /// cut and flagged — see [`crate::s3vectors::request::metadata`]. The flag travelled back
    /// with the row and nothing read it, so a cut answer rendered exactly like a whole one
    /// (#853). The population it can happen to is small and unbounded: over 3,246 rows in
    /// sixteen measured corpora one is cut, and the one that is is a catalog source, whose
    /// text is a whole markdown document that no format caps (RFC-0033 §8.3).
    ///
    /// **The local scan answers `false`, and that is the answer rather than a placeholder.**
    /// A local index carries whole text — nothing in `.yidam/index/` has a per-row ceiling —
    /// so `false` is a fact about that backend, and the asymmetry between the two is exactly
    /// what a reader of a result should be able to see.
    pub truncated: bool,
    /// The corpus this row came from, or `None` when it is this repository's own.
    ///
    /// **A row from another corpus is what a shared index makes possible** (RFC-0033 phase 3).
    /// Keys are `<genesis12>/<path>`, so a query naming more than one corpus gets rows whose
    /// `path` is a path in a repository this process cannot open. The value is the
    /// twelve-character genesis hash the index keys on — the corpus's one identity, and the
    /// authority [`crate::paths::reference_of_path`] renders into an RFC-0032 identifier.
    ///
    /// **`corpus` and not `origin`, which this surface already uses for something else.** A
    /// keyword result's `origin` names an installed dependency: a corpus under
    /// `.yidam/tonpa/`, whose nodes this process has read and `get_node` can return. A corpus
    /// sharing a vector index is not installed and not readable — what came back is the row
    /// and nothing else. Spelling both as `origin` would tell a client it could fetch
    /// something it cannot, which is the affordance the `id` field exists to keep honest.
    ///
    /// **Null for local**, the convention `origin` already follows here: absence of a corpus
    /// *is* the statement that the row is this repository's. A local index can never set it —
    /// one index directory is one corpus.
    ///
    /// It is deliberately not the declared alias a caller may have used to ask. An alias is a
    /// nickname chosen by the repository that declared it (RFC-0032 §4.5), and a result whose
    /// identity changed with the reader's config would not be an identity.
    pub corpus: Option<String>,
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
    /// Whether `classes` names **nodes** rather than rows.
    ///
    /// The difference is the whole of RFC-0033 §4.5. `retrieve`'s class filter is a claim
    /// about the row: the caller asked for rows the index labels `concept`, and the index's
    /// own labels answer for that. An anchored step's is a claim about the *node* the row
    /// resolves to — the step narrowed to classes the corpus uses, and a row's recorded label
    /// only answers for it while the index derived that label the way this binary does.
    ///
    /// So only this kind of filter has a precondition, and [`Self::as_applied`] is where an
    /// index that cannot meet it says so.
    pub about_nodes: bool,
}

impl Filter {
    /// Every class.
    pub fn any() -> Self {
        Self {
            classes: None,
            about_nodes: false,
        }
    }

    /// One class, or every class when `None` — `retrieve`'s shape exactly.
    pub fn class(name: Option<&str>) -> Self {
        Self {
            classes: name.map(|c| vec![c.to_string()]),
            about_nodes: false,
        }
    }

    /// A set of classes, as a claim about the rows — the shape a test or a row-level caller
    /// wants. An anchored step wants [`Self::nodes_of`].
    pub fn classes(names: &[String]) -> Self {
        Self {
            classes: Some(names.to_vec()),
            about_nodes: false,
        }
    }

    /// A set of classes, as a claim about the **nodes** the rows resolve to — an anchored
    /// step's shape.
    ///
    /// An empty set is [`Self::any()`] rather than a filter admitting nothing. A step that
    /// narrowed to no class has no candidate node either, so its residual returns the same
    /// nothing — and `Some(&[])` would reach the remote translation, which refuses it.
    pub fn nodes_of(names: &[String]) -> Self {
        if names.is_empty() {
            return Self::any();
        }
        Self {
            classes: Some(names.to_vec()),
            about_nodes: true,
        }
    }

    /// The part of this filter an index may actually apply.
    ///
    /// **This is where RFC-0033 §4.5's premise is discharged instead of assumed.** A row's
    /// `class` was written by whatever `yidam embed` wrote when the index was built; a node's
    /// class is computed now by [`crate::paths::class_of_path`]. Both are a function of the
    /// *same string* — the row carries the path its class was derived from — so they can only
    /// disagree if the derivation itself differed between the binary that built the index and
    /// this one. Not if a node moved: a node's class **is** its parent directory, so a move
    /// changes the path too, and a row whose path no longer names a node is already rejected
    /// by the caller's residual.
    ///
    /// That is a question an index can be asked once rather than guessed at per row:
    /// `path_derived` is [`crate::embed_config::EmbedConfig::classes_are_path_derived`], which
    /// `index_build` writes only after checking every record it indexed. An index that makes
    /// no such claim — every index built before the field existed — is handed `any()`, which
    /// is exactly what this surface did before the claim existed. The residual is
    /// authoritative either way, so the cost of a "no" is a wider fetch and never a wrong or
    /// empty answer.
    ///
    /// Applied by each backend rather than by the caller, because a remote index does not know
    /// its own contract until the witness has been fetched — which happens on the first
    /// search, after the caller has already built the filter.
    pub fn as_applied(&self, path_derived: bool) -> Self {
        if self.about_nodes && !path_derived {
            return Self::any();
        }
        self.clone()
    }

    /// Whether this filter admits a row of `class`. The local backend's whole reading of it.
    pub fn admits(&self, class: &str) -> bool {
        match &self.classes {
            None => true,
            Some(cs) => cs.iter().any(|c| c == class),
        }
    }
}

/// The corpora one search is asking about, and which of them is the asking repository's own.
///
/// **A local index is one corpus by construction and a remote one is not.** Keys in a vector
/// bucket are `<genesis12>/<path>` and `corpus` is filterable from the first push (RFC-0033
/// §4.2), so several corpora can share an index without any of them re-pushing — and the
/// question *"which of these already says something about X"* becomes askable. This is the
/// asking.
///
/// **Own is always in the set, and the named ones are added to it.** A server answers for the
/// corpus it was started in: its `absence` diagnosis counts that corpus's nodes, its
/// `get_node` reads that corpus's files, and a span that could *replace* its corpus rather
/// than widen past it would make every one of those answers about something else. A caller
/// that wants only the neighbours filters on [`Hit::origin`], which is exactly what that field
/// is for.
///
/// Ungated, like [`Filter`] and [`Searched`] and for the same reason: only a remote backend
/// can span, and the control flow that decides whether a span happened is compiled into every
/// build — including the light one CI runs on a pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpora {
    own: String,
    /// The others, deduplicated, without `own`, in the order the caller named them.
    also: Vec<String>,
}

impl Corpora {
    /// This corpus alone — what every query asked before #835, and what a query with no
    /// `corpora` argument still asks.
    pub fn own(own: impl Into<String>) -> Self {
        Self {
            own: own.into(),
            also: Vec::new(),
        }
    }

    /// This corpus and the ones named.
    ///
    /// Naming your own corpus is a no-op rather than an error: a caller listing the corpora in
    /// a shared index has no reason to remove itself from the list, and `$in [x, x]` is a
    /// predicate nobody should have to think about.
    pub fn across(own: impl Into<String>, also: &[String]) -> Self {
        let own = own.into();
        let mut seen = std::collections::BTreeSet::new();
        let also = also
            .iter()
            .filter(|c| **c != own && seen.insert((*c).clone()))
            .cloned()
            .collect();
        Self { own, also }
    }

    /// Every corpus this search may return a row from, own first.
    pub fn ids(&self) -> Vec<&str> {
        std::iter::once(self.own.as_str())
            .chain(self.also.iter().map(String::as_str))
            .collect()
    }

    /// Whether this search reaches past the corpus it was asked in.
    ///
    /// What the MCP contract's `scope` reports, and it reports what *happened*: a call naming
    /// only corpora this one already is answers `local`, the same way `query --across` answers
    /// `local` for a repository with no dependencies installed.
    pub fn is_across(&self) -> bool {
        !self.also.is_empty()
    }

    /// What [`Hit::corpus`] should say about a row keyed under `corpus`: `Some` when the row
    /// belongs to another corpus, `None` when it is this repository's own.
    pub fn foreign(&self, corpus: &str) -> Option<String> {
        (corpus != self.own).then(|| corpus.to_string())
    }

    /// Whether a row keyed under `corpus` is one this search asked for.
    pub fn admits(&self, corpus: &str) -> bool {
        self.own == corpus || self.also.iter().any(|c| c == corpus)
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
        classes_are_path_derived: std::cell::Cell::new(false),
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

/// BM25's two free parameters, at the values the retrieval literature settled on.
///
/// Not configurable, and that is a decision rather than an omission. Tuning `k1` and `b`
/// means tuning against a graded goal set, and the one goal set this repository has is
/// `bench`'s — which refuses to run at all when the flat arm would be keyword search. A knob
/// with no way to measure a turn of it is a knob that gets turned by taste.
const BM25_K1: f32 = 1.2;
const BM25_B: f32 = 0.75;

/// BM25 over the set of nodes being scanned.
///
/// One scorer, shared by `retrieve`'s fallback and an anchored query's. They were the same
/// three lines written twice, and the second copy is the one that would have quietly stopped
/// matching the first — which matters more here than it looks, because `bench` compares an
/// anchored arm against a flat one and a scoring difference between them would be read as a
/// result.
///
/// **Why it is a type and not a function.** Its predecessor scored the fraction of query
/// terms a node contained, which needs nothing but the node in front of it. The consequence
/// was not that ranking was poor — it was that there was *no* ranking: every node holding all
/// of the query's terms scored exactly `1.0`, the tie broke on the qualified id, and a
/// two-term query over a corpus where forty nodes mentioned both terms returned the first `k`
/// **alphabetically** (#1026). Term frequency, inverse document frequency and length
/// normalization are all statements about the *set*, so the set has to be seen before any one
/// member of it can be scored.
pub(crate) struct Bm25 {
    /// One entry per query term that at least one document holds, with that term's inverse
    /// document frequency over the scanned set.
    ///
    /// A term no document holds is dropped here rather than carried at zero: it cannot move
    /// an ordering, and dropping it keeps [`Bm25::score`]'s inner loop over the terms that
    /// can.
    terms: Vec<(String, f32)>,
    /// Mean document length in whitespace tokens, and never zero — see [`Bm25::over`].
    mean_len: f32,
}

impl Bm25 {
    /// Collect the statistics of a scanned set.
    ///
    /// `docs` is the composed, lowercased text of **every** candidate the caller will scan,
    /// including the ones that will match nothing. Document frequency and mean length are
    /// properties of the set and not of the hits; taken over the hits alone every term looks
    /// common and every document looks long, which is the ordering this replaces.
    ///
    /// **The two callers scan different sets, and their IDF therefore differs.** `retrieve`
    /// reaches installed dependencies and an anchor does not — the asymmetry is deliberate
    /// and argued at each call site. Computing IDF over whichever set is scanned is the
    /// intended reading rather than a drift: IDF measures how much a term discriminates
    /// *within the collection the answer is drawn from*, and an anchor cannot return a
    /// dependency's node however common the term is over there. A statistic taken over a
    /// wider set than the answer would weight a term by documents that can never come back.
    pub(crate) fn over<'a>(query: &str, docs: impl IntoIterator<Item = &'a str>) -> Self {
        let query_terms = terms(query);
        let mut df = vec![0usize; query_terms.len()];
        let mut n = 0usize;
        let mut total_len = 0usize;
        // One pass. Both statistics come off the same walk, and a second walk would mean
        // either cloning the iterator or holding the composed text twice.
        for doc in docs {
            n += 1;
            total_len += doc.split_whitespace().count();
            for (i, term) in query_terms.iter().enumerate() {
                if doc.contains(term.as_str()) {
                    df[i] += 1;
                }
            }
        }
        let terms = query_terms
            .into_iter()
            .zip(df)
            .filter(|(_, df)| *df > 0)
            .map(|(term, df)| {
                // THE `+1` VARIANT, and it is a behaviour rather than a preference. The
                // textbook `ln((n - df + 0.5) / (df + 0.5))` goes negative once a term is in
                // more than half the set, so a node holding a common term would score *below*
                // a node holding nothing of the query and could drop out of an answer it used
                // to be in. This form is positive everywhere, which is what makes the set a
                // query returns exactly the set term-presence returned — only the order moves.
                let idf = 1.0 + (n as f32 - df as f32 + 0.5) / (df as f32 + 0.5);
                (term, idf.ln())
            })
            .collect();
        // A set with no documents, or one whose documents are all empty, has no mean to
        // normalize against. `1.0` keeps the division defined; nothing in such a set can
        // score anyway, since an empty document contains no non-empty term and `terms` above
        // is then empty.
        let mean_len = match total_len {
            0 => 1.0,
            total => total as f32 / n as f32,
        };
        Self { terms, mean_len }
    }

    /// One document's score, or `None` when it holds none of the query's terms.
    ///
    /// `haystack` is expected already lowercased and is expected to be one of the documents
    /// [`Bm25::over`] was given — the caller builds it once per node and this is called once
    /// per node, so lowercasing here would be the same work in a worse place.
    pub(crate) fn score(&self, haystack: &str) -> Option<f32> {
        let len = haystack.split_whitespace().count() as f32;
        // The length penalty, computed once: at `b = 0.75` a document three times the mean
        // length needs two and a half occurrences of a term to score what one occurrence
        // scores in a document at the mean, and twice the mean needs one and three quarters.
        let norm = BM25_K1 * (1.0 - BM25_B + BM25_B * len / self.mean_len);
        let mut total = 0.0;
        let mut hit = false;
        for (term, idf) in &self.terms {
            // Occurrences as a SUBSTRING, which is what `contains` matched and is deliberately
            // unchanged: the set of nodes a query returns is the set it returned before, and
            // only the order is new. Matching on a token boundary instead would be a recall
            // change — it would stop `graph` finding `graphs` — and that is a separate
            // question from whether the answer can be ordered.
            let tf = haystack.matches(term.as_str()).count() as f32;
            if tf == 0.0 {
                continue;
            }
            hit = true;
            // Saturating in `tf`: the tenth occurrence of a term says far less than the
            // second, and the fraction above approaches `k1 + 1` rather than growing without
            // bound. That is the half a raw term count gets wrong, and the length penalty is
            // the other half.
            total += idf * (tf * (BM25_K1 + 1.0)) / (tf + norm);
        }
        hit.then_some(total)
    }
}

/// Serialize a score as the `f64` its own shortest text denotes, so both surfaces publish the
/// same number.
///
/// `yidam query` writes its report as JSON **text**, where serde_json formats an `f32` at
/// `f32` width: `2.5881803`. The MCP `query` tool builds a [`serde_json::Value`] instead,
/// where every number is an `f64`, and the same score arrives widened to
/// `2.5881803035736084`. Two surfaces then publish visibly different numbers for a score they
/// computed identically, on the same corpus, in the same process.
///
/// Narrowing here — parsing the `f32`'s own shortest representation back as an `f64` — makes
/// them agree, because it is the number the CLI's text already denotes. Neither surface loses
/// a digit it had: an `f32` has no more.
///
/// #1026 found this rather than caused it. Every keyword score used to be a fraction with a
/// small denominator, so the parity test between the two surfaces compared `0.75` against
/// `0.75` for two years and never reached a score whose digits mattered. The cosine arm would
/// have shown it, and `--features vector-read` is not in the PR matrix.
pub(crate) fn serialize_score<S>(score: &f32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // `to_string` is the shortest text that round-trips to this `f32`, and `f64` parses every
    // form of it — including `inf` and `NaN`, which stay whatever each serializer already did
    // with them rather than becoming a second behaviour here.
    serializer.serialize_f64(score.to_string().parse().unwrap_or(f64::NAN))
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

    // ── ranking ───────────────────────────────────────────────────────────────

    /// A fixture whose alphabetical order is **not** its ranking, which is the point.
    ///
    /// Four documents: two hold every query term and differ in both length and term
    /// frequency, one holds only the common term, and one holds nothing. The fourth is not
    /// padding — document frequency and mean length are properties of the set, so a fixture
    /// containing only its own hits would give `dam` the same weight as `hydropeaking` and
    /// make every document look short.
    ///
    /// The ids are chosen so that the predecessor's answer and BM25's disagree. Under term
    /// presence `a-long-review` and `z-short-note` both score exactly `1.0`, the tie breaks on
    /// the id, and the long one comes back first.
    const DOCS: &[(&str, &str)] = &[
        (
            "a-long-review",
            "a review of the literature on sub-daily discharge variation below hydroelectric \
             facilities, a phenomenon named hydropeaking, covering the regulatory history, the \
             measurement record, the statistical summaries in common use, the ecological \
             response studies, the mitigation trials, and the open questions that remain after \
             forty years of work on rivers where a dam is present",
        ),
        (
            "b-dam-history",
            "the dam was built in 1957 and raised in 1974, and the reservoir behind it stores \
             two seasons of runoff",
        ),
        (
            "c-low-flow",
            "seven-day low flow with a ten-year recurrence interval, computed from the gauge \
             record",
        ),
        (
            "z-short-note",
            "hydropeaking at the dam: hydropeaking is the diel cycle",
        ),
    ];

    /// Every document in the fixture, in the shape [`Bm25::over`] takes.
    fn texts() -> impl Iterator<Item = &'static str> {
        DOCS.iter().map(|(_, text)| *text)
    }

    /// The ordering both call sites produce: score descending, then the id.
    ///
    /// The tie-break is reproduced here rather than approximated, because it is what makes
    /// these tests falsifiable. A scorer that returned a constant would leave this sorting
    /// alphabetically — which is what it did before #1026 — so the assertions below go red
    /// under a constant and would not under a membership check.
    fn ranked(query: &str, docs: &'static [(&'static str, &'static str)]) -> Vec<&'static str> {
        let bm25 = Bm25::over(query, docs.iter().map(|(_, text)| *text));
        let mut scored: Vec<(&'static str, f32)> = docs
            .iter()
            .filter_map(|(id, text)| bm25.score(text).map(|score| (*id, score)))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        scored.into_iter().map(|(id, _)| id).collect()
    }

    /// **The defect #1026 names.** The whole returned order, not membership.
    ///
    /// A short note that is *about* hydropeaking at a dam outranks a long review that merely
    /// mentions both — length normalization and saturating term frequency, which is the half
    /// a fraction of terms hit cannot express. Asserting membership here would pass against
    /// the scorer this replaced, and so would asserting a score: its defect was that its
    /// scores were *equal*.
    #[test]
    fn a_short_node_about_the_query_outranks_a_long_one_that_mentions_it() {
        assert_eq!(
            ranked("hydropeaking dam", DOCS),
            vec!["z-short-note", "a-long-review", "b-dam-history"],
        );
    }

    /// A rare term is worth more than a common one, which is the other half.
    ///
    /// `dam` is in three of the four documents and `hydropeaking` in two, so a document
    /// holding only the rarer term outranks one holding only the commoner. Under term
    /// presence both hold one of two terms, both score `0.5`, and the answer is again
    /// alphabetical — `b-dam-history` first.
    #[test]
    fn the_rarer_term_carries_the_more_weight() {
        let bm25 = Bm25::over("hydropeaking dam", texts());
        let rare = bm25.score(DOCS[3].1).unwrap();
        let common = bm25.score(DOCS[1].1).unwrap();
        assert!(
            rare > common,
            "a document holding the rare term scored {rare}, one holding the common term \
             {common}"
        );
    }

    /// **The set that comes back is the set that came back before.** Only the order is new.
    ///
    /// The textbook IDF, `ln((n - df + 0.5) / (df + 0.5))`, goes negative once a term is in
    /// more than half the set — so a node holding a term every other node holds would score
    /// below a node holding nothing of the query, and would drop out of an answer it used to
    /// be in. This pins the `+1` variant by its consequence rather than by its formula.
    #[test]
    fn a_term_every_document_holds_still_scores_above_holding_nothing() {
        let docs = ["flow in a channel", "flow over a weir", "flow at a gauge"];
        let bm25 = Bm25::over("flow", docs);
        for doc in docs {
            let score = bm25.score(doc);
            assert!(
                score.is_some_and(|s| s > 0.0),
                "a document holding the query's only term scored {score:?}"
            );
        }
        // And nothing else changed about membership: no term, no row.
        assert_eq!(bm25.score("nothing in common"), None);
    }

    /// A set with nothing in it is no score rather than a `NaN`.
    ///
    /// Reachable: every `--class` filter that admits no node, and every step whose candidate
    /// classes are empty. It is the arithmetic case — mean document length is a divisor, and
    /// a set with no length at all would hand a caller a `NaN` to sort by.
    #[test]
    fn a_set_with_no_length_is_no_score_rather_than_a_nan() {
        let empty = Bm25::over("hydropeaking", std::iter::empty());
        assert_eq!(empty.score("hydropeaking at the dam"), None);
        let blank = Bm25::over("hydropeaking", ["", ""]);
        assert_eq!(blank.score("hydropeaking at the dam"), None);
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
        assert_eq!(Bm25::over("", texts()).score("anything at all"), None);
        // Whitespace is not a term either, and `terms` is what decides that.
        assert_eq!(Bm25::over("   ", texts()).score("anything at all"), None);
    }

    fn row(path: &str, class: &str) -> Hit {
        Hit {
            path: path.to_string(),
            class: class.to_string(),
            label: String::new(),
            text: String::new(),
            score: 1.0,
            truncated: false,
            corpus: None,
        }
    }

    /// A filter naming rows is pushed whatever the index says about its class metadata.
    ///
    /// `retrieve`'s shape. The caller asked for rows the index labels `concept`, and the
    /// index's own labels are what answer that — there is no second derivation involved and
    /// nothing to vouch for.
    #[test]
    fn a_filter_about_rows_is_applied_whether_or_not_the_index_vouches() {
        let f = Filter::class(Some("concept"));
        assert_eq!(f.as_applied(true), f);
        assert_eq!(f.as_applied(false), f);
    }

    /// A filter naming nodes is applied only by an index that can vouch for its class
    /// metadata, and widens to `any()` otherwise.
    #[test]
    fn a_filter_about_nodes_widens_against_an_index_that_vouches_for_nothing() {
        let f = Filter::nodes_of(&["concept".to_string(), "person".to_string()]);
        assert_eq!(f.as_applied(true), f);
        assert_eq!(f.as_applied(false), Filter::any());
    }

    /// Widening is to every class, not to none. The distinction is the difference between a
    /// wider fetch and an empty result, which is the whole of RFC-0033 §4.5.
    #[test]
    fn widening_admits_every_class() {
        let widened = Filter::nodes_of(&["concept".to_string()]).as_applied(false);
        assert!(widened.admits("concept"));
        assert!(widened.admits("anything-at-all"));
    }

    /// An empty class set is `any()` from the start, in either shape: `Some(&[])` reaches the
    /// remote translation, which refuses it rather than sending `$in: []`.
    #[test]
    fn a_step_that_narrowed_to_no_class_never_produces_an_empty_class_set() {
        let f = Filter::nodes_of(&[]);
        assert_eq!(f, Filter::any());
        assert_eq!(f.as_applied(true), Filter::any());
    }

    /// A row a search returns is checked against nothing here, and that is the point of asking
    /// the index once: a catalog source's `class` is its catalog `type` and its path's parent
    /// is `catalog`, so a per-row check of the two derivations would have to know which rows
    /// were nodes before it could run at all.
    #[test]
    fn the_question_is_asked_of_the_index_and_not_of_a_row() {
        let source = row(".yidam/catalog/streamgage-api.md", "api");
        assert!(Filter::nodes_of(&["api".to_string()])
            .as_applied(true)
            .admits(&source.class));
    }

    // ── which corpora a search is about ───────────────────────────────────────

    fn also(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    /// One corpus is one corpus, and the asking one leads the set.
    ///
    /// The order is asserted because it reaches a canonical request: `filter::to_json` renders
    /// `ids()` into a `$in` array, and a set whose order moved between runs would make a
    /// signed request that differs from one query to the next while meaning the same thing.
    #[test]
    fn own_is_one_corpus_and_leads_a_wider_set() {
        assert_eq!(Corpora::own("abc123").ids(), vec!["abc123"]);
        assert!(!Corpora::own("abc123").is_across());

        let wide = Corpora::across("abc123", &also(&["other9", "third4"]));
        assert_eq!(wide.ids(), vec!["abc123", "other9", "third4"]);
        assert!(wide.is_across());
    }

    /// The asking corpus cannot be excluded, and naming it again does not duplicate it.
    ///
    /// Both halves of the rule the MCP contract states: the serving corpus is always in the
    /// set, because every other answer the tool gives is about that corpus.
    #[test]
    fn the_asking_corpus_is_always_in_the_set_exactly_once() {
        let named_itself = Corpora::across("abc123", &also(&["abc123"]));
        assert_eq!(named_itself.ids(), vec!["abc123"]);
        // And it is `local`: nothing beyond this corpus was asked for, whatever was typed.
        assert!(!named_itself.is_across());

        let repeated = Corpora::across("abc123", &also(&["other9", "other9", "abc123"]));
        assert_eq!(repeated.ids(), vec!["abc123", "other9"]);
    }

    /// A row's origin is `None` for the asking corpus and its id for any other, and a corpus
    /// nobody asked about is admitted by neither.
    #[test]
    fn a_rows_corpus_is_named_only_when_it_is_not_this_one() {
        let wide = Corpora::across("abc123", &also(&["other9"]));
        assert_eq!(wide.foreign("abc123"), None);
        assert_eq!(wide.foreign("other9"), Some("other9".to_string()));
        assert!(wide.admits("abc123") && wide.admits("other9"));
        assert!(!wide.admits("third4"));
        // The witness's corpus is not a corpus, and no set admits it.
        assert!(!wide.admits(crate::s3vectors::META_CORPUS));
        assert!(!Corpora::own("abc123").admits(crate::s3vectors::META_CORPUS));
    }
}
