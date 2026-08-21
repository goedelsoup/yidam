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

    Ok(regressions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{CheckReport, CheckResult};

    fn snap(version: Option<&str>, id: &str, passed: bool) -> Snapshot {
        Snapshot {
            protocol_version: version.map(str::to_string),
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
