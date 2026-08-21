pub mod check;
pub mod diff;
pub mod scenario;
pub mod snapshot;
pub mod transcript;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// The bootstrap protocol version — [VERSIONING.md](../../../../../VERSIONING.md), Layer 3.
///
/// Covers which structural checks exist, what the genesis commit must contain, the scenario
/// schema, and the result snapshot format. Every snapshot records the version it was taken
/// under, and `diff` refuses to compare across versions: an S-check that changed meaning
/// makes a pass→fail transition say nothing about the model.
///
/// 0.1.0 → 0.2.0 restates S1–S3 and S5–S7 against the instance corpus at `.yidam/corpus/`.
/// A major bump by this layer's table — existing checks changed — and the version the
/// document quoted for the whole of 0.1.0 without the constant existing at all.
pub const PROTOCOL_VERSION: &str = "0.2.0";

pub fn run(scenario: PathBuf, model: String, output: PathBuf) -> Result<()> {
    let scenario_data = scenario::load(&scenario)?;

    // Before the run, not after: the transcript streams into it while the agent works.
    std::fs::create_dir_all(&output).context("creating output directory")?;

    let worktree = tempfile::TempDir::new().context("creating temp worktree")?;
    let worktree_path = worktree.path().to_owned();

    prepare_worktree(&worktree_path)?;
    invoke_bootstrap(&worktree_path, &scenario_data, &model, &output)?;
    capture_state(&worktree_path, &output)?;

    let run = transcript::read(&output.join(TRANSCRIPT), &model)?;
    let structural = check::run_all(&output)?;
    snapshot::write(&output, &structural, Some(run.clone()))?;

    println!("{}", run.summary());
    println!(
        "structural: {}/{} passed",
        structural.passed(),
        structural.total()
    );
    if run.was_denied() {
        println!(
            "WARNING: {} tool call(s) were denied by the permission layer ({}).\n\
             This run was prevented from acting. The structural verdict above describes the \
             harness, not the model.",
            run.permission_denials.len(),
            run.permission_denials.join(", ")
        );
    }
    Ok(())
}

pub fn check(result: PathBuf) -> Result<()> {
    let report = check::run_all(&result)?;
    report.print();
    if report.any_failed() {
        std::process::exit(1);
    }
    Ok(())
}

pub fn diff(baseline: PathBuf, candidate: PathBuf) -> Result<()> {
    let base = snapshot::load(&baseline)?;
    let cand = snapshot::load(&candidate)?;
    let regressions = diff::compare(&base, &cand)?;
    if regressions.is_empty() {
        println!("no regressions");
    } else {
        for r in &regressions {
            println!("REGRESSION: {r}");
        }
        std::process::exit(1);
    }
    Ok(())
}

// ── worktree preparation ──────────────────────────────────────────────────────

fn find_template_root() -> Result<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("git rev-parse --show-toplevel")?;
    if !out.status.success() {
        anyhow::bail!("not inside the yidam git repository");
    }
    let path = String::from_utf8(out.stdout).context("git output utf8")?;
    Ok(PathBuf::from(path.trim()))
}

/// Directory names that are build output or local state, wherever they appear.
///
/// `prepare_worktree` used to copy the tree minus `.git`, which is 193,000 files — against
/// 29,000 once these are pruned. None of them would exist in a repository made by
/// `yidam clone`, so excluding them makes the worktree more faithful, not less.
const NOT_CONTENT: [&str; 7] = [
    ".git",
    ".local",
    "target",
    "node_modules",
    "dist",
    ".venv",
    "__pycache__",
];

/// Must this path be kept out of the tree the bootstrap agent runs in?
///
/// `yidam/tests/` is the instrument: the rubric, the judge's criteria, the harness, and each
/// scenario's `good_bootstrap_looks_like` — the reference description of a good result for
/// the very domain the agent is about to be asked about. HARNESS.md has always said the
/// bootstrap agent does not read it. Nothing made that true; the copy loop took the whole
/// tree, so the agent under evaluation ran in a directory containing its own answer key.
///
/// Held out here rather than trusted to the skill's reading list, because a list is an
/// instruction and this is a property. `the_scoring_layer_does_not_reach_the_worktree`
/// asserts it over the real tree.
fn is_held_out(rel: &Path) -> bool {
    rel.starts_with("yidam/tests")
}

