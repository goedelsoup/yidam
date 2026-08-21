//! The rubric, read rather than restated.
//!
//! `rubric.md` states S1–S7 and Q1–Q7 in two markdown tables. Before this module the same
//! criteria were also written out in `docs/quality-rubric.md`, in `check.rs`'s description
//! strings, and in prose in `judge.md` — four transcriptions, each pinned only by itself.
//! The parity task's own comment names the failure class: "two transcriptions each pinned
//! only by itself."
//!
//! The Q criteria are read from here because the judge is scored against them and a judge
//! prompt built from a copy would drift from the document reviewers read. The S descriptions
//! are exposed too, and `check.rs` does not consume them yet.

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

    #[test]
    fn the_real_rubric_parses() {
        let r = load(&rubric_path()).unwrap();
        assert_eq!(r.structural.len(), 7, "{:?}", r.structural);
        assert_eq!(r.quality.len(), 7, "{:?}", r.quality);
        assert_eq!(r.quality_ids(), ["Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"]);
        assert!(r.structural[0].description.contains("class definition"));
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
