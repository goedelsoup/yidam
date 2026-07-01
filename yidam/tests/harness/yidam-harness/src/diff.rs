use anyhow::Result;

use crate::snapshot::Snapshot;

pub fn compare(baseline: &Snapshot, candidate: &Snapshot) -> Result<Vec<String>> {
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