/// The template files a prepared worktree receives, pruned at the directory so an excluded
/// tree is never walked into.
fn template_files(template: &Path) -> impl Iterator<Item = walkdir::DirEntry> + '_ {
    let template = template.to_owned();
    walkdir::WalkDir::new(&template)
        .into_iter()
        .filter_entry(move |e| {
            let name = e.file_name().to_string_lossy();
            if NOT_CONTENT.contains(&name.as_ref()) {
                return false;
            }
            match e.path().strip_prefix(&template) {
                Ok(rel) => rel.as_os_str().is_empty() || !is_held_out(rel),
                Err(_) => true,
            }
        })
        .filter_map(|e| e.ok())
}

fn prepare_worktree(dest: &Path) -> Result<()> {
    let template = find_template_root()?;

    for entry in template_files(&template) {
        let rel = entry
            .path()
            .strip_prefix(&template)
            .context("strip prefix")?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }

    // Initialize git repo
    run_git(dest, &["init", "-b", "main"])?;
    run_git(dest, &["config", "user.name", "harness"])?;
    run_git(dest, &["config", "user.email", "harness@yidam"])?;

    // Stage everything — the bootstrap agent makes the genesis commit
    run_git(dest, &["add", "-A"])?;

    Ok(())
}

// ── bootstrap agent invocation ────────────────────────────────────────────────

/// The event stream, as captured beside the result.
pub const TRANSCRIPT: &str = "transcript.jsonl";

fn invoke_bootstrap(
    worktree: &Path,
    scenario: &scenario::Scenario,
    model: &str,
    output: &Path,
) -> Result<()> {
    let bootstrap_content =
        std::fs::read_to_string(worktree.join("BOOTSTRAP.md")).context("reading BOOTSTRAP.md")?;

    let concepts: String = scenario
        .seed_concepts
        .iter()
        .map(|c| format!("- **{}**: {}", c.name, c.hint))
        .collect::<Vec<_>>()
        .join("\n");

    // Single-agent approximation: inline domain owner context so the bootstrap
    // agent can complete the full process without an interactive partner.
    // A two-agent implementation would run a separate domain owner agent that
    // responds only when queried, to properly test Q1 (clarifying questions).
    let prompt = format!(
        "{bootstrap_content}\n\n\
         ---\n\n\
         ## Harness context\n\n\
         You are running in the yidam bootstrap test harness. Complete the full \
         bootstrap using the domain context below — no interactive domain owner \
         is present. Use these details to drive the ontology discovery step, \
         then scaffold, seed the corpus, and make the genesis commit.\n\n\
         **Domain:** {domain}\n\
         **Central question:** {question}\n\
         **Seed concepts:**\n{concepts}",
        domain = scenario.domain,
        question = scenario.central_question,
    );

    // The agent is about to be given a permission posture that lets it run `rm -rf` and
    // `git commit` without being asked. What makes that safe is that it acts on a throwaway
    // copy — so check that, here, rather than trusting the caller to have passed one.
    let template = find_template_root()?;
    if worktree.starts_with(&template) || worktree == template {
        anyhow::bail!(
            "refusing to run the bootstrap agent inside the template at {}: the worktree must \
             be a disposable copy",
            template.display()
        );
    }

    let transcript = std::fs::File::create(output.join(TRANSCRIPT))
        .with_context(|| format!("creating {TRANSCRIPT}"))?;

    let status = std::process::Command::new("claude")
        .current_dir(worktree)
        .args([
            "--print",
            "--model",
            model,
            // The event stream, rather than the final text. `--verbose` is what makes
            // stream-json emit the per-turn events; without it there is a result and nothing
            // leading to it.
            "--verbose",
            "--output-format",
            "stream-json",
            // Under the default mode every Write is denied and the process still exits 0
            // reporting success — so a bootstrap run wrote nothing and the checks read that
            // as a corpus the model failed to produce. A person bootstrapping by hand
            // approves exactly these operations; the harness has to stand in for them, and
            // the guard above is what keeps the blast radius to the temporary copy.
            "--permission-mode",
            "bypassPermissions",
        ])
        .arg(&prompt)
        .stdout(std::process::Stdio::from(transcript))
        .status()
        .context("running `claude` CLI — is it installed and in PATH?")?;

    // A non-zero exit is not fatal here. The transcript is already on disk, and it is the
    // only thing that can say what went wrong — discarding it to report "exited non-zero"
    // throws away the evidence for the failure being reported.
    if !status.success() {
        eprintln!(
            "warning: the bootstrap agent exited with {status}; see {}",
            output.join(TRANSCRIPT).display()
        );
    }

    Ok(())
}

