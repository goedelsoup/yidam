//! `yidam gates` — the gate table, generated from the CI workflow rather than maintained beside it.
//!
//! A derived repository's README used to say, in a sentence somebody wrote, which checks
//! `mise run ci` runs. The sentence was right the day it was written and had no way to stay
//! right: the workflow gained a step and the sentence did not, and the reporting repository
//! found its gate list wrong in three hand-maintained places before generating the table from
//! `ci.yml` itself (#1066). This is that generator, upstream, so every derived repository gets
//! the table and the gate that holds it current.
//!
//! **It reads the workflow and nothing else.** `yidam regen --check` gates the block, so what
//! it holds has to be a function of the commit — and `.github/workflows/ci.yml` is tracked,
//! which is the whole of why this can be a REGEN block at all. It does not run the gates, ask
//! `mise` what it would run, or read `mise.toml`: the local composite is held to CI's by a
//! test upstream (`derived_repo_smoke.rs`), and a table that read both would have to decide
//! which one to believe when they disagreed.
//!
//! **Every `run:` step of every job, in file order.** Not only the three the sentence named.
//! A step that installs the binary can fail the build as surely as the step that runs
//! `regen --check`, and a reader asking *why is this push red* is served by a table that has
//! the failing step in it. The `notes` column carries what the table would otherwise mislead
//! on: a job that only runs once a layer exists, a step that reports and never gates, and the
//! directory a command runs in.

use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::path::Path;

use crate::regen::update_file_regen;

/// Where the workflow is read from, relative to the repository root.
pub(crate) const WORKFLOW: &str = ".github/workflows/ci.yml";

/// One `run:` step, reduced to what the table says about it.
#[derive(Debug, PartialEq, Eq)]
struct Row {
    job: String,
    step: String,
    runs: String,
    notes: Vec<String>,
}

pub fn gates(root: Option<&Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let content = render(&root)?;
    crate::regen::emit(&content);
    // A literal: `every_generator_in_the_crate_is_listed` reads call sites for the name they
    // write, and a constant is invisible to it.
    update_file_regen(&root.join("README.md"), "yidam gates", &content)
}

/// The block's content for this repository.
fn render(root: &Path) -> Result<String> {
    let path = root.join(WORKFLOW);
    if !path.exists() {
        return Ok(format!(
            "_No `{WORKFLOW}` — nothing gates a push to this repository._"
        ));
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {WORKFLOW}"))?;
    let workflow: serde_yaml::Value =
        serde_yaml::from_str(&text).with_context(|| format!("parsing {WORKFLOW}"))?;
    Ok(table(&rows(&workflow)))
}

/// Every `run:` step of every job, in the order the file gives them.
fn rows(workflow: &serde_yaml::Value) -> Vec<Row> {
    let Some(jobs) = workflow["jobs"].as_mapping() else {
        return Vec::new();
    };
    let layers = layers(workflow);
    let mut rows = Vec::new();
    for (id, job) in jobs {
        let id = id.as_str().unwrap_or_default().to_string();
        let name = job["name"].as_str().unwrap_or(&id).to_string();
        let job_notes = job_notes(job, &layers);
        let Some(steps) = job["steps"].as_sequence() else {
            continue;
        };
        for step in steps {
            let Some(run) = step["run"].as_str() else {
                continue;
            };
            let mut notes = Vec::new();
            if let Some(cond) = step["if"].as_str() {
                notes.push(format!("when `{}`", cond.trim()));
            }
            if step["continue-on-error"].as_bool() == Some(true) {
                notes.push("reports, gates nothing".to_string());
            }
            if let Some(dir) = step["working-directory"].as_str() {
                notes.push(format!("in `{dir}/`"));
            }
            notes.extend(job_notes.iter().cloned());
            rows.push(Row {
                job: name.clone(),
                step: step_name(step, run),
                runs: runs(run),
                notes,
            });
        }
    }
    rows
}

/// The notes every step of a job inherits: its `if:` and its default working directory.
fn job_notes(job: &serde_yaml::Value, layers: &[(String, String)]) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(cond) = job["if"].as_str() {
        let cond = cond.trim();
        // `needs.detect.outputs.crates == 'true'` is how the scaffold gates a layer's job on
        // the layer existing. Said as the path the reader can check, when the detect step
        // says which path that is; quoted as written otherwise.
        let layer = layers
            .iter()
            .find(|(key, _)| cond.contains(&format!("needs.detect.outputs.{key} == 'true'")));
        match layer {
            Some((_, path)) => notes.push(format!("only when `{path}` exists")),
            None => notes.push(format!("when `{cond}`")),
        }
    }
    if let Some(dir) = job["defaults"]["run"]["working-directory"].as_str() {
        notes.push(format!("in `{dir}/`"));
    }
    notes
}

