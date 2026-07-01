pub mod check;
pub mod diff;
pub mod scenario;
pub mod snapshot;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

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

    // Write git log in default verbose format — check.rs S4/S5 parse from this
    let log = std::process::Command::new("git")
        .current_dir(worktree)
        .args(["log"])
        .output()
        .context("git log")?;
    std::fs::write(output.join("commit.log"), &log.stdout).context("writing commit.log")?;

    // Copy corpus/ → output/corpus/ (S1, S2, S3, S7)
    let corpus_src = worktree.join("corpus");
    if corpus_src.exists() {
        copy_dir(&corpus_src, &output.join("corpus"))?;
    }

    // Copy scaffold dirs → output/ (S6 checks for agents/, catalog/, skills/)
    for dir in ["agents", "catalog", "skills"] {
        let src = worktree.join(dir);
        if src.exists() {
            copy_dir(&src, &output.join(dir))?;
        }
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
