//! Naming an embedding model, in a build that may not be able to create an index.
//!
//! `resolve_model` lived in `cmd/index_build.rs`, which is gated on `index` because it needs
//! `lancedb` and therefore protoc. The read path needs the same function — embedding a query
//! means loading the same model the index was built with — and needs none of that.
//!
//! So it moves, and the move is the one `deps.rs` already made:
//!
//! > Commit `99713ec` moved `load_lock` and `sha256_hex` out of `cmd/tonpa/` and into
//! > `deps.rs`, because `doctor` must read a lock in a build where `cmd::tonpa` does not
//! > exist — the feature buys the *network*, and reading a file and hashing it are not
//! > network operations.
//!
//! Here the feature buys the *build*, and naming a model is not building one.

use anyhow::Result;
use fastembed::{EmbeddingModel, TextEmbedding};

/// The model an index is built with when `.yidam/config.toml` names none.
pub const DEFAULT_MODEL: &str = "Xenova/all-MiniLM-L6-v2";

/// The model this name refers to, its dimension, and its weights file.
///
/// The error lists every model the linked `fastembed` supports, because the set is a property
/// of the build rather than of anything a person can look up — a name that worked against one
/// release can be gone in the next, and a bare "unknown model" would send the reader to the
/// wrong documentation.
pub fn resolve_model(name: &str) -> Result<(EmbeddingModel, i32, String)> {
    TextEmbedding::list_supported_models()
        .into_iter()
        .find(|m| m.model_code == name)
        .map(|m| (m.model, m.dim as i32, m.model_file))
        .ok_or_else(|| {
            let list = TextEmbedding::list_supported_models()
                .into_iter()
                .map(|m| format!("  {}", m.model_code))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::anyhow!("unknown model {name:?}\n\nSupported models:\n{list}")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default the config falls back to has to be one this build can actually resolve, or
    /// every index build and every query fails on a name nobody chose.
    #[test]
    fn the_default_model_resolves_in_this_build() {
        assert!(resolve_model(DEFAULT_MODEL).is_ok());
    }

    /// An unknown name lists what is available. The set depends on the linked `fastembed`, so
    /// a reader cannot look it up anywhere but here.
    #[test]
    fn an_unknown_model_names_the_ones_that_exist() {
        let e = resolve_model("not-a-model").unwrap_err().to_string();
        assert!(e.contains("unknown model"), "{e}");
        assert!(e.contains(DEFAULT_MODEL), "lists a real one: {e}");
    }
}
