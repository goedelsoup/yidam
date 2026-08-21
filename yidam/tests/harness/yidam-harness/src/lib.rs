pub mod check;
pub mod diff;
pub mod scenario;
pub mod snapshot;

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

    let worktree = tempfile::TempDir::new().context("creating temp worktree")?;
    let worktree_path = worktree.path().to_owned();

    prepare_worktree(&worktree_path)?;
    invoke_bootstrap(&worktree_path, &scenario_data, &model)?;
    capture_state(&worktree_path, &output)?;

    let structural = check::run_all(&output)?;
    snapshot::write(&output, &structural)?;
    println!(
        "structural: {}/{} passed",
        structural.passed(),
        structural.total()
    );
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

fn prepare_worktree(dest: &Path) -> Result<()> {
    let template = find_template_root()?;

    // Copy all template files, excluding .git/
    for entry in walkdir::WalkDir::new(&template)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !e.path().components().any(|c| c.as_os_str() == ".git"))
    {
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

fn invoke_bootstrap(worktree: &Path, scenario: &scenario::Scenario, model: &str) -> Result<()> {
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

    let status = std::process::Command::new("claude")
        .current_dir(worktree)
        .args(["--print", "--model", model])
        .arg(&prompt)
        .status()
        .context("running `claude` CLI — is it installed and in PATH?")?;

    if !status.success() {
        anyhow::bail!("bootstrap agent exited with non-zero status");
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
