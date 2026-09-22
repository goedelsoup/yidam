use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::check::{drift, CheckReport};
use crate::quality::QualityReport;
use crate::transcript::RunRecord;
use crate::PROTOCOL_VERSION;

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    /// The protocol version this snapshot's verdicts are valid for — the one `diff` compares
    /// on. `None` for a snapshot written before the version was recorded at all, which is
    /// every snapshot from 0.1.0, and the reason this is an option rather than a defaulted
    /// string: "written by a harness that did not know its own version" is a fact worth
    /// keeping, not one worth guessing at.
    ///
    /// Written by a run, it is the version that run was taken under. It can also be moved
    /// forward by [`revalidate`] without re-running the model, and then `revalidated` says
    /// where it came from and why — so "valid for" and "taken under" stay separable.
    #[serde(default)]
    pub protocol_version: Option<String>,
    /// Set when this snapshot's `protocol_version` was carried forward rather than produced.
    /// Absent on every snapshot a run wrote, which is what makes its presence readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revalidated: Option<Revalidation>,
    /// What produced this result. `None` for a snapshot taken by `harness check`, which
    /// re-reads a captured directory and never invoked a model.
    #[serde(default)]
    pub run: Option<RunRecord>,
    /// The judge's verdict. `None` when the run was not scored — scoring costs a second
    /// model call and is opt-in, so absent means "not asked", never "nothing to report".
    #[serde(default)]
    pub quality: Option<QualityReport>,
    pub structural: CheckReport,
}

/// Why a snapshot's verdicts were carried across a protocol bump instead of being re-run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Revalidation {
    /// The version the run was actually taken under. Kept because `protocol_version` no
    /// longer answers that question once this field exists.
    pub from: String,
    pub to: String,
    /// The argument that the recorded verdicts still mean under `to` what they meant under
    /// `from`. Required, and required here rather than in a commit message, because the
    /// snapshot is what `diff` reads and a reader who disagrees with the carry-forward has
    /// to be able to find the claim beside the thing it licensed.
    pub reason: String,
}

impl Snapshot {
    /// How to name this snapshot's protocol version in a message to a person.
    pub fn version_label(&self) -> &str {
        self.protocol_version.as_deref().unwrap_or("unversioned")
    }

    /// The criterion IDs this snapshot carries bands for, in recorded order. Empty when the
    /// run was not scored — which is "not asked", not "scored nothing".
    pub fn scored_ids(&self) -> Vec<&str> {
        self.quality
            .iter()
            .flat_map(|q| q.criteria.iter().map(|c| c.id.as_str()))
            .collect()
    }
}

