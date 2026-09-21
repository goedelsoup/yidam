//! Answering a query out of a vector bucket instead of a file.
//!
//! # What is remote and what is not
//!
//! The vectors. Not the model: the query text is still embedded here, by this process, with
//! `fastembed` — which is why this module is behind `vector-read` exactly like [`super::vector`]
//! is. A build that cannot embed cannot query a remote index any more than a local one, and
//! the light binary's `no_vector_support` answer is unchanged by every line of this.
//!
//! # The witness, over the wire
//!
//! A local index carries `embed.config.json` beside its rows. A remote one carries the same
//! document as the metadata of a reserved record, and it is fetched on the same first-search
//! path that loads the model — one extra round trip, in a call that has just paid seconds to
//! load weights, and only for sessions that search at all.
//!
//! When that fetch fails, the verdict is `Unverifiable` rather than an error: a contract that
//! could not be read is not a contract that disagrees, and the alternative is a corpus whose
//! retrieval stops working because one `GetVectors` was throttled.

use std::cell::RefCell;

use super::{Filter, Hit, Searched};
use crate::embed_config::{EmbedConfig, Verdict};
use crate::s3vectors::{ops, transport::Client, RemoteIndex};

pub(crate) struct RemoteState {
    pub index: RemoteIndex,
    /// The short genesis hash every key is prefixed with, and every query filters on.
    pub corpus: String,
    pub model_id: String,
    /// The transport. Built at load time so a missing credential is reported when the server
    /// starts rather than on the first question somebody asks it.
    pub client: Client,
    /// Lazily initialised on the first search — loading model weights takes seconds and many
    /// sessions never search at all.
    pub embedder: RefCell<Option<fastembed::TextEmbedding>>,
    /// The witness verdict, computed once beside the embedder and reused after.
    pub space: RefCell<Option<Verdict>>,
    /// Whether the witness says this index's rows carry the class their path derives.
    ///
    /// Beside [`Self::space`] and for the same reason: it is read off the same document, on
    /// the same first-search fetch. `false` until that fetch has happened, which is the
    /// honest state — an index that has told this process nothing has not told it yes — and
    /// the filter is applied after it, never before.
    pub classes_are_path_derived: std::cell::Cell<bool>,
}

/// The top `k` rows the filter admits, highest similarity first.
///
/// The shape is [`super::vector::search`]'s exactly, because both feed the same two call sites
/// and a caller that had to know which backend it was talking to would be a third place for
/// the `degraded` convention to live.
pub(crate) fn search(
    state: &RemoteState,
    query: &str,
    k: usize,
    filter: &Filter,
    residual: impl Fn(&Hit) -> bool,
) -> Result<Searched, String> {
    let session = ops::Session::new(&state.client);

    let mut embedder = state.embedder.borrow_mut();
    if embedder.is_none() {
        let (model, _, _) = crate::embedding::resolve_model(&state.model_id)
            .map_err(|e| format!("resolving embedding model: {e}"))?;
        let loaded = fastembed::TextEmbedding::try_new(fastembed::InitOptions::new(model))
            .map_err(|e| format!("loading embedding model {}: {e}", state.model_id))?;

        let contract = witness_contract(&session, state);
        let verdict = match &contract {
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
            // No contract that could be read. An index pushed before the witness existed is
            // here, and so is one whose `GetVectors` was throttled — neither is a contract
            // that *disagrees*, which is the only thing that should stop a search.
            None => Verdict::Unverifiable,
        };
        state
            .classes_are_path_derived
            .set(contract.is_some_and(|c| c.classes_are_path_derived()));
        *state.space.borrow_mut() = Some(verdict);
        *embedder = Some(loaded);
    }

    if matches!(state.space.borrow().as_ref(), Some(Verdict::Mismatch)) {
        return Ok(Searched::SpaceMismatch);
    }

    let query_vec = embedder
        .as_ref()
        .ok_or("the embedder was initialised above and is missing")?
        .embed(vec![query.to_string()], None)
        .map_err(|e| format!("embedding query: {e}"))?
        .remove(0);

    // Asked of the witness that was just fetched, and not by the caller: a remote index does
    // not know its own contract until then, and the caller built this filter before the first
    // round trip had happened.
    let filter = filter.as_applied(state.classes_are_path_derived.get());

    match ops::query(
        &session,
        &state.index,
        &state.corpus,
        &query_vec,
        &filter,
        residual,
        k,
    ) {
        Ok(hits) => Ok(Searched::Hits(hits)),
        // Not an `Err`: the caller has a keyword arm and this question still has an answer.
        // It is a worse answer, and it says so — see `super::REMOTE_UNAVAILABLE`.
        Err(failure) => Ok(Searched::Unavailable(failure.message)),
    }
}

/// The contract the index carries, or `None` when it carries none this call could read.
fn witness_contract(session: &ops::Session, state: &RemoteState) -> Option<EmbedConfig> {
    let fetched = ops::fetch_witness(session, &state.index).ok()??;
    let encoded = fetched
        .metadata
        .get(crate::s3vectors::META_KEY_EMBED_CONFIG)?
        .as_str()?;
    serde_json::from_str(encoded).ok()
}
