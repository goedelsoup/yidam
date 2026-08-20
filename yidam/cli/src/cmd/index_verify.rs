//! `yidam index-verify` — is this consumer in the same vector space as this index?
//!
//! `embed.config.json` pins every setting a consumer needs to embed a query the way the index
//! was built: model, weights file, pooling, normalization. What it could not do was let a
//! consumer find out whether it had actually done so.
//!
//! That gap has a shape. A consumer loading fp32 weights where the index is quantized gets
//! vectors that are wrong by ~1e-3 per element — far outside retrieval-safe tolerance, and
//! nowhere near far enough to look broken. Cosine scores stay plausible. Results stay
//! ordered. Nothing errors. The one place agreement was ever proven is a parity fixture in
//! CI, and a fixture does not travel with an index.
//!
//! So the witness travels with it now, and this command reads it.
//!
//! # Light, deliberately
//!
//! This does not embed anything. `--provider` names a command that does: it receives the
//! probe on stdin and writes a JSON array of floats on stdout. That is what makes the check
//! available to the consumer that needs it most — a Python process holding an index directory
//! and its own embedder, which is exactly the situation where the drift goes unnoticed. A
//! verifier that required the ML stack would be one only the runtime that is already correct
//! could run.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::embed_config::{verify, EmbedConfig, Verdict, EMBED_CONFIG_FILENAME};

#[derive(serde::Serialize)]
pub struct VerifyReport {
    /// Repository-relative index directory, or the path as given when it is outside.
    pub index: String,
    /// `match` | `known-delta` | `mismatch` | `unverifiable`.
    pub verdict: &'static str,
    /// Whether the consumer may query this index. False only for `mismatch`.
    pub passed: bool,
    pub model_id: String,
    pub embedding_dim: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<String>,
    /// Largest per-element difference from the witness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_drift: Option<f32>,
    /// The bound that was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f32>,
    /// The runtime a `known-delta` verdict was granted to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    pub problems: Vec<String>,
}

