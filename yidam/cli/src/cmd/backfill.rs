use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

use yidam_core::git::{classify_commit, CommitKind};

/// A decision record extracted from an epistemic commit, serialized to
/// `.yidam/decisions/<shorthash>-<slug>.yml`. Matches the decision record
/// schema in docs/information-architecture.md; `id` and `summary` are the
/// fields the rest of the tooling (`yidam decisions-log`) reads.
#[derive(Serialize)]
struct BackfilledDecision {
    id: String,
    summary: String,
    verb: String,
    author: String,
    date: String,
    commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
}

/// Scan the git history of `target` and write a decision record into
/// `.yidam/decisions/` for every epistemic commit.
///
/// `since_ref` bounds the walk: when present, only commits reachable from HEAD
/// but not from `since_ref` are visited (i.e. `<since_ref>..HEAD`). When
/// absent the full history is scanned.
///
/// Re-running is idempotent: records are named by commit hash and existing
/// files are never overwritten. Corpus node extraction is not implemented —
/// a knowledge node needs a class schema and a curated concept, which cannot
/// be derived from commit metadata; that curation is left to agent workflows
/// over the backfilled decisions.
pub fn backfill_history(target: &Path, since_ref: Option<&str>) -> Result<()> {
    let range = match since_ref {
        Some(r) => format!("{r}..HEAD"),
        None => "HEAD".to_string(),
    };

    let output = std::process::Command::new("git")
        .args([
            "log",
            "--reverse",
            "--format=%H\x1f%s\x1f%b\x1f%an\x1f%aI\x1e",
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
        .split('\x1e')
        .filter(|s| !s.trim().is_empty())
        .filter_map(|block| CommitRecord::parse(block.trim()))
        .collect();

    if commits.is_empty() {
        println!("Backfill: no commits in range ({range}) — nothing to scan");
        return Ok(());
    }

    println!("Backfill: scanning {} commit(s) ({range})", commits.len());

    let decisions_dir = target.join(".yidam").join("decisions");
    std::fs::create_dir_all(&decisions_dir)
        .with_context(|| format!("creating {}", decisions_dir.display()))?;

    let mut written = 0usize;
    let mut existing = 0usize;
    let mut operational = 0usize;

    for c in &commits {
        let event = classify_commit(&c.hash, &c.subject);
        let short = &c.hash[..c.hash.len().min(8)];

        match event.kind {
            CommitKind::Operational => {
                operational += 1;
                println!("  {short} [Operational] {} — skipped", c.subject);
            }
            CommitKind::Epistemic => {
                let filename = format!("{short}-{}.yml", slugify(&c.subject));
                let path = decisions_dir.join(&filename);
                if path.exists() {
                    existing += 1;
                    println!("  {short} [Epistemic]   {} — exists", c.subject);
                    continue;
                }

                let record = BackfilledDecision {
                    id: format!("{short}-{}", slugify(&c.subject)),
                    summary: c.subject.clone(),
                    verb: event.verb,
                    author: c.author.clone(),
                    date: c.date.clone(),
                    commit: c.hash.clone(),
                    context: if c.body.is_empty() {
                        None
                    } else {
                        Some(c.body.clone())
                    },
                };
                std::fs::write(&path, serde_yaml::to_string(&record)?)
                    .with_context(|| format!("writing {}", path.display()))?;
                written += 1;
                println!("  {short} [Epistemic]   {} — written", c.subject);
            }
        }
    }

    println!();
    println!(
        "Backfill complete: {written} decision record(s) written to {}, \
         {existing} already present, {operational} operational commit(s) skipped.",
        decisions_dir.display()
    );
    println!(
        "Corpus node extraction is not derived from commit metadata — curate the \
         backfilled decisions into corpus nodes via agent workflows."
    );
    Ok(())
}

/// Standalone `yidam backfill` — runs against the current repository.
///
/// Requires the repo to be bootstrapped (`.yidam/` present); for a repo that
/// has not adopted yidam yet, `yidam overlay --backfill` is the entry point.
pub fn backfill(since_ref: Option<&str>) -> Result<()> {
    let root = crate::paths::repo_root()?;
    if !root.join(".yidam").exists() {
        anyhow::bail!(
            "no .yidam/ directory at {} — this repo is not bootstrapped.\n       \
             For a new repo, use `yidam overlay <target> --backfill` instead.",
            root.display()
        );
    }
    backfill_history(&root, since_ref)
}

fn slugify(subject: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = true; // suppress leading dashes
    for ch in subject.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
        if slug.len() >= 40 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

#[derive(Debug)]
struct CommitRecord {
    hash: String,
    subject: String,
    body: String,
    author: String,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn commit(dir: &Path, file: &str, message: &str) {
        std::fs::write(dir.join(file), message).unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", message]);
    }

    fn init_repo() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.email", "test@test.com"]);
        git(&root, &["config", "user.name", "Tester"]);
        (tmp, root)
    }

    fn decision_files(root: &Path) -> Vec<String> {
        let dir = root.join(".yidam").join("decisions");
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn backfill_writes_epistemic_and_skips_operational() {
        let (_tmp, root) = init_repo();
        commit(&root, "a.txt", "establish: confounding as a corpus concept");
        commit(&root, "b.txt", "regen: corpus index");

        backfill_history(&root, None).unwrap();

        let files = decision_files(&root);
        assert_eq!(
            files.len(),
            1,
            "only the epistemic commit is recorded: {files:?}"
        );
        assert!(files[0].contains("-establish-confounding-as-a-corpus"));
        assert!(files[0].ends_with(".yml"));

        let text =
            std::fs::read_to_string(root.join(".yidam").join("decisions").join(&files[0])).unwrap();
        let d: crate::parse::Decision = serde_yaml::from_str(&text).unwrap();
        assert_eq!(
            d.summary.as_deref(),
            Some("establish: confounding as a corpus concept")
        );
        assert!(d.id.is_some());
    }

    #[test]
    fn backfill_is_idempotent() {
        let (_tmp, root) = init_repo();
        commit(&root, "a.txt", "establish: first concept");

        backfill_history(&root, None).unwrap();
        let first = decision_files(&root);
        backfill_history(&root, None).unwrap();
        let second = decision_files(&root);

        assert_eq!(first, second);
    }

    #[test]
    fn backfill_respects_since_ref() {
        let (_tmp, root) = init_repo();
        commit(&root, "a.txt", "establish: old concept");
        git(&root, &["tag", "v1"]);
        commit(&root, "b.txt", "establish: new concept");

        backfill_history(&root, Some("v1")).unwrap();

        let files = decision_files(&root);
        assert_eq!(
            files.len(),
            1,
            "only commits after v1 are scanned: {files:?}"
        );
        assert!(files[0].contains("establish-new-concept"));
    }

    #[test]
    fn slugify_truncates_and_sanitizes() {
        assert_eq!(slugify("establish: A/B testing!"), "establish-a-b-testing");
        assert_eq!(slugify("???"), "untitled");
        assert!(slugify(&"x".repeat(100)).len() <= 40);
    }
}
