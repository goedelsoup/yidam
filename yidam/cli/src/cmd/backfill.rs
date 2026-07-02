use anyhow::{Context, Result};
use std::path::Path;

/// Scan the git history of `target` and accumulate knowledge, decisions, and
/// other structured artifacts into `.yidam/`.
///
/// `since_ref` bounds the walk: when present, only commits reachable from HEAD
/// but not from `since_ref` are visited (i.e. `<since_ref>..HEAD`). When
/// absent the full history is scanned.
pub fn backfill_history(target: &Path, since_ref: Option<&str>) -> Result<()> {
    let range = match since_ref {
        Some(r) => format!("{r}..HEAD"),
        None => "HEAD".to_string(),
    };

    let output = std::process::Command::new("git")
        .args([
            "log",
            "--reverse",
            "--format=%H\x1f%s\x1f%b\x1f%an\x1f%aI",
            &range,
        ])
        .current_dir(target)
        .output()
        .context("running git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git log failed: {stderr}");
    }

    let log = String::from_utf8(output.stdout).context("git log output utf8")?;

    let commits: Vec<CommitRecord> = log
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .filter_map(|block| CommitRecord::parse(block.trim()))
        .collect();

    if commits.is_empty() {
        println!("Backfill: no commits in range ({range}) — nothing to scan");
        return Ok(());
    }

    println!("Backfill: scanning {} commit(s) ({range})", commits.len());

    // TODO: classify each commit (epistemic, operational, decision) using
    //       the parity SDK's classify_commit function or an embedded ruleset.
    // TODO: extract claims from commit body and changed file content.
    // TODO: write decisions to .yidam/decisions/<hash>.yml and knowledge
    //       nodes to .yidam/corpus/<class>/<slug>.yml.

    for c in &commits {
        let kind = classify_kind(&c.subject);
        println!("  {} [{kind:?}] {}", &c.hash[..8], c.subject);
    }

    println!(
        "Backfill scan complete ({} commit(s) — extraction not yet implemented)",
        commits.len()
    );
    Ok(())
}

#[derive(Debug)]
struct CommitRecord {
    hash: String,
    subject: String,
    #[allow(dead_code)]
    body: String,
    #[allow(dead_code)]
    author: String,
    #[allow(dead_code)]
    date: String,
}

impl CommitRecord {
    fn parse(block: &str) -> Option<Self> {
        // git log --format uses \x1f as field separator
        let mut fields = block.splitn(5, '\x1f');
        Some(Self {
            hash: fields.next()?.trim().to_string(),
            subject: fields.next()?.trim().to_string(),
            body: fields.next()?.trim().to_string(),
            author: fields.next()?.trim().to_string(),
            date: fields.next()?.trim().to_string(),
        })
    }
}

#[derive(Debug)]
enum CommitKind {
    Epistemic,
    Operational,
}

// Matches the parity SDK spec (prelude/sdks/rust/src/git.rs OPERATIONAL_VERBS).
// Replace with a direct dependency on the parity SDK crate when it lands.
const OPERATIONAL_VERBS: &[&str] = &[
    "extract",
    "refresh",
    "compute",
    "index",
    "bundle",
    "reconcile",
    "build",
];

/// Pre-parity placeholder. Extracts the verb before the first ": " and looks it
/// up in OPERATIONAL_VERBS; everything else is Epistemic. Must be replaced with
/// a call to the parity SDK's `classify_commit` before write-out is implemented.
fn classify_kind(subject: &str) -> CommitKind {
    let verb = subject
        .find(": ")
        .map(|pos| subject[..pos].trim())
        .unwrap_or("");
    if OPERATIONAL_VERBS.contains(&verb) {
        CommitKind::Operational
    } else {
        CommitKind::Epistemic
    }
}
