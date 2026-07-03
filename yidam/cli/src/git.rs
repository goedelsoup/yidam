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

pub fn active_phase_count(root: &Path) -> usize {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["branch", "--list", "ma/*", "rigpa/*"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