/// Carry a snapshot's verdicts forward to a newer protocol version without re-running it.
///
/// **What this is for.** `diff` refuses to compare across protocol versions, so a committed
/// baseline stops being comparable the moment the protocol bumps: every fresh run stamps the
/// new version, every `diff` against the baseline takes the refusal branch, and the
/// regression gate goes inert while nothing goes red. The expensive half of an eval is
/// producing the run, and re-running one to move a version string is paying for it twice.
///
/// **What it checks, and what it cannot.** Two necessary conditions, both measured:
///
/// 1. The checks recomputed under the new protocol return exactly the recorded verdicts. The
///    corpus in a baseline never changes, so this says the S-half of the snapshot is already
///    what the new protocol produces.
/// 2. The bands recorded are the criteria the rubric states now. Adding a Q criterion — which
///    0.2.0 → 0.3.0 did — leaves a snapshot carrying one band fewer than the instrument asks
///    for, and comparing those would report an absence as a result.
///
/// Neither is sufficient, and the gap is the reason for `reason`. Identical verdicts on *this*
/// corpus do not prove the checks are the same function: a check that got *stricter* can agree
/// on a baseline that passes both and still turn a candidate's honest pass into a fail, which
/// is a moved check reported as a regression — precisely what `diff`'s refusal exists to
/// prevent. A check that got *wider* cannot: everything it used to accept it still accepts,
/// so a pass→fail across the boundary is the model. Deciding which happened is reading a
/// diff, and no code here can do it. So the caller states the argument and it ships in the
/// snapshot.
pub fn revalidate(
    snapshot: &Snapshot,
    fresh: &CheckReport,
    rubric_quality_ids: &[&str],
    to: &str,
    reason: &str,
) -> Result<Snapshot> {
    let from = snapshot.version_label().to_string();
    if from == to {
        anyhow::bail!("already recorded at protocol {to} — there is nothing to carry forward");
    }
    if reason.trim().is_empty() {
        anyhow::bail!(
            "a revalidation needs a reason. It is the argument that the recorded verdicts \
             still mean under {to} what they meant under {from}, and without it this is a \
             baseline following the code rather than holding it to anything."
        );
    }

    let drifted = drift(&snapshot.structural, fresh);
    if !drifted.is_empty() {
        anyhow::bail!(
            "the checks no longer return the recorded verdicts for this corpus:\n  {}\n\
             The corpus has not changed, so {to}'s checks say something different about it \
             than {from}'s did. That is a re-record, not a revalidation — run \
             `harness check` against this directory and decide what the new verdicts mean.",
            drifted.join("\n  ")
        );
    }

    // As a set, not a sequence: `quality::parse` holds the judge to scoring every criterion
    // exactly once and says nothing about the order they come back in, so a snapshot whose
    // bands are the right ones in a different order is comparable and must not be refused.
    let scored = snapshot.scored_ids();
    let (mut scored_set, mut rubric_set) = (scored.clone(), rubric_quality_ids.to_vec());
    scored_set.sort_unstable();
    rubric_set.sort_unstable();
    if !scored.is_empty() && scored_set != rubric_set {
        anyhow::bail!(
            "the recorded bands are not the criteria the rubric states now: recorded [{}], \
             rubric [{}].\nThe instrument changed, so the bands are not this protocol's \
             bands. Re-score with `harness judge` — it reads the captured result and needs \
             no bootstrap re-run.",
            scored.join(", "),
            rubric_quality_ids.join(", ")
        );
    }

    Ok(Snapshot {
        protocol_version: Some(to.to_string()),
        revalidated: Some(Revalidation {
            from,
            to: to.to_string(),
            reason: reason.trim().to_string(),
        }),
        run: snapshot.run.clone(),
        quality: snapshot.quality.clone(),
        structural: snapshot.structural.clone(),
    })
}

pub fn write(
    result_dir: &Path,
    structural: &CheckReport,
    run: Option<RunRecord>,
    quality: Option<QualityReport>,
) -> Result<()> {
    save(
        result_dir,
        &Snapshot {
            protocol_version: Some(PROTOCOL_VERSION.to_string()),
            revalidated: None,
            run,
            quality,
            structural: structural.clone(),
        },
    )
}

/// Write a snapshot verbatim. `write` is the path a run takes; this is the path a snapshot
/// that already exists takes when only its provenance changed.
pub fn save(result_dir: &Path, snap: &Snapshot) -> Result<()> {
    let json = serde_json::to_string_pretty(snap)?;
    std::fs::write(result_dir.join("structural.json"), json).context("writing structural.json")
}

