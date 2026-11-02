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

use std::collections::BTreeMap;

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
pub fn load(model: EmbeddingModel, runtime: &Runtime) -> Result<TextEmbedding> {
    self::runtime()?;
    Ok(TextEmbedding::try_new(runtime.options(model))?)
}

/// How ONNX Runtime is set up for one embedder: `[index] threads` and `session_options`.
///
/// The default is fastembed's own, which is what every query embeds with. Only `index-build`
/// reads the config (#1427): a build embeds every record at once, and a machine where that
/// stalls needs a way to change how it runs that does not need a new binary.
#[derive(Debug, Default, Clone)]
pub struct Runtime {
    pub threads: Option<usize>,
    pub session_options: BTreeMap<String, String>,
}

impl Runtime {
    pub(crate) fn from_config(index: &crate::config::IndexConfig) -> Self {
        Self {
            threads: index.threads,
            session_options: index.session_options.clone(),
        }
    }

    fn options(&self, model: EmbeddingModel) -> TextInitOptions {
        let mut options = TextInitOptions::new(model);
        if let Some(n) = self.threads {
            options = options.with_intra_threads(n);
        }
        for (key, value) in &self.session_options {
            options = options.with_session_config(key, value);
        }
        options
    }

    /// One line for `index-build` to print, so a slow build says what it was running with.
    pub fn describe(&self) -> String {
        let threads = match self.threads {
            None => "one per core".to_string(),
            Some(0) => "ONNX Runtime's choice".to_string(),
            Some(n) => n.to_string(),
        };
        let entries = self
            .session_options
            .iter()
            .map(|(k, v)| format!(", {k}={v}"))
            .collect::<String>();
        format!("threads: {threads}{entries}")
    }
}

/// The batch `index-build` uses when `[index] batch_size` names none: fastembed's own, so an
/// index built without the key is the one it would have been before the key existed.
pub const DEFAULT_BATCH_SIZE: usize = 256;

/// Embed `texts` a batch at a time, calling `progress(done, total)` after each.
///
/// The batches are `index-build`'s and not fastembed's for the progress alone: one
/// `embed` over every record printed nothing until it returned, and #1427's build ran 32
/// minutes looking exactly as stalled as it was. `embed` is the model's, passed in so the
/// batching can be tested without one.
pub fn embed_in_batches<E>(
    texts: &[String],
    batch_size: usize,
    mut embed: E,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<Vec<f32>>>
where
    E: FnMut(Vec<String>) -> Result<Vec<Vec<f32>>>,
{
    anyhow::ensure!(batch_size > 0, "`[index] batch_size` must be at least 1");
    let mut out = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(batch_size) {
        let got = embed(chunk.to_vec())?;
        anyhow::ensure!(
            got.len() == chunk.len(),
            "the model returned {} vector(s) for a batch of {}",
            got.len(),
            chunk.len()
        );
        out.extend(got);
        progress(out.len(), texts.len());
    }
    Ok(out)
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

    /// The config's keys reach the options fastembed builds the session from (#1427). A key
    /// that parsed and never got here is one a stalled machine would set and watch do nothing.
    #[test]
    fn the_config_reaches_the_session_options() {
        let index: crate::config::IndexConfig =
            toml::from_str("threads = 0\nsession_options = { \"mlas.disable_kleidiai\" = \"1\" }")
                .unwrap();
        let runtime = Runtime::from_config(&index);
        let options = runtime.options(EmbeddingModel::AllMiniLML6V2);
        assert_eq!(options.intra_threads, Some(0));
        assert_eq!(
            options.session_config,
            vec![("mlas.disable_kleidiai".to_string(), "1".to_string())]
        );
        assert_eq!(
            runtime.describe(),
            "threads: ONNX Runtime's choice, mlas.disable_kleidiai=1"
        );
    }

    /// Absent keys leave fastembed's defaults alone, so a corpus that sets nothing builds the
    /// index it built before.
    #[test]
    fn an_empty_config_is_fastembeds_default() {
        let options = Runtime::default().options(EmbeddingModel::AllMiniLML6V2);
        assert_eq!(options.intra_threads, None);
        assert!(options.session_config.is_empty());
    }

    /// Every text comes back once and in order, progress is reported after every batch, and a
    /// short final batch counts too.
    #[test]
    fn batches_cover_every_text_in_order() {
        let texts: Vec<String> = (0..7).map(|i| i.to_string()).collect();
        let mut sizes = Vec::new();
        let mut reported = Vec::new();
        let out = embed_in_batches(
            &texts,
            3,
            |batch| {
                sizes.push(batch.len());
                Ok(batch.iter().map(|t| vec![t.parse().unwrap()]).collect())
            },
            |done, total| reported.push((done, total)),
        )
        .unwrap();
        assert_eq!(sizes, [3, 3, 1]);
        assert_eq!(reported, [(3, 7), (6, 7), (7, 7)]);
        let order: Vec<f32> = out.iter().map(|v| v[0]).collect();
        assert_eq!(order, [0., 1., 2., 3., 4., 5., 6.]);
    }

    /// A zero batch would loop on nothing; it is refused by name instead.
    #[test]
    fn a_zero_batch_is_refused() {
        let e = embed_in_batches(&["a".into()], 0, |_| Ok(vec![]), |_, _| {}).unwrap_err();
        assert!(e.to_string().contains("batch_size"), "{e}");
    }

    /// A model answering a batch with the wrong number of vectors would misalign every row
    /// after it with its record.
    #[test]
    fn a_short_answer_is_an_error() {
        let e = embed_in_batches(
            &["a".into(), "b".into()],
            2,
            |_| Ok(vec![vec![0.]]),
            |_, _| {},
        )
        .unwrap_err();
        assert!(
            e.to_string().contains("1 vector(s) for a batch of 2"),
            "{e}"
        );
    }
}
