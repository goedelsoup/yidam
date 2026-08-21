//! The judge's verdict, as a schema rather than as prose.
//!
//! `judge.md` used to end with "produce a markdown report". Nothing could read one. The
//! rubric's regression rule — *any quality criterion drops by ≥1 band* — needs a band per
//! criterion that survives to the next run and can be compared to it, and a paragraph does
//! not survive as anything.
//!
//! Two properties this type enforces that a report format could not:
//!
//! **Evidence before band.** `evidence` is the first field in the schema and the prompt asks
//! for it first, because a model that states a verdict and then explains it is explaining a
//! verdict it has already committed to. Quoting the node text before scoring it makes the
//! band answerable to something.
//!
//! **Every criterion, exactly once.** A judge that returns six of seven has not scored the
//! seventh, and a report missing a criterion is not a report with an implicit pass. Parsing
//! rejects it.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Ordered worst to best, so `<` on the derived ordering means "dropped a band".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Band {
    Fail,
    Marginal,
    Pass,
}

impl Band {
    pub fn as_str(&self) -> &'static str {
        match self {
            Band::Fail => "fail",
            Band::Marginal => "marginal",
            Band::Pass => "pass",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CriterionVerdict {
    pub id: String,
    /// What in the corpus, the commit, or the transcript bears on this criterion. Quoted, so
    /// a reader can disagree with the band without re-running anything.
    ///
    /// Required, and required even when the criterion fails for absence — "no clarifying
    /// turns appear before the first Write" is evidence. A band with nothing behind it is an
    /// assertion, and the whole reason for this field is that assertions do not survive
    /// review.
    pub evidence: Vec<String>,
    pub band: Band,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityReport {
    pub criteria: Vec<CriterionVerdict>,
    pub overall: Band,
    pub most_important_finding: String,
}

impl QualityReport {
    pub fn band_of(&self, id: &str) -> Option<Band> {
        self.criteria.iter().find(|c| c.id == id).map(|c| c.band)
    }

    pub fn print(&self) {
        for c in &self.criteria {
            println!("[{}] {} — {}", c.band.as_str(), c.id, c.rationale);
        }
        println!("overall: {}", self.overall.as_str());
        println!("finding: {}", self.most_important_finding);
    }
}

/// Parse a judge's reply and check it against the criteria it was asked to score.
///
/// `expected` comes from `rubric.md` rather than a list here, so a criterion added to the
/// document is a criterion the judge is held to without anyone editing this file.
pub fn parse(reply: &str, expected: &[&str]) -> Result<QualityReport> {
    let json = extract_json(reply)
        .ok_or_else(|| anyhow::anyhow!("no JSON object in the judge's reply:\n{reply}"))?;
    let report: QualityReport = serde_json::from_str(&json)
        .map_err(|e| anyhow::anyhow!("the judge's JSON did not match the schema: {e}\n{json}"))?;

    let scored: Vec<&str> = report.criteria.iter().map(|c| c.id.as_str()).collect();

    let missing: Vec<&&str> = expected.iter().filter(|id| !scored.contains(id)).collect();
    if !missing.is_empty() {
        bail!(
            "the judge did not score {missing:?}. A criterion with no verdict is not a \
             criterion that passed."
        );
    }
    let unknown: Vec<&&str> = scored.iter().filter(|id| !expected.contains(id)).collect();
    if !unknown.is_empty() {
        bail!("the judge scored {unknown:?}, which rubric.md does not state");
    }
    for id in expected {
        if report.criteria.iter().filter(|c| &c.id == id).count() > 1 {
            bail!("the judge scored {id} more than once");
        }
    }
    let unevidenced: Vec<&str> = report
        .criteria
        .iter()
        .filter(|c| c.evidence.iter().all(|e| e.trim().is_empty()))
        .map(|c| c.id.as_str())
        .collect();
    if !unevidenced.is_empty() {
        bail!(
            "the judge gave a band with no evidence for {unevidenced:?}. Absence is evidence \
             and can be stated; an empty list is a verdict answerable to nothing."
        );
    }
    Ok(report)
}

/// Pull the first balanced `{…}` out of a reply, ignoring braces inside strings.
///
/// A fenced block would be easier to find and easier to get wrong: models emit
/// ```` ```json ````, plain ```` ``` ````, or no fence at all, and prose after the object is
/// common. Scanning for balance handles all of them.
fn extract_json(reply: &str) -> Option<String> {
    let bytes = reply.as_bytes();
    let start = reply.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for i in start..bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(reply[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: [&str; 2] = ["Q1", "Q2"];

    fn verdict(id: &str, band: &str) -> String {
        format!(r#"{{"id":"{id}","evidence":["a quote"],"band":"{band}","rationale":"because"}}"#)
    }

    fn report(bodies: &[String]) -> String {
        format!(
            r#"{{"criteria":[{}],"overall":"marginal","most_important_finding":"a finding"}}"#,
            bodies.join(",")
        )
    }

    #[test]
    fn a_complete_report_parses() {
        let r = parse(
            &report(&[verdict("Q1", "pass"), verdict("Q2", "fail")]),
            &IDS,
        )
        .unwrap();
        assert_eq!(r.band_of("Q1"), Some(Band::Pass));
        assert_eq!(r.band_of("Q2"), Some(Band::Fail));
        assert_eq!(r.overall, Band::Marginal);
    }

    #[test]
    fn prose_around_the_object_does_not_defeat_it() {
        let wrapped = format!(
            "Here is my assessment:\n\n```json\n{}\n```\n\nLet me know if you want more.",
            report(&[verdict("Q1", "pass"), verdict("Q2", "pass")])
        );
        assert!(parse(&wrapped, &IDS).is_ok());
    }

    #[test]
    fn a_brace_inside_a_string_does_not_end_the_object() {
        let body =
            r#"{"id":"Q1","evidence":["the node said {this}"],"band":"pass","rationale":"ok"}"#;
        let r = parse(&report(&[body.to_string(), verdict("Q2", "pass")]), &IDS).unwrap();
        assert_eq!(r.criteria[0].evidence[0], "the node said {this}");
    }

    /// The failure the schema exists to make impossible: a silent gap read as a pass.
    #[test]
    fn a_criterion_the_judge_skipped_is_an_error() {
        let err = parse(&report(&[verdict("Q1", "pass")]), &IDS).unwrap_err();
        assert!(err.to_string().contains("Q2"), "{err}");
    }

    #[test]
    fn a_criterion_the_rubric_does_not_state_is_an_error() {
        let bodies = [
            verdict("Q1", "pass"),
            verdict("Q2", "pass"),
            verdict("Q9", "pass"),
        ];
        let err = parse(&report(&bodies), &IDS).unwrap_err();
        assert!(err.to_string().contains("Q9"), "{err}");
    }

    #[test]
    fn scoring_a_criterion_twice_is_an_error() {
        let bodies = [
            verdict("Q1", "pass"),
            verdict("Q1", "fail"),
            verdict("Q2", "pass"),
        ];
        let err = parse(&report(&bodies), &IDS).unwrap_err();
        assert!(err.to_string().contains("more than once"), "{err}");
    }

    #[test]
    fn a_band_with_no_evidence_is_an_error() {
        let bare = r#"{"id":"Q1","evidence":[],"band":"pass","rationale":"looks fine"}"#;
        let err = parse(&report(&[bare.to_string(), verdict("Q2", "pass")]), &IDS).unwrap_err();
        assert!(err.to_string().contains("no evidence"), "{err}");
    }

    #[test]
    fn a_reply_with_no_object_is_an_error_rather_than_an_empty_report() {
        assert!(parse("I was unable to assess this corpus.", &IDS).is_err());
    }

    /// The ordering is what the regression rule means by "drops by ≥1 band".
    #[test]
    fn the_bands_are_ordered_worst_to_best() {
        assert!(Band::Fail < Band::Marginal && Band::Marginal < Band::Pass);
    }
}