/// What the `detect` job's outputs mean, read off its script.
///
/// The scaffold writes `test -f crates/Cargo.toml && echo "crates=true" …` per layer. Each
/// line that has both a `test -f <path>` and an `echo "<key>=true"` yields `(key, path)`;
/// a workflow without that shape yields nothing, and the job-level `if:` is quoted as written.
fn layers(workflow: &serde_yaml::Value) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let Some(steps) = workflow["jobs"]["detect"]["steps"].as_sequence() else {
        return found;
    };
    for step in steps {
        let Some(run) = step["run"].as_str() else {
            continue;
        };
        for line in run.lines() {
            let Some(path) = line.split_whitespace().skip_while(|w| *w != "-f").nth(1) else {
                continue;
            };
            let Some(key) = line
                .split("echo \"")
                .nth(1)
                .and_then(|rest| rest.split("=true").next())
            else {
                continue;
            };
            found.push((key.to_string(), path.to_string()));
        }
    }
    found
}

/// A step's name: `name:` where the author gave one, `id:` failing that, and the command's
/// first line otherwise — which is what the Actions UI shows for an unnamed step.
fn step_name(step: &serde_yaml::Value, run: &str) -> String {
    step["name"]
        .as_str()
        .or_else(|| step["id"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| first_line(run))
}

/// A one-line command, quoted; a script, described. Thirty lines of shell in a table cell
/// is not a table, and the step's name is what a reader finds it by in the Actions log.
fn runs(run: &str) -> String {
    let lines: Vec<&str> = run.lines().filter(|l| !l.trim().is_empty()).collect();
    match lines.as_slice() {
        [] => "_(empty)_".to_string(),
        [one] => format!("`{}`", one.trim()),
        many => format!("_a {}-line script_", many.len()),
    }
}

fn first_line(run: &str) -> String {
    run.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// A cell: pipes escaped, newlines gone, so a command containing either cannot break the row.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn table(rows: &[Row]) -> String {
    if rows.is_empty() {
        return format!("_`{WORKFLOW}` declares no `run:` step, so nothing gates a push._");
    }
    let jobs = {
        let mut seen: Vec<&str> = Vec::new();
        for r in rows {
            if !seen.contains(&r.job.as_str()) {
                seen.push(&r.job);
            }
        }
        seen.len()
    };
    let mut out = format!(
        "Generated from [`{WORKFLOW}`]({WORKFLOW}): {} step{} across {jobs} job{}, in the \
         order they run. Every row can fail a push unless its notes say otherwise.\n\n\
         | job | step | runs | notes |\n|---|---|---|---|\n",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" },
        if jobs == 1 { "" } else { "s" },
    );
    for r in rows {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            cell(&r.job),
            cell(&r.step),
            cell(&r.runs),
            cell(&r.notes.join("; "))
        );
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKFLOW_TEXT: &str = r#"
name: ci
jobs:
  detect:
    name: detect layers
    steps:
      - uses: actions/checkout@v4
      - id: check
        run: |
          test -f crates/Cargo.toml && echo "crates=true" >> "$GITHUB_OUTPUT" || echo "crates=false" >> "$GITHUB_OUTPUT"
  corpus:
    name: corpus (graph gate)
    steps:
      - uses: actions/checkout@v4
      - name: Say which yidam is answering
        run: yidam --version
      - name: Report how stale the vendored prelude is
        continue-on-error: true
        run: |
          set -eu
          echo one
          echo two
      - name: graph-check
        run: yidam graph-check
      - name: lint
        run: yidam lint --commits --range origin/main..HEAD
      - name: REGEN blocks are current
        run: yidam regen --check
  crates:
    name: crates (domain computer)
    needs: detect
    if: needs.detect.outputs.crates == 'true'
    defaults:
      run:
        working-directory: crates
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --workspace
      - run: cargo doc --no-deps | tee doc.log
        env:
          RUSTDOCFLAGS: -D warnings
"#;

    fn parsed() -> Vec<Row> {
        rows(&serde_yaml::from_str(WORKFLOW_TEXT).unwrap())
    }

    /// Every `run:` step, in file order, and no `uses:` step — an action is not a gate a
    /// reader can run.
    #[test]
    fn every_run_step_is_a_row_in_file_order() {
        let rows = parsed();
        let steps: Vec<&str> = rows.iter().map(|r| r.step.as_str()).collect();
        assert_eq!(
            steps,
            [
                "check",
                "Say which yidam is answering",
                "Report how stale the vendored prelude is",
                "graph-check",
                "lint",
                "REGEN blocks are current",
                "cargo test --workspace",
                "cargo doc --no-deps | tee doc.log",
            ]
        );
    }

    /// The three gates the hand-written sentence used to name are rows, quoted verbatim.
    #[test]
    fn the_corpus_gates_are_quoted_as_commands() {
        let rows = parsed();
        let runs: Vec<&str> = rows
            .iter()
            .filter(|r| r.job == "corpus (graph gate)")
            .map(|r| r.runs.as_str())
            .collect();
        assert!(runs.contains(&"`yidam graph-check`"), "{runs:?}");
        assert!(
            runs.contains(&"`yidam lint --commits --range origin/main..HEAD`"),
            "{runs:?}"
        );
        assert!(runs.contains(&"`yidam regen --check`"), "{runs:?}");
    }

    /// A script is described by its length rather than pasted; a step that may fail without
    /// failing the build says so.
    #[test]
    fn a_script_is_described_and_an_advisory_step_is_marked() {
        let rows = parsed();
        let report = rows
            .iter()
            .find(|r| r.step.starts_with("Report how stale"))
            .unwrap();
        assert_eq!(report.runs, "_a 3-line script_");
        assert_eq!(report.notes, ["reports, gates nothing"]);
    }

    /// A layer job's condition is said as the path that switches it on, read off the detect
    /// step, and its default directory is carried onto every row.
    #[test]
    fn a_conditional_job_names_the_path_that_enables_it() {
        let rows = parsed();
        let test = rows
            .iter()
            .find(|r| r.runs == "`cargo test --workspace`")
            .unwrap();
        assert_eq!(test.job, "crates (domain computer)");
        assert_eq!(
            test.notes,
            ["only when `crates/Cargo.toml` exists", "in `crates/`"]
        );
    }

    /// A pipe in a command would end the cell; it is escaped, and the table keeps its columns.
    #[test]
    fn a_pipe_in_a_command_does_not_break_the_row() {
        let text = table(&parsed());
        let doc = text
            .lines()
            .find(|l| l.contains("cargo doc"))
            .expect("the doc row");
        assert!(doc.contains("--no-deps \\| tee"), "{doc}");
        assert_eq!(doc.matches(" | ").count(), 3, "four cells: {doc}");
    }

    /// The table's lead counts what the table holds, so a reader can see a row is missing.
    #[test]
    fn the_lead_counts_steps_and_jobs() {
        let text = table(&parsed());
        assert!(text.starts_with(&format!(
            "Generated from [`{WORKFLOW}`]({WORKFLOW}): 8 steps across 3 jobs"
        )));
        assert_eq!(text.lines().filter(|l| l.starts_with("| ")).count(), 9);
    }

    /// The scaffold's own workflow parses and yields the gates it ships — the fixture above
    /// is a sketch of it, and this is the check that the sketch is still true of the file.
    #[test]
    fn the_scaffolded_workflow_yields_its_three_corpus_gates() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join("sadhana/github/workflows/ci.yml")).unwrap();
        let rows = rows(&serde_yaml::from_str(&text).unwrap());
        for gate in [
            "`yidam graph-check`",
            "`yidam lint --commits --range origin/main..HEAD`",
            "`yidam regen --check`",
        ] {
            assert!(
                rows.iter().any(|r| r.runs == gate),
                "the scaffold's workflow no longer runs {gate}, or this generator cannot see it"
            );
        }
        assert!(
            rows.iter().any(|r| r
                .notes
                .iter()
                .any(|n| n == "only when `crates/Cargo.toml` exists")),
            "the crates job's condition is no longer read as the path that enables it"
        );
    }

    /// Nothing to read is a sentence, not an error: a repository that has not installed the
    /// workflow still runs `regen`.
    #[test]
    fn a_repository_without_the_workflow_renders_a_sentence() {
        let dir = tempfile::tempdir().unwrap();
        let text = render(dir.path()).unwrap();
        assert!(text.contains("No `.github/workflows/ci.yml`"), "{text}");
    }
}
