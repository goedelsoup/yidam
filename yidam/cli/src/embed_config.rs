use serde::{Deserialize, Serialize};

/// Filename for the embedding reproducibility contract, written next to
/// `meta.json` in the index directory and carried into every index-bearing
/// export.
pub const EMBED_CONFIG_FILENAME: &str = "embed.config.json";

/// The embedding reproducibility contract.
///
/// Pins everything a consumer (browser agent, MCP server, sqlite-vec export)
/// needs to embed a query with the *same* settings the index was built with.
/// A silent pooling or normalization mismatch between runtimes degrades
/// retrieval without any error signal — this file is how consumers detect it.
///
/// A consumer that cannot satisfy this contract (e.g. the model is
/// unavailable in its runtime) must degrade to keyword search, not embed
/// with different settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbedConfig {
    /// Contract format version. Bumped only on breaking changes; consumers
    /// must ignore unknown fields.
    pub format_version: String,
    /// Hugging Face model id, resolvable by transformers.js and
    /// sentence-transformers (e.g. `Xenova/all-MiniLM-L6-v2`).
    pub model_id: String,
    /// Output vector dimensionality.
    pub embedding_dim: i32,
    /// ONNX weights file within the model repo (e.g.
    /// `onnx/model_quantized.onnx`). Consumers must load the same weights:
    /// quantized and fp32 exports of the same model differ by ~1e-3 per
    /// element, far beyond retrieval-safe tolerance.
    pub model_file: String,
    /// Token pooling strategy applied to the final hidden state.
    pub pooling: String,
    /// Whether output vectors are L2-normalized.
    pub normalize: bool,
    /// The fastembed `EmbeddingModel` enum variant used by the Rust
    /// reference implementation.
    pub fastembed_model_enum: String,
}

impl EmbedConfig {
    /// Build the contract for a fastembed-backed index.
    ///
    /// All fastembed text models used here apply mean pooling and L2
    /// normalization; if a model with different settings is ever added,
    /// this constructor must grow the corresponding parameters.
    pub fn for_fastembed_model(
        model_id: &str,
        embedding_dim: i32,
        model_file: &str,
        model_enum: &str,
    ) -> Self {
        Self {
            format_version: "1".to_string(),
            model_id: model_id.to_string(),
            embedding_dim,
            model_file: model_file.to_string(),
            pooling: "mean".to_string(),
            normalize: true,
            fastembed_model_enum: model_enum.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EmbedConfig {
        EmbedConfig::for_fastembed_model(
            "Xenova/all-MiniLM-L6-v2",
            384,
            "onnx/model_quantized.onnx",
            "AllMiniLML6V2Q",
        )
    }

    #[test]
    fn round_trips_through_json() {
        let cfg = sample();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let back: EmbedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn serializes_expected_fields() {
        let v: serde_json::Value = serde_json::to_value(sample()).unwrap();
        assert_eq!(v["format_version"], "1");
        assert_eq!(v["model_id"], "Xenova/all-MiniLM-L6-v2");
        assert_eq!(v["embedding_dim"], 384);
        assert_eq!(v["model_file"], "onnx/model_quantized.onnx");
        assert_eq!(v["pooling"], "mean");
        assert_eq!(v["normalize"], true);
        assert_eq!(v["fastembed_model_enum"], "AllMiniLML6V2Q");
    }
}
