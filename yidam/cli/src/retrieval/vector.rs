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
use crate::retrieval::{Hit, Searched};

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

/// The top `k` rows the filter admits, by cosine similarity, highest first.
///
/// `filter` is the pushable half of the question and `residual` the rest — see
/// [`crate::retrieval::Filter`]. A local scan can apply both without distinction, and does not:
/// keeping them separate here is what lets the two backends be held to one reading of a
/// filter, and what makes `retrieve`'s class test and an anchored step's ownership test
/// different kinds of thing in both.
pub(crate) fn search(
    index: &IndexState,
    query: &str,
    k: usize,
    filter: &crate::retrieval::Filter,
    residual: impl Fn(&Hit) -> bool,
) -> Result<Searched, String> {
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
        .ok_or("the embedder was initialised above and is missing")?
        .embed(vec![query.to_string()], None)
        .map_err(|e| format!("embedding query: {e}"))?
        .remove(0);

    // What this index will actually apply. A filter naming *nodes* is a claim about a
    // derivation the index has to vouch for — see `crate::retrieval::Filter::as_applied`.
    // A local scan pays nothing for the wider read, so a "no" only ever costs a remote index
    // a round trip; it is asked here anyway, because one reading of a filter across both
    // backends is what stops them disagreeing about what one means.
    let filter = filter.as_applied(
        index
            .embed_config
            .as_ref()
            .is_some_and(|c| c.classes_are_path_derived()),
    );

    // Index vectors are L2-normalized (see embed.config.json), so cosine
    // similarity reduces to the dot product.
    let hits: Vec<Hit> = index
        .rows
        .iter()
        .filter(|r| filter.admits(&r.class))
        .map(|r| Hit {
            path: r.path.clone(),
            class: r.class.clone(),
            label: r.label.clone(),
            text: r.text.clone(),
            score: r.vector.iter().zip(&query_vec).map(|(a, b)| a * b).sum(),
        })
        .collect();

    // Ordered and cut by the same function the remote backend uses. Ties break on the path,
    // not on index order: two rows at the same score must come back in the same order on every
    // run, or a golden that pins an entry node is pinning the Arrow file's row layout.
    Ok(Searched::Hits(crate::s3vectors::response::finish(
        hits, residual, k,
    )))
}
