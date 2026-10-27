//! Naming an embedding model, in a build that may not be able to create an index.
//!
//! `resolve_model` lived in `cmd/index_build.rs`, which was gated on `index` because it needed
//! `lancedb` and therefore protoc. The read path needs the same function — embedding a query
//! means loading the same model the index was built with — and needed none of that. (#1287
//! later removed lancedb, and `index_build` is now in `vector-read` too.)
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
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

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

/// Load `model` to embed with, or say why this machine cannot.
///
/// Every embedder in the binary is made here, so the one target that links ONNX Runtime at run
/// time rather than at build time — x86_64-apple-darwin, for which `ort` publishes no prebuilt
/// (see the per-target tables in Cargo.toml) — has one place to check for it. `ort` *panics*
/// when that library is missing; checking first turns the panic into an error, which `retrieve`
/// already degrades on and `index-build` already reports.
pub fn load(model: EmbeddingModel) -> Result<TextEmbedding> {
    runtime()?;
    Ok(TextEmbedding::try_new(TextInitOptions::new(model))?)
}

/// Every target but one links ONNX Runtime statically, so there is nothing to find.
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn runtime() -> Result<()> {
    Ok(())
}

/// Load `libonnxruntime.dylib` the way `ort` would, through `ort`'s own fallible entry point.
///
/// `ort::init_from` fills the same once-cell that `ort` later reads instead of loading the
/// library itself, so a success here is the load fastembed uses, and a failure is returned
/// rather than becoming `ort`'s `expect`. Its version check is the one that matters: fastembed
/// asks for ONNX Runtime API 24, so an older library is refused here by name.
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn runtime() -> Result<()> {
    let path = std::env::var("ORT_DYLIB_PATH")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "libonnxruntime.dylib".to_string());
    match ort::init_from(&path) {
        Ok(_environment) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(
            "no usable ONNX Runtime at {path:?} ({e}). On an Intel Mac yidam loads it at run \
             time: `brew install onnxruntime`, or set ORT_DYLIB_PATH to a libonnxruntime.dylib \
             of version 1.24 or newer"
        )),
    }
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
