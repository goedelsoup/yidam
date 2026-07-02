use anyhow::{Context, Result};
use std::path::Path;

use crate::paths::repo_root;

/// One row of the `yidam phases` table: an active inquiry phase backed by a
/// `ma/*` or `rigpa/*` branch.
pub(crate) struct PhaseRow {
    pub name: String,
    pub ref_name: String,
    pub owner: String,
    pub started: String,
    pub commits: usize,
}

fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

/// The branch phase commits are counted against: `main` if it exists,
/// otherwise `master`.
fn base_branch(root: &Path) -> Option<String> {
    for name in ["main", "master"] {
        if git_stdout(root, &["rev-parse", "--verify", "--quiet", name]).is_some() {
            return Some(name.to_string());
        }
    }
    None
}

/// "substrate-survey" → "Substrate survey"
fn humanize(slug: &str) -> String {
    let spaced = slug.replace(['-', '_'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

pub(crate) fn collect_phases(root: &Path) -> Result<Vec<PhaseRow>> {
    let refs = git_stdout(
        root,
        &[
            "for-each-ref",
            "refs/heads/ma",
            "refs/heads/rigpa",
            "--format=%(refname:short)",
        ],
    )
    .context("listing ma/* and rigpa/* refs")?;

    let base = base_branch(root);
    let mut rows = Vec::new();

    for ref_name in refs.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let slug = ref_name.split_once('/').map(|(_, s)| s).unwrap_or(ref_name);

        let owner = git_stdout(root, &["log", "-1", "--format=%an", ref_name])
            .unwrap_or_else(|| "unknown".to_string());

        // Commits and start date are relative to the base branch: only work
        // unique to the phase counts. A phase with no unique commits yet
        // falls back to the tip commit's date.
        let (commits, started) = match &base {
            Some(b) if *b != ref_name => {
                let not_base = format!("^{b}");
                let count = git_stdout(root, &["rev-list", "--count", ref_name, &not_base])
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                let first_unique = git_stdout(
                    root,
                    &["log", "--reverse", "--format=%as", ref_name, &not_base],
                )
                .and_then(|s| s.lines().next().map(str::to_string))
                .filter(|s| !s.is_empty());
                let started = match first_unique {
                    Some(d) => d,
                    None => git_stdout(root, &["log", "-1", "--format=%as", ref_name])
                        .unwrap_or_else(|| "—".to_string()),
                };
                (count, started)
            }
            _ => {
                let count = git_stdout(root, &["rev-list", "--count", ref_name])
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                let started = git_stdout(root, &["log", "-1", "--format=%as", ref_name])
                    .unwrap_or_else(|| "—".to_string());
                (count, started)
            }
        };

        rows.push(PhaseRow {
            name: humanize(slug),
            ref_name: ref_name.to_string(),
            owner,
            started,
            commits,
        });
    }

    Ok(rows)
}

pub(crate) fn render_phases(rows: &[PhaseRow]) -> String {
    if rows.is_empty() {
        return "No active phases (no ma/* or rigpa/* branches).".to_string();
    }

    let headers = ["Phase", "Ref", "Owner", "Started", "Commits"];
    let cells: Vec<[String; 5]> = rows
        .iter()
        .map(|r| {
            [
                r.name.clone(),
                r.ref_name.clone(),
                r.owner.clone(),
                r.started.clone(),
                r.commits.to_string(),
            ]
        })
        .collect();

    let widths: Vec<usize> = (0..5)
        .map(|i| {
            cells
                .iter()
                .map(|c| c[i].chars().count())
                .chain(std::iter::once(headers[i].chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut lines = Vec::with_capacity(rows.len() + 2);
    let fmt_row = |cols: [&str; 5]| -> String {
        cols.iter()
            .enumerate()
            .map(|(i, c)| format!("{c:<width$}", width = widths[i]))
            .collect::<Vec<_>>()
            .join("   ")
            .trim_end()
            .to_string()
    };

    lines.push(fmt_row(headers));
    lines.push(
        widths
            .iter()
            .map(|w| "\u{2500}".repeat(*w))
            .collect::<Vec<_>>()
            .join("   "),
    );
    for c in &cells {
        lines.push(fmt_row([&c[0], &c[1], &c[2], &c[3], &c[4]]));
    }
    lines.join("\n")
}

/// Print the table of active inquiry phases (`ma/*` and `rigpa/*` branches).
pub fn phases() -> Result<()> {
    let root = repo_root()?;
    let rows = collect_phases(&root)?;
    println!("{}", render_phases(&rows));
    Ok(())
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

    fn init_repo() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.email", "test@test.com"]);
        git(&root, &["config", "user.name", "Tester"]);
        std::fs::write(root.join("a.txt"), "a").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", "chore: genesis — test"]);
        (tmp, root)
    }

    #[test]
    fn collect_phases_reports_unique_commits() {
        let (_tmp, root) = init_repo();
        git(&root, &["checkout", "-q", "-b", "ma/substrate-survey"]);
        std::fs::write(root.join("b.txt"), "b").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", "establish: substrate map"]);
        git(&root, &["checkout", "-q", "main"]);

        let rows = collect_phases(&root).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ref_name, "ma/substrate-survey");
        assert_eq!(rows[0].name, "Substrate survey");
        assert_eq!(rows[0].owner, "Tester");
        assert_eq!(rows[0].commits, 1);
        assert!(!rows[0].started.is_empty());
    }

    #[test]
    fn collect_phases_includes_rigpa_refs() {
        let (_tmp, root) = init_repo();
        git(&root, &["branch", "rigpa/glacial-review"]);

        let rows = collect_phases(&root).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ref_name, "rigpa/glacial-review");
        assert_eq!(rows[0].commits, 0);
    }

    #[test]
    fn render_phases_aligns_columns() {
        let rows = vec![
            PhaseRow {
                name: "Substrate survey".into(),
                ref_name: "ma/substrate".into(),
                owner: "goedelsoup".into(),
                started: "2026-06-20".into(),
                commits: 12,
            },
            PhaseRow {
                name: "Glacial review".into(),
                ref_name: "rigpa/glacial".into(),
                owner: "goedelsoup".into(),
                started: "2026-06-28".into(),
                commits: 4,
            },
        ];
        let out = render_phases(&rows);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("Phase"));
        assert!(lines[1].starts_with('\u{2500}'));
        assert!(lines[2].contains("ma/substrate"));
        assert!(lines[3].contains("rigpa/glacial"));
    }

    #[test]
    fn render_phases_empty_state() {
        assert!(render_phases(&[]).contains("No active phases"));
    }
}
