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
    /// How the `class` on every row of this index was derived, when the index can vouch for it.
    ///
    /// **Not an embedding setting, and it is here anyway.** Everything else on this struct
    /// pins how to embed a query. This pins how to read a row's *metadata* — which is a
    /// different question with the same audience and the same distribution problem: it has to
    /// reach a consumer holding an index and nothing else, including one holding it through a
    /// vector bucket, where this document is the only thing that travels. A second travelling
    /// contract for one string would be a second thing to forget to carry.
    ///
    /// [`CLASS_SOURCE_PARENT_DIRECTORY`] is the only value, and `None` means the index makes
    /// no claim: every index built before this field existed is there, and so is one whose
    /// rows were found not to satisfy it. A reader that needs the claim treats `None` as "no",
    /// which costs it a wider search and never a wrong answer — see
    /// [`crate::retrieval::Filter::as_applied`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_source: Option<String>,
}

/// A row's `class` is the name of the directory its `path` sits in — [`crate::paths::class_of_path`].
///
/// The only value [`EmbedConfig::class_source`] takes today. It is a *claim about the rows*,
/// checked against every one of them when the index is built, and not a statement about which
/// binary built it: a derivation that changes takes a new value here, and every index carrying
/// the old one goes on being read correctly by the readers that understand it.
pub const CLASS_SOURCE_PARENT_DIRECTORY: &str = "parent-directory";

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
            // Not claimed by a constructor. [`EmbedConfig::class_source`] is a claim about
            // rows, and this function has never seen one — `index_build` attaches it after
            // checking every record, the way [`Self::with_verification`] attaches a witness
            // after embedding the probe.
            class_source: None,
        }
    }

    /// Declare that every row of this index carries the class its own path derives.
    ///
    /// Takes the checked flag rather than doing the check, so the caller holding the records
    /// is the one that answers, and a corpus that fails it produces a contract saying nothing
    /// rather than one saying the wrong thing.
    pub fn with_class_source(mut self, path_derived: bool) -> Self {
        self.class_source = path_derived.then(|| CLASS_SOURCE_PARENT_DIRECTORY.to_string());
        self
    }

    /// Whether this index vouches for its rows' `class` being their path's parent directory.
    ///
    /// An unrecognised value is a "no", not an error: a contract written by a later yidam
    /// naming a derivation this binary does not implement is exactly the case where a reader
    /// must not assume.
    pub fn classes_are_path_derived(&self) -> bool {
        self.class_source.as_deref() == Some(CLASS_SOURCE_PARENT_DIRECTORY)
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

/// Why two contracts are not the same vector space, or `None` when they are.
///
/// **Document against document, with no embedder in the room.** [`verify`] answers the other
/// form of this question — *is this consumer in the index's space* — and needs a vector a
/// consumer produced. This one is asked by `yidam index-push`, which loads no model and has
/// no vector to offer: what it holds is the contract its own index carries and the contract
/// the remote index already carries, and comparing those is enough to catch the case that
/// matters.
///
/// **The case that matters is one vector index holding two spaces.** A vector index's
/// dimension is fixed at creation, so two corpora with *different* dimensions cannot share one
/// — that is refused by the service and by `response::disagreement` before it. Two corpora
/// with the same dimension and different models can, and eleven of the thirty models
/// `fastembed` offers produce a dimension some other model also produces (RFC-0033 §8.1). The
/// index carries exactly one witness, so the second push would overwrite the first's contract
/// and a query spanning both would rank one corpus's rows against the other's space — with
/// scores in the range a correct ranking has. It is #536's failure, arriving from inside an
/// index rather than from a binary upgrade.
///
/// The fields compared are the ones that decide what a vector *is*: the model, the weights
/// file, the pooling, the normalization and the dimension. `fastembed_model_enum` is not among
/// them — it names the Rust reference implementation's variant, so a consumer in another
/// language would differ on it while embedding identically — and neither is `format_version`,
/// which is the document's own shape. Where both documents carry a witness the probe vectors
/// are compared too, under the tolerance the *existing* index declared, because that is the
/// one check that cannot be satisfied by two documents agreeing about the wrong weights.
pub fn space_disagreement(existing: &EmbedConfig, ours: &EmbedConfig) -> Option<String> {
    let mut differs = Vec::new();
    let mut field = |name: &str, a: &str, b: &str| {
        if a != b {
            differs.push(format!("{name} {a} vs {b}"));
        }
    };
    field("model", &existing.model_id, &ours.model_id);
    field("weights", &existing.model_file, &ours.model_file);
    field("pooling", &existing.pooling, &ours.pooling);
    field(
        "normalize",
        &existing.normalize.to_string(),
        &ours.normalize.to_string(),
    );
    field(
        "dimension",
        &existing.embedding_dim.to_string(),
        &ours.embedding_dim.to_string(),
    );

    if let (Some(a), Some(b)) = (&existing.verification, &ours.verification) {
        // Only where the two witnessed the *same* sentence. A probe that differs is not a
        // disagreement about the space; it is two documents answering different questions,
        // and comparing them would manufacture a conflict out of a format change.
        if a.probe == b.probe {
            let compared = a.prefix.len().min(b.prefix.len());
            let drift = (0..compared)
                .map(|i| (a.prefix[i] - b.prefix[i]).abs())
                .fold(0.0f32, f32::max);
            if compared > 0 && drift > a.tolerance {
                differs.push(format!(
                    "the witness probe embeds {drift:e} apart, against a declared tolerance of {:e}",
                    a.tolerance
                ));
            }
        }
    }

    (!differs.is_empty()).then(|| differs.join("; "))
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

    // ── one index, one vector space ───────────────────────────────────────────

    /// Two indexes built the same way agree, witness and all.
    #[test]
    fn the_same_contract_is_the_same_space() {
        let a = sample().with_verification(&[0.1, 0.2, 0.3]);
        assert_eq!(space_disagreement(&a, &a.clone()), None);
        // And without a witness on either side, which is every index built before the block
        // existed: the declared settings are what there is to compare, and they agree.
        assert_eq!(space_disagreement(&sample(), &sample()), None);
    }

    /// A different model at the same dimension is the case that matters.
    ///
    /// **The dimension would not have caught it.** Eleven of `fastembed`'s thirty models
    /// produce a dimension another model also produces (RFC-0033 §8.1), and a vector index
    /// fixes its dimension at creation — so two corpora in one index, one ranked in the
    /// other's space, is a state the service itself would accept.
    #[test]
    fn another_model_at_the_same_dimension_is_another_space() {
        let ours = EmbedConfig::for_fastembed_model(
            "BAAI/bge-small-en-v1.5",
            384,
            "onnx/model.onnx",
            "BGESmallENV15",
        );
        let why = space_disagreement(&sample(), &ours).expect("two models, one dimension");
        assert!(why.contains("model"), "{why}");
        assert!(why.contains("bge-small-en-v1.5"), "{why}");
    }

    /// The quantized-versus-fp32 case: one model, one dimension, different weights.
    ///
    /// The drift `index-verify` exists for — ~1e-3 per element, far outside retrieval-safe
    /// tolerance and nowhere near far enough to look broken.
    #[test]
    fn the_same_model_with_other_weights_is_another_space() {
        let fp32 = EmbedConfig::for_fastembed_model(
            "Xenova/all-MiniLM-L6-v2",
            384,
            "onnx/model.onnx",
            "AllMiniLML6V2",
        );
        let why = space_disagreement(&sample(), &fp32).expect("two weights files");
        assert!(why.contains("weights"), "{why}");
    }

    /// Two documents that declare the same settings and embed the probe differently disagree.
    ///
    /// This is the check the declared fields cannot make: a contract can be copied correctly
    /// and still describe a runtime that produces other numbers, which is #536's whole
    /// subject. The tolerance applied is the *existing* index's, because it is the one being
    /// written into.
    #[test]
    fn a_witness_that_embeds_the_probe_differently_is_another_space() {
        let theirs = sample().with_verification(&[0.1, 0.2, 0.3]);
        let ours = sample().with_verification(&[0.1, 0.9, 0.3]);
        let why = space_disagreement(&theirs, &ours).expect("the probe embeds differently");
        assert!(why.contains("witness probe"), "{why}");

        // Float noise is not a disagreement: the declared tolerance is what decides.
        let near = sample().with_verification(&[0.1, 0.2 + 1e-7, 0.3]);
        assert_eq!(space_disagreement(&theirs, &near), None);
    }

    /// A witness of a *different sentence* is not compared at all.
    ///
    /// Two documents answering different questions are not two answers to one, and comparing
    /// them would manufacture a conflict out of a format change.
    #[test]
    fn witnesses_of_different_probes_are_not_compared() {
        let mut theirs = sample().with_verification(&[0.1, 0.2, 0.3]);
        theirs.verification.as_mut().unwrap().probe = "another sentence".to_string();
        let ours = sample().with_verification(&[0.9, 0.9, 0.9]);
        assert_eq!(space_disagreement(&theirs, &ours), None);
    }

    /// `class_source` is not a claim about the space, and a difference in it is not a refusal.
    ///
    /// It rides on this document for distribution rather than because it is an embedding
    /// setting (see the field's own note), and a push refused over it would be a push refused
    /// over how a row's `class` was derived — which changes no vector.
    #[test]
    fn a_claim_about_class_metadata_is_not_a_claim_about_the_space() {
        let mut theirs = sample();
        theirs.class_source = Some(CLASS_SOURCE_PARENT_DIRECTORY.to_string());
        assert_eq!(space_disagreement(&theirs, &sample()), None);
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

    /// An index that has not been checked declares nothing, and the document it writes is the
    /// one it wrote before the field existed.
    ///
    /// That matters twice. A reader holding it cannot mistake silence for a claim, and a
    /// corpus whose records *do* satisfy the rule produces a byte-identical
    /// `embed.config.json` to the one it produced yesterday — so the field appearing in a diff
    /// is a fact about the index, not about the yidam that built it.
    #[test]
    fn an_index_that_cannot_vouch_writes_no_class_source_at_all() {
        let cfg = sample().with_class_source(false);
        assert!(!cfg.classes_are_path_derived());
        let v: serde_json::Value = serde_json::to_value(&cfg).unwrap();
        assert!(
            v.get("class_source").is_none(),
            "a contract that claims nothing must not carry the key: {v}"
        );
    }

    #[test]
    fn an_index_that_checked_its_records_declares_the_derivation_by_name() {
        let cfg = sample().with_class_source(true);
        assert!(cfg.classes_are_path_derived());
        let v: serde_json::Value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(v["class_source"], CLASS_SOURCE_PARENT_DIRECTORY);
    }

    /// Every index built before this field existed reads as "no claim" rather than failing to
    /// parse — the whole population on disk today is in that state.
    #[test]
    fn a_contract_written_before_the_field_existed_still_parses_and_claims_nothing() {
        let older = serde_json::json!({
            "format_version": "1",
            "model_id": "Xenova/all-MiniLM-L6-v2",
            "embedding_dim": 384,
            "model_file": "onnx/model_quantized.onnx",
            "pooling": "mean",
            "normalize": true,
            "fastembed_model_enum": "AllMiniLML6V2Q"
        });
        let cfg: EmbedConfig = serde_json::from_value(older).unwrap();
        assert_eq!(cfg.class_source, None);
        assert!(!cfg.classes_are_path_derived());
    }

    /// A derivation this binary does not implement is a "no", not a parse error and not a
    /// yes. It is what a contract written by a *later* yidam looks like from here, and
    /// assuming it would be the one case where the push-down is wrong in a way no residual
    /// catches.
    #[test]
    fn a_derivation_this_binary_does_not_implement_is_not_vouched_for() {
        let mut cfg = sample();
        cfg.class_source = Some("some-later-rule".to_string());
        assert!(!cfg.classes_are_path_derived());
    }
}
