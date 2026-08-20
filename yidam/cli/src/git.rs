use std::path::Path;

/// Hash of the root (genesis) commit.
///
/// `git log --reverse --max-count=1` does NOT work for this: git applies the
/// count limit before reversing, returning the newest commit instead.
fn genesis_hash(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .output()
        .ok()?;
    String::from_utf8(out.stdout)
        .ok()?
        .lines()
        .next()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

pub fn genesis_date(root: &Path) -> String {
    genesis_hash(root)
        .and_then(|hash| {
            let out = std::process::Command::new("git")
                .current_dir(root)
                .args(["log", "-1", "--format=%as", &hash])
                .output()
                .ok()?;
            String::from_utf8(out.stdout).ok()
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "no commits".to_string())
}

pub fn head_commit_short(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn genesis_message(root: &Path) -> String {
    genesis_hash(root)
        .and_then(|hash| {
            let out = std::process::Command::new("git")
                .current_dir(root)
                .args(["log", "-1", "--format=%B", &hash])
                .output()
                .ok()?;
            String::from_utf8(out.stdout).ok()
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// An active inquiry phase: a branch under `ma/*` or `rigpa/*`, wherever it lives.
#[derive(Debug, PartialEq, Eq)]
pub struct PhaseRef {
    /// The phase, with any remote prefix removed: `ma/substrate-survey`.
    pub name: String,
    /// The ref to read for it — the local branch when there is one, else the
    /// remote-tracking ref. Never assume this equals `name`.
    pub git_ref: String,
}

fn is_phase(name: &str) -> bool {
    name.starts_with("ma/") || name.starts_with("rigpa/")
}

/// Reduce `git for-each-ref --format=%(refname:short) refs/heads refs/remotes` to phases.
///
/// Deduped by name, because `ma/foo` and `origin/ma/foo` are one phase seen from two
/// sides. Local wins: it is the ref whose tip somebody is actually moving. Sorted, because
/// this feeds a committed REGEN block and an unstable order is a diff on every run.
pub(crate) fn parse_phase_refs(out: &str) -> Vec<PhaseRef> {
    let mut by_name: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        // A local branch is already its own name. A remote-tracking ref carries the remote
        // as a first segment — `origin/ma/foo` — and names the same phase. `origin/HEAD`
        // and every ordinary branch fall through both arms and are skipped.
        let name = if is_phase(line) {
            line
        } else {
            match line.split_once('/') {
                Some((_remote, rest)) if is_phase(rest) => rest,
                _ => continue,
            }
        };
        let entry = by_name
            .entry(name.to_string())
            .or_insert_with(|| line.to_string());
        if line == name {
            *entry = line.to_string();
        }
    }
    by_name
        .into_iter()
        .map(|(name, git_ref)| PhaseRef { name, git_ref })
        .collect()
}

/// Every active phase, read from local **and** remote-tracking refs.
///
/// This read `git branch --list` — local only — and that made it unusable for the thing it
/// feeds. `actions/checkout` creates exactly one local branch, so CI counted 0 against a
/// repository holding 24, and [`crate::cmd::status`] writes that count into a REGEN block:
/// the committed block and the regenerated one disagreed on every push, and the graph gate
/// failed regardless of what the commit contained. A derived repository ran that way for an
/// extended period and shipped a local stopgap.
///
/// The contract this now keeps, which is the one anything feeding a REGEN block owes:
/// **the same bytes in a fresh clone as on a developer machine.**
///
/// Two consequences, both deliberate. A phase held only on an unpushed local branch is
/// still counted here but will not be counted in CI — push it, or accept the disagreement.
/// And a developer whose remote-tracking refs are stale sees what their last fetch saw.
pub fn phase_refs(root: &Path) -> Vec<PhaseRef> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args([
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| parse_phase_refs(&s))
        .unwrap_or_default()
}

pub fn active_phase_count(root: &Path) -> usize {
    phase_refs(root).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(out: &str) -> Vec<String> {
        parse_phase_refs(out).into_iter().map(|p| p.name).collect()
    }

    /// The shape a fresh clone has: one local branch, everything else remote-tracking.
    /// This is what CI sees, and reading `refs/heads` alone scored it 0.
    #[test]
    fn phases_are_found_in_a_fresh_clone() {
        let out = "\
main
origin/HEAD
origin/main
origin/ma/auditor
origin/ma/advocate
origin/rigpa/payload-budget
";
        assert_eq!(
            names(out),
            ["ma/advocate", "ma/auditor", "rigpa/payload-budget"]
        );
    }

    /// A branch on both sides is one phase, not two.
    #[test]
    fn a_phase_on_both_sides_is_counted_once_and_read_locally() {
        let out = "ma/auditor\norigin/ma/auditor\n";
        let refs = parse_phase_refs(out);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "ma/auditor");
        // Local wins: it is the ref whose tip is actually moving.
        assert_eq!(refs[0].git_ref, "ma/auditor");
    }

    /// Order does not depend on which side git listed first — the rendered table is
    /// committed, so an unstable order is a diff on every run.
    #[test]
    fn remote_first_and_local_first_agree() {
        assert_eq!(
            parse_phase_refs("origin/ma/auditor\nma/auditor\n"),
            parse_phase_refs("ma/auditor\norigin/ma/auditor\n")
        );
    }

    /// A phase that exists only on a remote is read through the remote-tracking ref.
    #[test]
    fn a_remote_only_phase_carries_the_ref_that_resolves() {
        let refs = parse_phase_refs("origin/rigpa/schema-reach\n");
        assert_eq!(refs[0].name, "rigpa/schema-reach");
        assert_eq!(refs[0].git_ref, "origin/rigpa/schema-reach");
    }

    /// Ordinary branches are not phases, on either side, and `origin/HEAD` is not a
    /// branch at all.
    #[test]
    fn non_phase_refs_are_ignored() {
        let out = "main\ndevelop\nfeat/ma-something\norigin/HEAD\norigin/main\nmalformed\n";
        assert!(names(out).is_empty(), "{:?}", names(out));
    }

    /// `malformed` starts with the letters of `ma` and is not a phase; the boundary is
    /// the slash.
    #[test]
    fn the_prefix_boundary_is_a_slash() {
        assert!(names("malformed\nrigpaX\n").is_empty());
        assert_eq!(names("ma/x\n"), ["ma/x"]);
    }

    /// A remote is not always called `origin`.
    #[test]
    fn any_remote_name_works() {
        assert_eq!(names("upstream/rigpa/bar\n"), ["rigpa/bar"]);
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn genesis_message_returns_root_commit_not_newest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.co"]);
        git(root, &["config", "user.name", "Test"]);
        std::fs::write(root.join("a"), "a").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "chore: genesis — my-domain"]);
        std::fs::write(root.join("b"), "b").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "establish: something newer"]);

        assert_eq!(genesis_message(root), "chore: genesis — my-domain");
        assert!(!genesis_date(root).is_empty());
    }
}
