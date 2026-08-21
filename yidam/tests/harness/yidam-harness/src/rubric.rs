//! The rubric, read rather than restated.
//!
//! `rubric.md` states S1–S7 and Q1–Q7 in two markdown tables. Before this module the same
//! criteria were also written out in `docs/quality-rubric.md`, in `check.rs`'s description
//! strings, and in prose in `judge.md` — four transcriptions, each pinned only by itself.
//! The parity task's own comment names the failure class: "two transcriptions each pinned
//! only by itself."
//!
//! The Q criteria are read from here because the judge is scored against them and a judge
//! prompt built from a copy would drift from the document reviewers read.
//!
//! The S descriptions are not read at runtime. `check::run_all` takes a result directory and
//! nothing else, and giving it a dependency on finding the repository — so that
//! `harness check` could fail because it was invoked from the wrong place — buys less than it
//! costs. They are pinned instead, which is what the parity fixtures do for the three SDKs
//! and what `the_documented_format_version_is_the_declared_one` does for Layer 4: two
//! transcriptions are a problem when each is pinned only by itself, not when a test pins them
//! to each other.
//!
//! `docs/quality-rubric.md` is the third copy, pinned here for the same reason. It is the
//! page the docs site renders, so it cannot simply be deleted in favour of a link.

use anyhow::{Context, Result};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct Rubric {
    pub structural: Vec<Criterion>,
    pub quality: Vec<Criterion>,
}

impl Rubric {
    pub fn quality_ids(&self) -> Vec<&str> {
        self.quality.iter().map(|c| c.id.as_str()).collect()
    }
}

/// Read `rubric.md` and return the two criterion sets.
pub fn load(path: &Path) -> Result<Rubric> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading rubric: {}", path.display()))?;
    let rubric = parse(&text);
    if rubric.structural.is_empty() || rubric.quality.is_empty() {
        anyhow::bail!(
            "{} parsed to {} structural and {} quality criteria — the tables moved, or their \
             ID column stopped looking like `S<n>` / `Q<n>`",
            path.display(),
            rubric.structural.len(),
            rubric.quality.len()
        );
    }
    Ok(rubric)
}

/// Rows are `| \`S1\` | description |`. The ID's letter decides which set it joins, so the
/// two tables do not have to be located by their headings — a criterion cannot be silently
/// filed under the wrong one by moving a section.
fn parse(text: &str) -> Rubric {
    let mut rubric = Rubric::default();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().trim_matches('`').trim())
            .collect();
        if cells.len() < 2 {
            continue;
        }
        let (id, description) = (cells[0], cells[1]);
        if description.is_empty() || !is_criterion_id(id) {
            continue;
        }
        let criterion = Criterion {
            id: id.to_string(),
            description: description.to_string(),
        };
        match id.as_bytes()[0] {
            b'S' => rubric.structural.push(criterion),
            b'Q' => rubric.quality.push(criterion),
            _ => {}
        }
    }
    rubric
}

