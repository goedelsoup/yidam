//! The vector retrieval path — the only part of `serve --mcp` that needs the ML stack.
//!
//! It lives in its own module so the light build never *names* `fastembed`. That is the
//! whole reason the file exists: `serve` used to be gated wholesale on the `index` feature
//! because [`IndexState`] held a `TextEmbedding`, so the one command that makes a corpus
//! reachable by an agent was in the build almost nobody installs. Everything else the
//! server does — reading a node, walking edges, listing a class, answering open questions —
//! was never index-dependent and is now compiled unconditionally.
//!
//! `super::keyword_retrieve` is the fallback, and it is not a stub: it spans installed
//! dependencies, labels each result with its `origin`, and reports `degraded: true` with a
//! reason. A light build answers every tool; only retrieval's *quality* differs, and it
//! says so on every call.

use serde_json::{json, Value};
use std::cell::RefCell;

use crate::model::VectorRow;

pub(crate) struct IndexState {
    pub rows: Vec<VectorRow>,
    pub model_id: String,
    /// Lazily initialised on the first `retrieve` call — loading model
    /// weights takes seconds and many sessions never call `retrieve`.
    pub embedder: RefCell<Option<fastembed::TextEmbedding>>,
}

pub(crate) fn retrieve(
    index: &IndexState,
    query: &str,
    k: usize,
    class_filter: Option<&str>,
) -> Result<Value, String> {
    let mut embedder = index.embedder.borrow_mut();
    if embedder.is_none() {
        let (model, _, _) = crate::cmd::index_build::resolve_model(&index.model_id)
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
        .filter(|r| class_filter.is_none_or(|c| r.class == c))
        .map(|r| {
            let score: f32 = r.vector.iter().zip(&query_vec).map(|(a, b)| a * b).sum();
            (r, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(k);

    Ok(json!({
        "degraded": false,
        // Present and null rather than absent, the same convention `origin` follows: a
        // client testing the key must not have to distinguish "not degraded" from "a
        // server too old to say why".
        "degraded_reason": Value::Null,
        "results": scored.iter().map(|(r, score)| json!({
            "path": r.path,
            "class": r.class,
            "label": r.label,
            "text": r.text,
            "score": score,
        })).collect::<Vec<_>>()
    }))
}
