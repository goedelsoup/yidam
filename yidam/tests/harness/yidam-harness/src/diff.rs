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
             The checks themselves changed between them, so this comparison is refused rather \
             than reported — and with it the whole regression gate, which is the state a stale \
             baseline leaves this in silently.\n\
             Re-run the baseline scenario under the current protocol and compare against that; \
             or, if the recorded verdicts are already what the current protocol produces, carry \
             the baseline forward with `harness rebaseline --result <dir> --reason <why>`, which \
             checks that and needs no model.",
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

/// What a comparison actually looked at, for printing beside its verdict.
///
/// "no regressions" is the same three syllables whether seven checks and eight bands agreed
/// or whether the quality half was never in play. `harness judge` writing `quality.json`
/// without updating `structural.json` once left a scored baseline that `diff` read as
/// unscored, and the gate reported "no regressions" for "nothing to compare" — the same
/// sentence, meaning the opposite thing. So the verdict now names its own scope.
pub fn scope(baseline: &Snapshot, candidate: &Snapshot) -> String {
    let checks = baseline
        .structural
        .results
        .iter()
        .filter(|b| candidate.structural.results.iter().any(|c| c.id == b.id))
        .count();
    let bands = match (&baseline.quality, &candidate.quality) {
        (Some(b), Some(_)) => Some(b.criteria.len()),
        _ => None,
    };
    let at = format!("at protocol {}", baseline.version_label());
    match bands {
        Some(n) => format!("compared {checks} structural check(s) and {n} quality band(s) {at}"),
        None => format!(
            "compared {checks} structural check(s) {at}; {}, so no band was compared",
            if baseline.quality.is_none() {
                "the baseline carries no judge score"
            } else {
                "the candidate was not scored"
            }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{CheckReport, CheckResult};
    use crate::quality::{Band, CriterionVerdict, QualityReport};

    fn snap(version: Option<&str>, id: &str, passed: bool) -> Snapshot {
        Snapshot {
            protocol_version: version.map(str::to_string),
            revalidated: None,
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
    fn an_unversioned_baseline_is_refused_rather_than_reported() {
        let err = compare(&snap(None, "S1", true), &snap(Some("0.2.0"), "S1", false)).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unversioned") && msg.contains("0.2.0"),
            "{msg}"
        );
    }

    /// The case that actually fires. The only committed baseline recorded 0.3.0 while
    /// `PROTOCOL_VERSION` was 0.4.0, so every `diff` against it took this branch — and the
    /// only refusal test covered the absent-version case, which no run can produce.
    #[test]
    fn two_different_versions_are_refused_rather_than_reported() {
        let err = compare(
            &snap(Some("0.3.0"), "S1", true),
            &snap(Some("0.4.0"), "S1", false),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("0.3.0") && msg.contains("0.4.0"),
            "the refusal must name both sides: {msg}"
        );
        assert!(
            msg.contains("rebaseline"),
            "the refusal must name the way out of it: {msg}"
        );
    }

    /// A refusal, not a silent pass. The regression this guards is the gate reporting
    /// "no regressions" across a boundary it cannot attribute.
    #[test]
    fn a_refused_comparison_reports_no_regressions_at_all() {
        assert!(compare(
            &snap(Some("0.3.0"), "S1", true),
            &snap(Some("0.4.0"), "S1", false),
        )
        .is_err());
    }

    #[test]
    fn a_verdict_names_the_bands_it_weighed() {
        let base = with_quality(snap(Some("0.4.0"), "S1", true), "Q3", Band::Pass);
        let cand = with_quality(snap(Some("0.4.0"), "S1", true), "Q3", Band::Pass);
        let line = scope(&base, &cand);
        assert!(line.contains("1 structural check"), "{line}");
        assert!(line.contains("1 quality band"), "{line}");
        assert!(line.contains("0.4.0"), "{line}");
    }

    /// "no regressions" over an unscored pair must not read like "no bands dropped".
    #[test]
    fn a_verdict_over_unscored_runs_says_no_band_was_compared() {
        let line = scope(
            &snap(Some("0.4.0"), "S1", true),
            &snap(Some("0.4.0"), "S1", true),
        );
        assert!(line.contains("no band was compared"), "{line}");
    }
}
