use anyhow::{bail, Result};

use crate::snapshot::Snapshot;

/// Compare two snapshots and report regressions.
///
/// Refuses to compare across protocol versions rather than reporting a difference it cannot
/// attribute. When S1 changes what it asks of a corpus, a pass→fail transition across that
/// boundary says the check moved, not that the model got worse — and a regression report
/// that cannot tell those apart is worse than no report, because someone will act on it.
pub fn compare(baseline: &Snapshot, candidate: &Snapshot) -> Result<Vec<String>> {
    if baseline.protocol_version != candidate.protocol_version {
        bail!(
            "cannot compare across bootstrap protocol versions: baseline is {}, candidate is {}.\n\
             The checks themselves changed between them. Re-run the baseline scenario under \
             the current protocol and compare against that.",
            baseline.version_label(),
            candidate.version_label()
        );
    }

    let mut regressions = Vec::new();

    for base_result in &baseline.structural.results {
        if let Some(cand_result) = candidate
            .structural
            .results
            .iter()
            .find(|r| r.id == base_result.id)
        {
            if base_result.passed && !cand_result.passed {
                regressions.push(format!(
                    "{} ({}) was passing, now failing{}",
                    base_result.id,
                    base_result.description,
                    cand_result
                        .detail
                        .as_deref()
                        .map(|d| format!(": {d}"))
                        .unwrap_or_default()
                ));
            }
        } else {
            regressions.push(format!("{} missing from candidate results", base_result.id));
        }
    }

    // "Any quality criterion drops by ≥1 band" — rubric.md. Only comparable when both runs
    // were scored: a candidate that was not judged has not regressed, it has not been asked.
    if let (Some(base_q), Some(cand_q)) = (&baseline.quality, &candidate.quality) {
        for base_c in &base_q.criteria {
            match cand_q.band_of(&base_c.id) {
                Some(now) if now < base_c.band => regressions.push(format!(
                    "{} dropped {} → {}: {}",
                    base_c.id,
                    base_c.band.as_str(),
                    now.as_str(),
                    cand_q
                        .criteria
                        .iter()
                        .find(|c| c.id == base_c.id)
                        .map(|c| c.rationale.as_str())
                        .unwrap_or_default()
                )),
                Some(_) => {}
                None => regressions.push(format!(
                    "{} was scored in the baseline and is missing from the candidate",
                    base_c.id
                )),
            }
        }
    }

    Ok(regressions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{CheckReport, CheckResult};
    use crate::quality::{Band, CriterionVerdict, QualityReport};

    fn snap(version: Option<&str>, id: &str, passed: bool) -> Snapshot {
        Snapshot {
            protocol_version: version.map(str::to_string),
            run: None,
            quality: None,
            structural: CheckReport {
                results: vec![CheckResult {
                    id: id.into(),
                    description: "a check".into(),
                    passed,
                    detail: None,
                }],
            },
        }
    }

    #[test]
    fn a_check_that_stopped_passing_is_a_regression() {
        let r = compare(
            &snap(Some("0.2.0"), "S1", true),
            &snap(Some("0.2.0"), "S1", false),
        )
        .unwrap();
        assert_eq!(r.len(), 1, "{r:?}");
    }

    fn with_quality(mut s: Snapshot, id: &str, band: Band) -> Snapshot {
        s.quality = Some(QualityReport {
            criteria: vec![CriterionVerdict {
                id: id.into(),
                evidence: vec!["a quote".into()],
                band,
                rationale: "because".into(),
            }],
            overall: band,
            most_important_finding: "a finding".into(),
        });
        s
    }

    #[test]
    fn a_band_that_dropped_is_a_regression() {
        let base = with_quality(snap(Some("0.2.0"), "S1", true), "Q3", Band::Pass);
        let cand = with_quality(snap(Some("0.2.0"), "S1", true), "Q3", Band::Marginal);
        let r = compare(&base, &cand).unwrap();
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(r[0].contains("Q3 dropped pass → marginal"), "{r:?}");
    }

    #[test]
    fn a_band_that_improved_is_not_a_regression() {
        let base = with_quality(snap(Some("0.2.0"), "S1", true), "Q3", Band::Fail);
        let cand = with_quality(snap(Some("0.2.0"), "S1", true), "Q3", Band::Pass);
        assert!(compare(&base, &cand).unwrap().is_empty());
    }

    /// A candidate nobody asked to score has not regressed.
    #[test]
    fn an_unscored_candidate_is_not_a_quality_regression() {
        let base = with_quality(snap(Some("0.2.0"), "S1", true), "Q3", Band::Pass);
        let cand = snap(Some("0.2.0"), "S1", true);
        assert!(compare(&base, &cand).unwrap().is_empty());
    }

    /// But a scored candidate that dropped a criterion has.
    #[test]
    fn a_criterion_missing_from_a_scored_candidate_is_a_regression() {
        let base = with_quality(snap(Some("0.2.0"), "S1", true), "Q3", Band::Pass);
        let cand = with_quality(snap(Some("0.2.0"), "S1", true), "Q4", Band::Pass);
        let r = compare(&base, &cand).unwrap();
        assert!(r.iter().any(|m| m.contains("Q3")), "{r:?}");
    }

    #[test]
    fn a_cross_version_comparison_is_refused_rather_than_reported() {
        let err = compare(&snap(None, "S1", true), &snap(Some("0.2.0"), "S1", false)).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unversioned") && msg.contains("0.2.0"),
            "{msg}"
        );
    }
}