// ── state capture ─────────────────────────────────────────────────────────────

fn capture_state(worktree: &Path, output: &Path) -> Result<()> {
    std::fs::create_dir_all(output).context("creating output directory")?;

    // The verbose log, for a person reading the result directory, and for S4's commit count.
    let log = std::process::Command::new("git")
        .current_dir(worktree)
        .args(["log"])
        .output()
        .context("git log")?;
    std::fs::write(output.join("commit.log"), &log.stdout).context("writing commit.log")?;

    // Subject lines, oldest first, for S4. A correct run writes four commits — genesis, two
    // `consume:`, one `vendor:` — so the question S4 asks is which commits they are, and a
    // count cannot answer it.
    let subjects = std::process::Command::new("git")
        .current_dir(worktree)
        .args(["log", "--reverse", "--format=%H%x09%s"])
        .output()
        .context("git log --format=%H%x09%s")?;
    std::fs::write(output.join("commits.tsv"), &subjects.stdout).context("writing commits.tsv")?;

    // The genesis message raw, from the ROOT commit. S5 counts its lines, and `git log`'s
    // indentation and blank separators are formatting rather than message. `-n 1` would read
    // HEAD, which at the end of a correct run is the `vendor:` commit — a fixed string from
    // step 8 rather than anything the agent wrote.
    let root = std::process::Command::new("git")
        .current_dir(worktree)
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .output()
        .context("git rev-list --max-parents=0")?;
    let root = String::from_utf8_lossy(&root.stdout).trim().to_string();
    let msg = if root.is_empty() {
        Vec::new()
    } else {
        std::process::Command::new("git")
            .current_dir(worktree)
            .args(["log", "--format=%B", "-n", "1", &root])
            .output()
            .context("git log --format=%B <root>")?
            .stdout
    };
    std::fs::write(output.join("genesis.msg"), &msg).context("writing genesis.msg")?;

    // The whole of `.yidam/` — the corpus S1–S3/S7 read and the scaffold S6 reads. Copied
    // wholesale rather than per-directory: the checks decide what is required, and a capture
    // that omits a directory makes an absent check indistinguishable from an absent copy.
    let yidam = worktree.join(".yidam");
    if yidam.exists() {
        copy_dir(&yidam, &output.join(".yidam"))?;
    }

    Ok(())
}

// ── shared utilities ──────────────────────────────────────────────────────────

fn run_git(dir: &Path, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("git {} failed", args.join(" "));
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let rel = entry.path().strip_prefix(src).context("strip prefix")?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HARNESS.md's claim, as an assertion over the real tree rather than a sentence.
    #[test]
    fn the_scoring_layer_does_not_reach_the_worktree() {
        let template = find_template_root().expect("running inside the yidam repository");

        // The held-out material has to exist, or this passes because the directory is gone.
        assert!(
            template.join("yidam/tests/rubric.md").exists(),
            "the rubric moved; this test is now asserting nothing"
        );

        let leaked: Vec<String> = template_files(&template)
            .filter_map(|e| {
                e.path()
                    .strip_prefix(&template)
                    .ok()
                    .map(|r| r.to_string_lossy().into_owned())
            })
            .filter(|p| p.starts_with("yidam/tests"))
            .collect();
        assert!(
            leaked.is_empty(),
            "the agent under test would run in a tree containing its own scoring criteria:\n{}",
            leaked
                .iter()
                .take(10)
                .map(|p| format!("  {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// The positive control. A predicate that excluded everything would pass the test above.
    #[test]
    fn the_prelude_and_the_entry_prompt_do_reach_the_worktree() {
        let template = find_template_root().unwrap();
        let kept: std::collections::HashSet<String> = template_files(&template)
            .filter_map(|e| {
                e.path()
                    .strip_prefix(&template)
                    .ok()
                    .map(|r| r.to_string_lossy().into_owned())
            })
            .collect();
        for required in [
            "BOOTSTRAP.md",
            "yidam/prelude/GRAPH.md",
            "yidam/prelude/skills/bootstrap.md",
            "sadhana/corpus/README.md",
        ] {
            assert!(
                kept.contains(required),
                "{required} did not reach the worktree"
            );
        }
    }

    #[test]
    fn build_output_is_pruned_at_the_directory() {
        let template = find_template_root().unwrap();
        let count = template_files(&template).count();
        assert!(
            count < 60_000,
            "{count} files — target/ or node_modules/ is being walked into"
        );
    }
}
