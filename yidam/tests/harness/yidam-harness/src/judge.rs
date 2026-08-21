//! Invoking the judge — step 6 of [HARNESS.md](../../../HARNESS.md), which until now was
//! design rather than implementation.
//!
//! The judge is given the corpus inline rather than a directory to explore. Three reasons,
//! in order of how much they matter: a prompt is reproducible where a tool-using exploration
//! is not, so two runs of the same judge over the same result see the same thing; a judge
//! that cannot use tools cannot wander into `yidam/tests/` and read the criteria it is
//! applying; and a genesis corpus is thirteen instances of at most forty lines, which fits.

use anyhow::{Context, Result};
use std::path::Path;

use crate::quality::QualityReport;
use crate::rubric::Rubric;
use crate::scenario::Scenario;
use crate::transcript::RunRecord;

/// The scorer. Fixed rather than a matrix dimension: the model under test varies, and a
/// yardstick that moves with the thing it measures measures nothing.
pub const DEFAULT_JUDGE_MODEL: &str = "claude-opus-5";

/// What the judge is shown. Assembled here so the test can read it without a model.
pub struct Brief {
    pub guidance: String,
    pub criteria: String,
    pub scenario: String,
    pub corpus: String,
    pub history: String,
    pub run: String,
}

impl Brief {
    pub fn render(&self) -> String {
        format!(
            "{}\n\n---\n\n## The criteria you are scoring\n\n{}\n\n---\n\n\
             ## The scenario this repository was bootstrapped from\n\n{}\n\n---\n\n\
             ## The corpus that was produced\n\n{}\n\n---\n\n\
             ## The history\n\n{}\n\n---\n\n## The run\n\n{}\n\n---\n\n\
             ## Reply\n\n\
             Reply with a single JSON object and nothing else. For each criterion, in ID \
             order, fill `evidence` **before** deciding `band` — quote the node text, the \
             commit line, or the transcript fact the band rests on. If a criterion fails for \
             absence, say what is absent; that is evidence, and an empty list is not.\n\n\
             ```json\n\
             {{\n\
             \x20 \"criteria\": [\n\
             \x20   {{\"id\": \"Q1\", \"evidence\": [\"…\"], \"band\": \"pass|marginal|fail\", \
             \"rationale\": \"one or two sentences\"}}\n\
             \x20 ],\n\
             \x20 \"overall\": \"pass|marginal|fail\",\n\
             \x20 \"most_important_finding\": \"the single thing worth acting on\"\n\
             }}\n\
             ```\n\n\
             Score every criterion exactly once. Do not revise a band once you have moved \
             past it.",
            self.guidance, self.criteria, self.scenario, self.corpus, self.history, self.run
        )
    }
}

/// Drop a leading YAML frontmatter block.
///
/// `judge.md` opens with `---`, which is skill metadata and not guidance — and which, passed
/// to a CLI as the first characters of a prompt, is read as a flag. That is how this was
/// found: every unit test passed and the first real invocation died with
/// `unknown option '---\nname: judge…'`. Prompts now go over stdin as well, so the shape of
/// the text can no longer be mistaken for an argument, but the frontmatter should not reach
/// the judge either way.
fn without_frontmatter(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("---\n") else {
        return text;
    };
    match rest.find("\n---") {
        Some(end) => rest[end..].trim_start_matches("\n---").trim_start(),
        None => text,
    }
}

/// Build the brief from a captured result directory.
pub fn brief(
    guidance: &str,
    rubric: &Rubric,
    scenario: &Scenario,
    result_dir: &Path,
    run: Option<&RunRecord>,
) -> Result<Brief> {
    let criteria = rubric
        .quality
        .iter()
        .map(|c| format!("- **{}** — {}", c.id, c.description))
        .collect::<Vec<_>>()
        .join("\n");

    let scenario_text = format!(
        "**Domain:** {}\n\n**Central question:** {}\n\n**A good bootstrap looks like:** {}",
        scenario.domain, scenario.central_question, scenario.good_bootstrap_looks_like
    );

    Ok(Brief {
        guidance: without_frontmatter(guidance).to_string(),
        criteria,
        scenario: scenario_text,
        corpus: render_corpus(result_dir),
        history: render_history(result_dir),
        run: render_run(run),
    })
}

