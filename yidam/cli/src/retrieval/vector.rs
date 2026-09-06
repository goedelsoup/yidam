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
    /// The index's reproducibility contract, carrying the witness [`search`] checks itself
    /// against. `None` for indexes built before the block existed — see [`Verdict::Unverifiable`].
    pub embed_config: Option<crate::embed_config::EmbedConfig>,
    /// Lazily initialised on the first search — loading model weights takes seconds and many
    /// sessions never search at all.
    pub embedder: RefCell<Option<fastembed::TextEmbedding>>,
    /// The witness verdict, computed once beside the embedder and reused after.
    pub space: RefCell<Option<crate::embed_config::Verdict>>,
}

/// What a search found, or why it could not be trusted to find it.
///
/// Not a `Result`: a space mismatch is not a failure of the search, it is the search being
/// the wrong instrument. Both call sites already carry a keyword arm — `retrieve` falls
/// through to `keyword_retrieve` and an anchored step to `keyword_entries` — so the honest
/// answer is the one they already give when there is no index at all, with its own reason.
pub(crate) enum Searched<'a> {
    Hits(Vec<(&'a VectorRow, f32)>),
    /// This binary embeds into a different space than the index was built in.
    SpaceMismatch,
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
) -> Result<Searched<'a>, String> {
    let mut embedder = index.embedder.borrow_mut();
    if embedder.is_none() {
        let (model, _, _) = crate::embedding::resolve_model(&index.model_id)
            .map_err(|e| format!("resolving embedding model: {e}"))?;
        let loaded = fastembed::TextEmbedding::try_new(fastembed::InitOptions::new(model))
            .map_err(|e| format!("loading embedding model {}: {e}", index.model_id))?;

        // The witness, checked here and not by a command someone remembers to run.
        //
        // `yidam index-verify` has always been able to answer this and has never been on
        // anyone's path: the query path embedded and scored without ever consulting the block
        // written for exactly this. So an upgrade that moved the vector space — fastembed 6
        // takes `ort` rc.13, whose ONNX Runtime is four versions newer and whose quantized
        // kernels answer differently — degraded rankings silently, with no error and no
        // warning (#536).
        //
        // One extra embed, on the path that has just paid seconds to load the model, and only
        // for the sessions that search at all. `runtime: None` is deliberate: `known_delta`
        // names runtimes that cannot load these weights, and this one just did.
        let verdict = match &index.embed_config {
            Some(config) => {
                let probe = loaded
                    .embed(
                        vec![crate::embed_config::VERIFICATION_PROBE.to_string()],
                        None,
                    )
                    .map_err(|e| format!("embedding the verification probe: {e}"))?
                    .remove(0);
                crate::embed_config::verify(config, &probe, None).verdict
            }
            // No contract to check against. Every index built before the block existed is
            // here, and failing closed would break all of them at once.
            None => crate::embed_config::Verdict::Unverifiable,
        };
        *index.space.borrow_mut() = Some(verdict);
        *embedder = Some(loaded);
    }

    if matches!(
        index.space.borrow().as_ref(),
        Some(crate::embed_config::Verdict::Mismatch)
    ) {
        return Ok(Searched::SpaceMismatch);
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
    Ok(Searched::Hits(scored))
}
