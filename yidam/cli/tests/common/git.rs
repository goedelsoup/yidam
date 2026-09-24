//! The one place an integration test may spawn git.
//!
//! #929: thirty-eight test binaries under `tests/` wrote their own `Command::new("git")` —
//! seventy-six invocations, with no agreement on whether a failure reports git's stderr or
//! merely that a status was not zero. They are collapsed here so the gate in
//! `tests/git_spawns.rs` can name three files and carry no allowlist beside them.
//!
//! **These are fixtures, not [`yidam::git::Git`].** A fixture writes to the repository, has
//! no revision to separate from a pathspec and no user config to defend against; and running
//! it through the thing under test would let a bug in the runner build a repository that
//! hides it. The library's own `#[cfg(test)]` modules have the same helpers in
//! `src/git/fixture.rs` — two copies, because a `#[cfg(test)]` module is unreachable from
//! across the crate boundary and there is no third option.
//!
//! Every helper here captures stderr rather than letting it through to the test's own, so a
//! failed fixture says *why* in its assertion instead of somewhere above it in the log.

#![allow(dead_code)] // each test binary uses a different part of this

use std::path::Path;
use std::process::{Command, Output};

/// The date six of these fixtures already pinned, spelled once.
///
/// Fixed, because a commit stamped with the clock is a commit whose *shape* — its age, which
/// side of a TTL it falls on, how it sorts — differs between a developer's machine and a
/// runner in UTC. The library's fixtures use the same instant (`git::fixture::FIXTURE_DATE`),
/// so a golden recorded from either reads the same.
pub const FIXTURE_DATE: &str = "2026-01-01T00:00:00Z";

/// Run git in `dir` and hand back whatever it did — the terminal for a caller that reads a
/// failure rather than asserting past it.
pub fn raw(dir: &Path, args: &[&str]) -> Output {
    spawn(dir, args, None)
}

/// The same, with the author and committer dates pinned.
pub fn raw_at(dir: &Path, args: &[&str], date: &str) -> Output {
    spawn(dir, args, Some(date))
}

/// Run git in `dir` and assert it succeeded.
pub fn git(dir: &Path, args: &[&str]) {
    check(dir, args, spawn(dir, args, None));
}

/// The same, at a fixed date.
///
/// Both variables, because git takes the author's date and the committer's from different
/// places and a reader comparing a report to a fixture is comparing whichever one it prints.
pub fn git_at(dir: &Path, args: &[&str], date: &str) {
    check(dir, args, spawn(dir, args, Some(date)));
}

/// Stdout of a git command in `dir`, trimmed, asserting it succeeded.
pub fn out(dir: &Path, args: &[&str]) -> String {
    let o = spawn(dir, args, None);
    check(dir, args, o.clone());
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// The same, at a fixed date — for the fixtures whose committing helper also reads a hash back.
pub fn out_at(dir: &Path, args: &[&str], date: &str) -> String {
    let o = spawn(dir, args, Some(date));
    check(dir, args, o.clone());
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// Whether git succeeded, for the assertions that are *about* it failing.
pub fn succeeded(dir: &Path, args: &[&str]) -> bool {
    spawn(dir, args, None).status.success()
}

fn spawn(dir: &Path, args: &[&str], date: Option<&str>) -> Output {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    if let Some(d) = date {
        cmd.env("GIT_AUTHOR_DATE", d).env("GIT_COMMITTER_DATE", d);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("spawning git {args:?} in {}: {e}", dir.display()))
}

fn check(dir: &Path, args: &[&str], out: Output) {
    assert!(
        out.status.success(),
        "git {args:?} failed in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
}