/// Every corpus file, with its path, in one block. Paths matter to Q4 — an edge's target is
/// a path — so they are shown rather than summarised away.
fn render_corpus(result_dir: &Path) -> String {
    let root = result_dir.join(crate::check::CORPUS_DIR);
    if !root.exists() {
        return "The corpus directory is absent. Nothing was produced.".into();
    }
    let mut files: Vec<_> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();

    if files.is_empty() {
        return "The corpus directory is present and empty. Nothing was produced.".into();
    }
    files
        .iter()
        .map(|path| {
            let rel = path.strip_prefix(&root).unwrap_or(path).display();
            let body =
                std::fs::read_to_string(path).unwrap_or_else(|e| format!("<unreadable: {e}>"));
            format!("### `{rel}`\n\n```yaml\n{}\n```", body.trim_end())
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_history(result_dir: &Path) -> String {
    let subjects = std::fs::read_to_string(result_dir.join("commits.tsv")).unwrap_or_default();
    let subjects: Vec<&str> = subjects
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(_, s)| s))
        .collect();
    let genesis = std::fs::read_to_string(result_dir.join("genesis.msg")).unwrap_or_default();

    format!(
        "Commit subjects, oldest first:\n\n{}\n\nThe genesis commit message in full:\n\n```\n{}\n```",
        if subjects.is_empty() {
            "(no commits)".to_string()
        } else {
            subjects
                .iter()
                .map(|s| format!("- `{s}`"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        genesis.trim_end()
    )
}

/// Facts about the run that bear on a criterion, and the caveat that goes with the one that
/// looks like it answers Q1 and does not.
fn render_run(run: Option<&RunRecord>) -> String {
    let Some(run) = run else {
        return "No run record — this result was re-scored from a captured directory.".into();
    };
    let mut lines = vec![format!(
        "- Assistant turns before the agent first wrote a file: {}",
        run.turns_before_first_write
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unknown".into())
    )];
    lines.push(
        "- **Caveat for Q1:** this run had no domain owner to ask. The bootstrap prompt \
         inlined the scenario and told the agent that no interactive partner was present, so \
         a count of zero says nothing about whether the agent would interrogate a person. \
         Score Q1 against what the transcript shows the agent *could* have done here, and say \
         in the evidence that the harness gave it nobody to ask."
            .to_string(),
    );
    if run.was_denied() {
        lines.push(format!(
            "- **This run was prevented from acting.** The permission layer refused: {}. \
             Anything missing from the corpus above may be missing for that reason.",
            run.permission_denials.join(", ")
        ));
    }
    lines.join("\n")
}

/// Run the judge and parse its verdict.
pub fn score(brief: &Brief, model: &str, expected: &[&str]) -> Result<QualityReport> {
    let output = crate::print_prompt(&["--model", model], &brief.render())
        .context("running `claude` for the judge — is it installed and in PATH?")?;

    let reply = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        anyhow::bail!(
            "the judge exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    crate::quality::parse(&reply, expected).context("reading the judge's verdict")
}

pub fn write(result_dir: &Path, report: &QualityReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(result_dir.join("quality.json"), json).context("writing quality.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rubric;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../..")
            .canonicalize()
            .unwrap()
    }

    fn test_scenario() -> Scenario {
        Scenario {
            id: "causal-inference".into(),
            domain: "Causal inference".into(),
            central_question: "How do we identify causal effects?".into(),
            seed_concepts: vec![],
            good_bootstrap_looks_like: "3–4 well-linked nodes".into(),
        }
    }

    fn fixture_result() -> PathBuf {
        repo_root().join("yidam/prelude/sdks/parity/fixtures/reports/basic/repo")
    }

    fn built() -> Brief {
        let rubric = rubric::load(&repo_root().join("yidam/tests/rubric.md")).unwrap();
        let guidance = std::fs::read_to_string(repo_root().join("yidam/tests/judge.md")).unwrap();
        brief(
            &guidance,
            &rubric,
            &test_scenario(),
            &fixture_result(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn the_brief_carries_the_corpus_with_its_paths() {
        let rendered = built().render();
        assert!(
            rendered.contains("concept/tailwater.yml"),
            "node paths reach the judge"
        );
        assert!(
            rendered.contains("Water downstream of a structure"),
            "node text reaches the judge"
        );
        assert!(
            rendered.contains("../concept/low-flow.yml"),
            "Q4 is about what an edge points at; the target has to be visible"
        );
    }

    /// The criteria come from rubric.md, so adding one there holds the judge to it without
    /// anyone editing this file.
    #[test]
    fn the_brief_states_every_criterion_the_rubric_states() {
        let rendered = built().render();
        for id in ["Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"] {
            assert!(rendered.contains(id), "{id} is not in the brief");
        }
    }

    #[test]
    fn the_brief_asks_for_evidence_before_the_band() {
        let rendered = built().render();
        let schema = rendered.rfind("\"evidence\"").unwrap();
        let band = rendered.rfind("\"band\"").unwrap();
        assert!(
            schema < band,
            "the schema must present evidence before band"
        );
        assert!(rendered.contains("**before** deciding `band`"));
    }

    /// The count that looks like it answers Q1 and does not.
    #[test]
    fn a_run_with_nobody_to_ask_says_so_in_the_brief() {
        let run = RunRecord {
            model_requested: "m".into(),
            model_resolved: None,
            session_id: None,
            num_turns: Some(4),
            duration_ms: None,
            total_cost_usd: None,
            subtype: None,
            is_error: None,
            permission_denials: vec![],
            turns_before_first_write: Some(0),
        };
        let rendered = render_run(Some(&run));
        assert!(rendered.contains("no domain owner to ask"));
        assert!(rendered.contains("Caveat for Q1"));
    }

    #[test]
    fn a_denied_run_warns_the_judge_not_to_read_absence_as_failure() {
        let run = RunRecord {
            model_requested: "m".into(),
            model_resolved: None,
            session_id: None,
            num_turns: None,
            duration_ms: None,
            total_cost_usd: None,
            subtype: None,
            is_error: None,
            permission_denials: vec!["Write".into()],
            turns_before_first_write: Some(0),
        };
        assert!(render_run(Some(&run)).contains("prevented from acting"));
    }

    /// The failure that got past every unit test and died on the first real invocation.
    #[test]
    fn the_skill_frontmatter_does_not_reach_the_judge() {
        let guidance = std::fs::read_to_string(repo_root().join("yidam/tests/judge.md")).unwrap();
        assert!(
            guidance.starts_with("---\n"),
            "judge.md still opens with frontmatter"
        );
        let stripped = without_frontmatter(&guidance);
        assert!(
            !stripped.starts_with('-'),
            "a prompt beginning with a dash reads as a flag"
        );
        assert!(!stripped.contains("description: Evaluate a bootstrap result"));
        assert!(stripped.starts_with("# Skill: judge"));
    }

    #[test]
    fn text_without_frontmatter_is_left_alone() {
        assert_eq!(
            without_frontmatter("# Title\n\nbody\n"),
            "# Title\n\nbody\n"
        );
        assert_eq!(
            without_frontmatter("---\nunterminated\n"),
            "---\nunterminated\n"
        );
    }

    #[test]
    fn an_absent_corpus_is_described_rather_than_rendered_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(render_corpus(tmp.path()).contains("absent"));
        std::fs::create_dir_all(tmp.path().join(crate::check::CORPUS_DIR)).unwrap();
        assert!(render_corpus(tmp.path()).contains("present and empty"));
    }
}
