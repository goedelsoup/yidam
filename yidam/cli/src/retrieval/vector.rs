//! The vector path — the only part of yidam that needs the ML stack.
//!
//! It lives in its own module so the light build never *names* `fastembed`. That is the
//! whole reason the file exists: `serve` used to be gated wholesale on the `index` feature
//! because [`IndexState`] held a `TextEmbedding`, so the one command that makes a corpus
//! reachable by an agent was in the build almost nobody installs. Everything else the server
//! does — reading a node, walking edges, listing a class, answering open questions, and now
//! executing a typed query — was never index-dependent and is compiled unconditionally.
//!
//! [`search`] returns *scores*, not a response. It used to return the MCP `retrieve` body,
//! which put half the `degraded` convention — the present-and-null half — inside the one
//! module the degradable build does not compile. Shaping happens at each call site now, so
//! both branches of that convention live where both are reachable.

use std::cell::RefCell;

use crate::model::VectorRow;

pub(crate) struct IndexState {
    pub rows: Vec<VectorRow>,
    pub model_id: String,
    /// Lazily initialised on the first search — loading model weights takes seconds and many
    /// sessions never search at all.
    pub embedder: RefCell<Option<fastembed::TextEmbedding>>,
}

/// The top `k` rows `keep` admits, by cosine similarity, highest first.
///
/// `keep` rather than a class name: `retrieve` filters on at most one class, and a query's
/// anchor filters on the classes its step narrowed to *and* on the row resolving to a node
/// this repository owns. A single `Option<&str>` could express the first and not the second.
pub(crate) fn search<'a>(
    index: &'a IndexState,
    query: &str,
    k: usize,
    keep: impl Fn(&VectorRow) -> bool,
) -> Result<Vec<(&'a VectorRow, f32)>, String> {
    let mut embedder = index.embedder.borrow_mut();
    if embedder.is_none() {
        let (model, _, _) = crate::embedding::resolve_model(&index.model_id)
            .map_err(|e| format!("resolving embedding model: {e}"))?;
        let loaded = fastembed::TextEmbedding::try_new(fastembed::InitOptions::new(model))
            .map_err(|e| format!("loading embedding model {}: {e}", index.model_id))?;
        *embedder = Some(loaded);
    }
    let query_vec = embedder
        .as_ref()
        .expect("embedder initialised above")
        .embed(vec![query.to_string()], None)
        .map_err(|e| format!("embedding query: {e}"))?
        .remove(0);

    // Index vectors are L2-normalized (see embed.config.json), so cosine
    // similarity reduces to the dot product.
    let mut scored: Vec<(&VectorRow, f32)> = index
        .rows
        .iter()
        .filter(|r| keep(r))
        .map(|r| {
            let score: f32 = r.vector.iter().zip(&query_vec).map(|(a, b)| a * b).sum();
            (r, score)
        })
        .collect();
    // Ties break on the path, not on index order: two rows at the same score must come back
    // in the same order on every run, or a golden that pins an entry node is pinning the
    // Arrow file's row layout.
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.path.cmp(&b.0.path)));
    scored.truncate(k);
    Ok(scored)
}