fn read_config(index: &Path) -> Result<EmbedConfig> {
    let path = index.join(EMBED_CONFIG_FILENAME);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {} — is this an index directory?", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Run the provider over the probe and read back a vector.
///
/// stdin/stdout rather than a library boundary, because the consumers this exists for are in
/// other languages and other processes. A three-line script is a valid provider.
fn run_provider(command: &str, probe: &str) -> Result<Vec<f32>> {
    let mut parts = command.split_whitespace();
    let program = parts.next().context("--provider is empty")?;
    let mut child = std::process::Command::new(program)
        .args(parts)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning provider {command:?}"))?;
    child
        .stdin
        .take()
        .context("provider has no stdin")?
        .write_all(probe.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        anyhow::bail!("provider {command:?} exited {}", out.status);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Vec<f32>>(text.trim()).with_context(|| {
        format!(
            "provider must write a JSON array of floats on stdout; got {:?}",
            text.chars().take(120).collect::<String>()
        )
    })
}

pub(crate) fn build_report(
    index: &Path,
    config: &EmbedConfig,
    vector: Option<&[f32]>,
    runtime: Option<&str>,
) -> VerifyReport {
    let outcome = match vector {
        Some(v) => verify(config, v, runtime),
        // No provider: report what the index declares and stop. Checking a consumer against
        // itself would answer a question nobody asked.
        None => crate::embed_config::Outcome {
            verdict: match config.verification {
                Some(_) => Verdict::Match,
                None => Verdict::Unverifiable,
            },
            max_drift: None,
            problems: match config.verification {
                Some(_) => vec![],
                None => vec!["this index carries no `verification` block".to_string()],
            },
        },
    };

    let (verdict, runtime_name, tolerance) = match &outcome.verdict {
        Verdict::Match => (
            "match",
            None,
            config.verification.as_ref().map(|v| v.tolerance),
        ),
        Verdict::KnownDelta { runtime, tolerance } => {
            ("known-delta", Some(runtime.clone()), Some(*tolerance))
        }
        Verdict::Mismatch => (
            "mismatch",
            None,
            config.verification.as_ref().map(|v| v.tolerance),
        ),
        Verdict::Unverifiable => ("unverifiable", None, None),
    };

    VerifyReport {
        index: index.to_string_lossy().replace('\\', "/"),
        verdict,
        passed: outcome.passed(),
        model_id: config.model_id.clone(),
        embedding_dim: config.embedding_dim,
        probe: config.verification.as_ref().map(|v| v.probe.clone()),
        max_drift: outcome.max_drift,
        tolerance,
        runtime: runtime_name,
        problems: outcome.problems,
    }
}

pub(crate) fn render_verify(r: &VerifyReport) -> String {
    let mut out = format!("{} — {} ({} dims)\n", r.index, r.model_id, r.embedding_dim);
    out.push_str(&match r.verdict {
        "match" if r.max_drift.is_some() => format!(
            "same vector space (drift {:.2e}, tolerance {:.0e})",
            r.max_drift.unwrap_or(0.0),
            r.tolerance.unwrap_or(0.0)
        ),
        "match" => "witness present; pass --provider to check a consumer against it".to_string(),
        "known-delta" => format!(
            "DIFFERENT vector space, within the bound `{}` declared for it\n  \
             drift {:.2e}, allowed {:.0e}\n  \
             Retrieval works and is measurably not identical. This is the expected \
             degradation, named.",
            r.runtime.clone().unwrap_or_default(),
            r.max_drift.unwrap_or(0.0),
            r.tolerance.unwrap_or(0.0)
        ),
        "unverifiable" => "no witness — this index cannot be checked against".to_string(),
        _ => "MISMATCH".to_string(),
    });
    for p in &r.problems {
        out.push_str(&format!("\n  {p}"));
    }
    out
}

/// Verify a consumer's embedding provider against an index's contract.
pub fn index_verify(
    index: Option<PathBuf>,
    provider: Option<String>,
    runtime: Option<String>,
    format: crate::report::Format,
) -> Result<()> {
    let root = crate::paths::repo_root().ok();
    let index = index.unwrap_or_else(|| {
        root.clone()
            .map(|r| crate::paths::yidam_index_dir(&r))
            .unwrap_or_else(|| PathBuf::from(".yidam/index"))
    });
    let config = read_config(&index)?;

    let vector = match (&provider, &config.verification) {
        (Some(cmd), Some(v)) => Some(run_provider(cmd, &v.probe)?),
        // A provider with nothing to check against: say so rather than run it and pretend.
        (Some(_), None) => None,
        (None, _) => None,
    };

    let report = build_report(&index, &config, vector.as_deref(), runtime.as_deref());
    let passed = report.passed;
    if format.is_json() {
        crate::report::emit(root.as_deref().unwrap_or(&index), report)?;
    } else {
        println!("{}", render_verify(&report));
    }
    if !passed {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed_config::{KnownDelta, Verification};

    fn config(prefix: Vec<f32>) -> EmbedConfig {
        let mut c = EmbedConfig::for_fastembed_model(
            "Xenova/all-MiniLM-L6-v2",
            4,
            "onnx/model_quantized.onnx",
            "AllMiniLML6V2Q",
        );
        c.verification = Some(Verification {
            probe: "knowledge graph traversal".to_string(),
            prefix_dims: prefix.len(),
            prefix,
            tolerance: 1e-5,
            known_delta: [(
                "sentence-transformers".to_string(),
                KnownDelta { tolerance: 1e-2 },
            )]
            .into_iter()
            .collect(),
        });
        c
    }

    /// A normalized 4-vector, so the norm check does not fire on the shape tests.
    fn unit(a: f32, b: f32, c: f32, d: f32) -> Vec<f32> {
        let n = (a * a + b * b + c * c + d * d).sqrt();
        vec![a / n, b / n, c / n, d / n]
    }

    /// Nudge one element and re-normalize — a vector from a *different* space rather than a
    /// longer one from the same.
    fn renormalized(v: &[f32], nudge: f32) -> Vec<f32> {
        let mut out = v.to_vec();
        out[0] += nudge;
        let n = out.iter().map(|x| x * x).sum::<f32>().sqrt();
        out.iter().map(|x| x / n).collect()
    }

    #[test]
    fn the_same_space_matches() {
        let v = unit(0.5, 0.5, 0.5, 0.5);
        let r = build_report(Path::new("i"), &config(v.clone()), Some(&v), None);
        assert_eq!(r.verdict, "match");
        assert!(r.passed);
        assert!(r.max_drift.unwrap() < 1e-6);
    }

    /// The case this whole feature exists for: fp32 where the index is quantized.
    ///
    /// It passes — and it says out loud that it is a different space, which is the entire
    /// difference between an expected degradation and an unnoticed one.
    #[test]
    fn a_declared_runtime_drift_passes_and_is_named() {
        let stored = unit(0.5, 0.5, 0.5, 0.5);
        // Perturbed AND re-normalized: an index of unit vectors compared against a provider
        // whose vector is 1.005 long is a mismatch on the norm before drift is even reached,
        // which is correct and is a different finding from this one.
        let theirs = renormalized(&stored, 5e-3);

        let r = build_report(
            Path::new("i"),
            &config(stored),
            Some(&theirs),
            Some("sentence-transformers"),
        );
        assert_eq!(r.verdict, "known-delta");
        assert!(r.passed);
        assert_eq!(r.runtime.as_deref(), Some("sentence-transformers"));
        assert!(render_verify(&r).contains("DIFFERENT vector space"));
    }

    /// An unnamed runtime gets no bound.
    ///
    /// Inferring one would invent exactly the permission this block exists to make explicit:
    /// a `known_delta` is a runtime declaring its own drift in advance, not a discount
    /// applied to whoever turns up.
    #[test]
    fn an_unnamed_runtime_is_held_to_the_strict_tolerance() {
        let stored = unit(0.5, 0.5, 0.5, 0.5);
        let theirs = renormalized(&stored, 5e-3);
        let r = build_report(Path::new("i"), &config(stored), Some(&theirs), None);
        assert_eq!(r.verdict, "mismatch");
        assert!(!r.passed);
    }

    #[test]
    fn drift_beyond_every_bound_says_to_degrade() {
        let stored = unit(0.5, 0.5, 0.5, 0.5);
        let theirs = unit(1.0, 0.0, 0.0, 0.0);
        let r = build_report(
            Path::new("i"),
            &config(stored),
            Some(&theirs),
            Some("sentence-transformers"),
        );
        assert_eq!(r.verdict, "mismatch");
        assert!(
            r.problems[0].contains("degrade to keyword search"),
            "{:?}",
            r.problems
        );
    }

    /// Wrong width is not drift. It is a different model, and no tolerance covers it.
    #[test]
    fn the_wrong_dimensionality_is_a_mismatch_before_any_comparison() {
        let stored = unit(0.5, 0.5, 0.5, 0.5);
        let r = build_report(Path::new("i"), &config(stored), Some(&[1.0, 0.0]), None);
        assert_eq!(r.verdict, "mismatch");
        assert!(r.max_drift.is_none(), "nothing was compared");
        assert!(r.problems[0].contains("dimensions"));
    }

    /// An index that stores normalized vectors and a provider that does not is a mismatch
    /// even when the direction is right — cosine over un-normalized vectors is not cosine.
    #[test]
    fn an_unnormalized_provider_is_caught() {
        let stored = unit(0.5, 0.5, 0.5, 0.5);
        let theirs: Vec<f32> = stored.iter().map(|x| x * 3.0).collect();
        let r = build_report(Path::new("i"), &config(stored), Some(&theirs), None);
        assert_eq!(r.verdict, "mismatch");
        assert!(r.problems[0].contains("norm"), "{:?}", r.problems);
    }

    /// Every index built before this field existed. Not a failure.
    #[test]
    fn an_index_with_no_witness_is_unverifiable_and_still_passes() {
        let c = EmbedConfig::for_fastembed_model("m", 4, "f", "E");
        let r = build_report(Path::new("i"), &c, Some(&unit(1.0, 0.0, 0.0, 0.0)), None);
        assert_eq!(r.verdict, "unverifiable");
        assert!(
            r.passed,
            "an index that gave nothing to check against is not the consumer's fault"
        );
        assert!(render_verify(&r).contains("cannot be checked"));
    }
}