/// `S1`, `Q7` — one letter then digits, and nothing else. Keeps the header row and the
/// `|---|` separator out, and keeps a stray table elsewhere in the document from joining in.
fn is_criterion_id(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some('S' | 'Q')) && {
        let rest: String = chars.collect();
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rubric_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rubric.md")
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../..")
            .canonicalize()
            .unwrap()
    }

    /// Structure, not a count. A hardcoded seven is one more transcription of the rubric,
    /// and it would have to be edited by the same hand that adds a criterion — which makes it
    /// a step to remember rather than a thing that holds.
    #[test]
    fn the_real_rubric_parses() {
        let r = load(&rubric_path()).unwrap();
        for (letter, set) in [('S', &r.structural), ('Q', &r.quality)] {
            assert!(!set.is_empty(), "no {letter} criteria parsed");
            let expected: Vec<String> = (1..=set.len()).map(|n| format!("{letter}{n}")).collect();
            let got: Vec<&str> = set.iter().map(|c| c.id.as_str()).collect();
            assert_eq!(
                got, expected,
                "{letter} criteria are not numbered without a gap"
            );
            assert!(
                set.iter().all(|c| c.description.len() > 10),
                "a criterion parsed with no description: {set:?}"
            );
        }
    }

    /// The S-checks `check.rs` implements and the S-checks the rubric states are the same
    /// set. This is the half of the bidirectional rule that can be asserted from here; the
    /// other half — every scorer maps to an ID — belongs with the scorers.
    #[test]
    fn every_structural_criterion_has_a_check() {
        let rubric = load(&rubric_path()).unwrap();
        let report = crate::check::run_all(Path::new("/nonexistent")).unwrap();
        let implemented: Vec<&str> = report.results.iter().map(|r| r.id.as_str()).collect();
        for criterion in &rubric.structural {
            assert!(
                implemented.contains(&criterion.id.as_str()),
                "rubric.md states {} and check.rs does not run it",
                criterion.id
            );
        }
        assert_eq!(
            implemented.len(),
            rubric.structural.len(),
            "check.rs runs checks the rubric does not state: {implemented:?}"
        );
    }

    /// Backticks are markdown formatting and carry no meaning here. Nothing else is
    /// normalised — a comparison that has to soften case or punctuation to succeed is one
    /// that will also soften a real divergence.
    fn plain(s: &str) -> String {
        s.replace('`', "")
    }

    /// The rubric row is the sentence the harness prints. Before this, `check.rs` said
    /// "each corpus node has ≥1 outgoing markdown link" while the rubric said something
    /// else entirely, and both were right about a corpus layout that no longer existed.
    #[test]
    fn every_check_reports_the_description_the_rubric_states() {
        let rubric = load(&rubric_path()).unwrap();
        let report = crate::check::run_all(Path::new("/nonexistent")).unwrap();

        for criterion in &rubric.structural {
            let result = report
                .results
                .iter()
                .find(|r| r.id == criterion.id)
                .unwrap_or_else(|| panic!("{} has no check", criterion.id));
            assert_eq!(
                plain(&result.description),
                plain(&criterion.description),
                "{} is described one way in rubric.md and another in check.rs",
                criterion.id
            );
        }
    }

    /// The docs-site copy states the same criteria as the rubric the harness implements.
    ///
    /// It said "exactly 1 git commit exists" for as long as `check.rs` did, which is how a
    /// reader of the documentation would have learned a requirement no correct bootstrap has
    /// ever satisfied.
    #[test]
    fn the_docs_copy_of_the_rubric_states_the_same_criteria() {
        let rubric = load(&rubric_path()).unwrap();
        let docs = load(&repo_root().join("docs/quality-rubric.md")).unwrap();

        for (label, source, copy) in [
            ("structural", &rubric.structural, &docs.structural),
            ("quality", &rubric.quality, &docs.quality),
        ] {
            let source: Vec<_> = source
                .iter()
                .map(|c| (&c.id, plain(&c.description)))
                .collect();
            let copy: Vec<_> = copy
                .iter()
                .map(|c| (&c.id, plain(&c.description)))
                .collect();
            assert_eq!(
                source, copy,
                "docs/quality-rubric.md and yidam/tests/rubric.md disagree about the {label} \
                 criteria. The rubric beside the harness is the source; regenerate the docs \
                 page from it."
            );
        }
    }

    #[test]
    fn a_separator_row_is_not_a_criterion() {
        let r = parse("| ID | Check |\n|---|---|\n| `S1` | a thing |\n| `Q1` | another |\n");
        assert_eq!(r.structural.len(), 1);
        assert_eq!(r.quality.len(), 1);
        assert_eq!(r.structural[0].description, "a thing");
    }

    #[test]
    fn a_table_that_is_not_the_rubric_is_ignored() {
        let r = parse("| Skill | Purpose |\n|---|---|\n| `bootstrap` | init |\n");
        assert!(r.structural.is_empty() && r.quality.is_empty());
    }

    #[test]
    fn an_empty_rubric_is_an_error_rather_than_an_empty_run() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("rubric.md");
        std::fs::write(&path, "# Rubric\n\nNo tables here.\n").unwrap();
        assert!(
            load(&path).is_err(),
            "zero criteria must not read as nothing to score"
        );
    }
}
