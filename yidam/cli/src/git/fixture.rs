//! Building a git repository to test against.
//!
//! #929: twenty-two test modules under `src/` spawned git forty-three times to build the
//! repositories they test against — the same six lines rewritten, with no agreement on
//! whether a commit is signed, dated or quiet, or on whether a failure says why. They are collapsed here so the gate in
//! `tests/git_spawns.rs` can name three files without an allowlist of twenty-two exemptions
//! standing beside it.
//!
//! **These are fixtures, not the runner.** They deliberately do not go through [`super::run`]:
//! a fixture needs to write to the repository, has no revision to separate and no user
//! config to defend against, and running it through the thing under test would mean a bug in
//! the runner could build a repository that hides it. [`git`] here spawns directly and
//! asserts, which is what a fixture owes.
//!
//! Integration tests under `tests/` cannot reach a `#[cfg(test)]` module of the library, so
//! they have the same helpers in `tests/common/git.rs`. Two copies, both named by the gate,
//! because the crate boundary leaves no third option.

use std::path::Path;
use std::process::Command;

/// Run git in `dir` and assert it succeeded.
///
/// Dates are pinned by [`commit`] rather than here, so a caller reaching for this directly
/// gets the clock — which is right for `init` and `config` and wrong for a commit whose date
/// an assertion will read.
pub(crate) fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawning git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
}

/// Stdout of a git command in `dir`, trimmed. For a fixture that needs to read back a hash
/// it just created.
pub(crate) fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawning git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// An initialized repository on `main` with an identity configured.
///
/// `-b main` rather than whatever `init.defaultBranch` says: half this suite asks about the
/// baseline by name, and a developer whose git defaults to `master` should not see different
/// failures from CI.
pub(crate) fn init(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", FIXTURE_EMAIL]);
    git(dir, &["config", "user.name", FIXTURE_AUTHOR]);
}

/// The author every fixture commit carries. Named, because three tests assert who wrote a
/// commit, and an assertion against a bare `"T"` reads as a typo rather than as this.
pub(crate) const FIXTURE_AUTHOR: &str = "T";
pub(crate) const FIXTURE_EMAIL: &str = "t@t.co";

/// The date every fixture commit carries unless one is given.
///
/// Fixed, because a commit stamped with the clock is a commit whose *shape* — its age, which
/// side of a TTL it falls on, how it sorts — changes between a developer's machine and a
/// runner in UTC.
pub(crate) const FIXTURE_DATE: &str = "2026-01-01T00:00:00Z";

/// Stage everything and commit at [`FIXTURE_DATE`].
pub(crate) fn commit(dir: &Path, msg: &str) {
    commit_at(dir, msg, FIXTURE_DATE);
}

/// Stage everything and commit at `date`, for the fixtures that assert about ordering or age.
pub(crate) fn commit_at(dir: &Path, msg: &str, date: &str) {
    git(dir, &["add", "-A"]);
    let out = Command::new("git")
        .current_dir(dir)
        .args(["commit", "-q", "--allow-empty", "--no-gpg-sign", "-m", msg])
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .output()
        .unwrap_or_else(|e| panic!("spawning git commit: {e}"));
    assert!(
        out.status.success(),
        "git commit -m {msg:?} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
}

/// [`init`] plus one commit — the smallest repository that has a genesis.
pub(crate) fn repo(dir: &Path, genesis: &str) {
    init(dir);
    commit(dir, genesis);
}

/// Write a file, creating its parent directories.
pub(crate) fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, body).unwrap();
}