pub fn load(result_dir: &Path) -> Result<Snapshot> {
    let content = std::fs::read_to_string(result_dir.join("structural.json"))
        .context("reading structural.json")?;
    serde_json::from_str(&content).context("parsing structural.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::CheckResult;
    use crate::quality::{Band, CriterionVerdict, QualityReport};

    fn result(id: &str, passed: bool) -> CheckResult {
        CheckResult {
            id: id.into(),
            description: "a check".into(),
            passed,
            detail: None,
        }
    }

    fn snap(version: &str, results: Vec<CheckResult>, scored: &[&str]) -> Snapshot {
        Snapshot {
            protocol_version: Some(version.into()),
            revalidated: None,
            run: None,
            quality: (!scored.is_empty()).then(|| QualityReport {
                criteria: scored
                    .iter()
                    .map(|id| CriterionVerdict {
                        id: (*id).into(),
                        evidence: vec!["a quote".into()],
                        band: Band::Pass,
                        rationale: "because".into(),
                    })
                    .collect(),
                overall: Band::Pass,
                most_important_finding: "a finding".into(),
            }),
            structural: CheckReport { results },
        }
    }

    #[test]
    fn a_baseline_the_current_checks_agree_with_carries_forward() {
        let recorded = snap("0.3.0", vec![result("S1", true)], &["Q1"]);
        let fresh = CheckReport {
            results: vec![result("S1", true)],
        };
        let carried = revalidate(&recorded, &fresh, &["Q1"], "0.4.0", "S4 only widened").unwrap();
        assert_eq!(carried.protocol_version.as_deref(), Some("0.4.0"));
        let r = carried.revalidated.unwrap();
        assert_eq!(r.from, "0.3.0");
        assert_eq!(r.to, "0.4.0");
        assert_eq!(r.reason, "S4 only widened");
    }

    /// The verdicts are the evidence. Without them agreeing, this is a re-record.
    #[test]
    fn a_baseline_whose_verdicts_moved_is_refused() {
        let recorded = snap("0.3.0", vec![result("S1", true)], &[]);
        let fresh = CheckReport {
            results: vec![result("S1", false)],
        };
        let err = revalidate(&recorded, &fresh, &[], "0.4.0", "a reason").unwrap_err();
        assert!(err
            .to_string()
            .contains("S1 was passing and is now failing"));
    }

    /// 0.2.0 → 0.3.0 added Q8. A snapshot carrying seven bands is not comparable against an
    /// instrument that asks for eight, however well the S-checks agree.
    #[test]
    fn a_baseline_scored_against_a_different_rubric_is_refused() {
        let recorded = snap("0.2.0", vec![result("S1", true)], &["Q1", "Q2"]);
        let fresh = CheckReport {
            results: vec![result("S1", true)],
        };
        let err =
            revalidate(&recorded, &fresh, &["Q1", "Q2", "Q3"], "0.3.0", "a reason").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Q3"), "{msg}");
        assert!(msg.contains("harness judge"), "{msg}");
    }

    /// The judge is held to scoring every criterion once, not to an order. A snapshot whose
    /// bands came back as Q2, Q1 is the rubric's band set and must carry forward.
    #[test]
    fn bands_recorded_in_another_order_are_still_the_rubrics_bands() {
        let recorded = snap("0.3.0", vec![result("S1", true)], &["Q2", "Q1"]);
        let fresh = CheckReport {
            results: vec![result("S1", true)],
        };
        assert!(revalidate(&recorded, &fresh, &["Q1", "Q2"], "0.4.0", "a reason").is_ok());
    }

    /// An unscored baseline has no bands to hold to the rubric, and that is not a failure.
    #[test]
    fn an_unscored_baseline_is_not_held_to_the_rubric() {
        let recorded = snap("0.3.0", vec![result("S1", true)], &[]);
        let fresh = CheckReport {
            results: vec![result("S1", true)],
        };
        assert!(revalidate(&recorded, &fresh, &["Q1", "Q2"], "0.4.0", "a reason").is_ok());
    }

    #[test]
    fn a_revalidation_without_a_reason_is_refused() {
        let recorded = snap("0.3.0", vec![result("S1", true)], &[]);
        let fresh = CheckReport {
            results: vec![result("S1", true)],
        };
        let err = revalidate(&recorded, &fresh, &[], "0.4.0", "   ").unwrap_err();
        assert!(err.to_string().contains("needs a reason"));
    }

    #[test]
    fn a_baseline_already_at_the_target_version_is_refused() {
        let recorded = snap("0.4.0", vec![result("S1", true)], &[]);
        let fresh = CheckReport {
            results: vec![result("S1", true)],
        };
        let err = revalidate(&recorded, &fresh, &[], "0.4.0", "a reason").unwrap_err();
        assert!(err.to_string().contains("nothing to carry forward"));
    }

    /// The field is new, so every snapshot written before it exists parses without it — and
    /// a snapshot that was never revalidated does not grow a null key saying so.
    #[test]
    fn a_snapshot_without_the_field_round_trips_as_not_revalidated() {
        let json = r#"{"protocol_version":"0.3.0","structural":{"results":[]}}"#;
        let parsed: Snapshot = serde_json::from_str(json).unwrap();
        assert!(parsed.revalidated.is_none());
        let written = serde_json::to_string(&parsed).unwrap();
        assert!(!written.contains("revalidated"), "{written}");
    }
}
