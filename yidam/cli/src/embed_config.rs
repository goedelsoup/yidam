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
    /// A witness a consumer can reproduce.
    ///
    /// Optional because every index built before this field existed has none, and an
    /// unverifiable index is not a wrong one — see [`Verdict::Unverifiable`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<Verification>,
}

/// The sentence the contract is witnessed on.
///
/// The same probe the parity fixture uses, so the numbers written into an index and the
/// numbers CI compares across three runtimes are the same numbers. A test asserts they have
/// not come apart.
pub const VERIFICATION_PROBE: &str = "knowledge graph traversal";

/// How many leading dimensions are carried.
///
/// Eight, matching the parity fixture. Enough that a different vector space cannot agree by
/// coincidence, few enough that the block stays readable in a committed file — the whole
/// 384 would make `embed.config.json` something nobody opens.
pub const VERIFICATION_PREFIX_DIMS: usize = 8;

/// Agreement required of a consumer that loaded the same weights.
///
/// The parity fixture's `[expected] tolerance`. A test asserts they have not diverged.
pub const VERIFICATION_TOLERANCE: f32 = 1e-5;

/// Runtimes that provably cannot load the quantized weights, and the drift they measure.
///
/// `sentence-transformers` runs the fp32 PyTorch export — same model, different precision,
/// elements up to ~8e-3 apart on the probe. Carried into every index because the consumer
/// who needs it holds an index directory and nothing else; the parity fixture that measured
/// it lives in CI and never travels.
pub const KNOWN_DELTAS: &[(&str, f32)] = &[("sentence-transformers", 1e-2)];

/// A reproducible witness of the vector space an index was built in.
///
/// The settings above pin what a consumer *should* do. This is what lets it find out whether
/// it did. Without it, a consumer embedding with different weights gets plausible cosine
/// scores that are quietly wrong, and nothing in the running system can notice — the parity
/// fixture that proves cross-runtime agreement lives in CI and never travels with an index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verification {
    /// The sentence embedded to produce `prefix`.
    pub probe: String,
    /// Length of `prefix`.
    pub prefix_dims: usize,
    /// The first `prefix_dims` elements of the normalized embedding of `probe`.
    pub prefix: Vec<f32>,
    /// Per-element agreement required of a consumer loading the same weights.
    pub tolerance: f32,
    /// Runtimes that provably cannot load `model_file`, and the drift they measure.
    ///
    /// A declared, bounded difference is the honest outcome for a runtime with no access to
    /// the exact weights — `sentence-transformers` runs fp32 where this index is quantized.
    /// The alternative on offer is not "close enough"; it is degrade to keyword search.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub known_delta: std::collections::BTreeMap<String, KnownDelta>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnownDelta {
    pub tolerance: f32,
}

/// What a verification run concluded.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Same space, within `tolerance`.
    Match,
    /// Different space, within a bound that runtime declared in advance.
    ///
    /// Passes, and says so out loud with the measured drift: an expected degradation named
    /// is a different thing from an unnoticed one, and this is the whole point of the block.
    KnownDelta { runtime: String, tolerance: f32 },
    /// Beyond every declared bound, or the wrong shape entirely.
    Mismatch,
    /// The index carries no witness. Every index built before this field existed is here.
    ///
    /// Not a failure. A consumer cannot be blamed for an index that gave it nothing to check
    /// against, and failing closed would break every existing index at once.
    Unverifiable,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub verdict: Verdict,
    /// Largest per-element difference from `prefix`, or None when nothing was compared.
    pub max_drift: Option<f32>,
    /// What is wrong, when something is.
    pub problems: Vec<String>,
}

impl Outcome {
    /// Whether the consumer may query this index.
    pub fn passed(&self) -> bool {
        !matches!(self.verdict, Verdict::Mismatch)
    }
}

/// Check a provider's embedding of the probe against the index's witness.
///
/// `runtime` names the consumer, so a declared `known_delta` can apply to it. A consumer that
/// does not name itself is held to `tolerance` — which is correct: an unnamed runtime has not
/// declared anything, and inferring a bound for it would be inventing the permission the
/// block exists to make explicit.
pub fn verify(config: &EmbedConfig, vector: &[f32], runtime: Option<&str>) -> Outcome {
    let Some(v) = &config.verification else {
        return Outcome {
            verdict: Verdict::Unverifiable,
            max_drift: None,
            problems: vec![format!(
                "this index carries no `verification` block — rebuild it with a yidam that                  writes one to make {} checkable",
                config.model_id
            )],
        };
    };

    let mut problems = Vec::new();
    if vector.len() != config.embedding_dim as usize {
        problems.push(format!(
            "provider returned {} dimensions; the index is {}",
            vector.len(),
            config.embedding_dim
        ));
    }
    if config.normalize {
        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        // Loose: this catches an un-normalized provider, not float noise.
        if !vector.is_empty() && (norm - 1.0).abs() > 1e-3 {
            problems.push(format!(
                "the index stores L2-normalized vectors and the provider's has norm {norm:.4}"
            ));
        }
    }
    if !problems.is_empty() {
        return Outcome {
            verdict: Verdict::Mismatch,
            max_drift: None,
            problems,
        };
    }

    let compared = v.prefix.len().min(vector.len());
    let max_drift = (0..compared)
        .map(|i| (vector[i] - v.prefix[i]).abs())
        .fold(0.0f32, f32::max);

    if max_drift <= v.tolerance {
        return Outcome {
            verdict: Verdict::Match,
            max_drift: Some(max_drift),
            problems: vec![],
        };
    }
    if let Some(name) = runtime {
        if let Some(delta) = v.known_delta.get(name) {
            if max_drift <= delta.tolerance {
                return Outcome {
                    verdict: Verdict::KnownDelta {
                        runtime: name.to_string(),
                        tolerance: delta.tolerance,
                    },
                    max_drift: Some(max_drift),
                    problems: vec![],
                };
            }
        }
    }
    // Built in pieces rather than one wrapped literal: a `\`-continued string is one
    // reflow away from carrying the indentation into the message, and this message is the
    // one a consumer reads when its retrieval is silently wrong.
    let allowed = match runtime.and_then(|r| v.known_delta.get(r).map(|d| (r, d))) {
        Some((r, d)) => format!("{:.0e} for `{r}`", d.tolerance),
        None => format!("{:.0e}", v.tolerance),
    };
    Outcome {
        verdict: Verdict::Mismatch,
        max_drift: Some(max_drift),
        problems: vec![format!(
            "the provider embeds {max_drift:.2e} away from this index's space; the contract allows {allowed}. This is a different vector space: degrade to keyword search rather than querying it."
        )],
    }
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
            verification: None,
        }
    }

    /// Attach the witness, from an embedding of [`VERIFICATION_PROBE`].
    pub fn with_verification(mut self, probe_vector: &[f32]) -> Self {
        self.verification = Some(Verification {
            probe: VERIFICATION_PROBE.to_string(),
            prefix_dims: VERIFICATION_PREFIX_DIMS,
            prefix: probe_vector
                .iter()
                .take(VERIFICATION_PREFIX_DIMS)
                .copied()
                .collect(),
            tolerance: VERIFICATION_TOLERANCE,
            known_delta: KNOWN_DELTAS
                .iter()
                .map(|(name, tolerance)| {
                    (
                        (*name).to_string(),
                        KnownDelta {
                            tolerance: *tolerance,
                        },
                    )
                })
                .collect(),
        });
        self
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
