//! A gate must not pass on a repository it cannot see.
//!
//! `repo_root()` falls back to the working directory when `git rev-parse` fails, so every
//! report would run anywhere. For a report that is tolerable — it prints that it found
//! nothing. For `graph-check` and `lint` it was not: from an empty directory that was not
//! even a git repository, `graph-check` printed "No corpus content found" and **exited 0**,
//! and `lint` reported "0 finding(s), no errors".
//!
//! That made a misconfigured repository and a clean one the same observation. A derived
//! repository whose CI ran the gate from the wrong directory — or which never had the
//! infrastructure at all — would go green forever, and nothing anywhere would say so.
//!
//! Each test below asserts on the **exit code**, never on the prose. A gate that says the
//! right words and returns 0 is still broken.

use std::path::Path;
use std::process::Command;

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// A directory that is not a git repository at all.
fn bare() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// A git repository that yidam never bootstrapped: no `.yidam/`.
fn plain_git() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .status()
        .unwrap()
        .success();
    assert!(ok, "git init failed");
    tmp
}

/// A derived repository an hour after genesis: `.yidam/` exists, the corpus is empty.
///
/// This is the case the check must NOT reject, and the reason it tests for `.yidam/`
/// rather than for corpus content. An empty corpus is a legitimate state; an absent
/// `.yidam/` is a repository yidam is not looking at.
fn bootstrapped_but_empty() -> tempfile::TempDir {
    let tmp = plain_git();
    std::fs::create_dir_all(tmp.path().join(".yidam").join("corpus")).unwrap();
    tmp
}

#[test]
fn graph_check_fails_outside_a_git_repository() {
    let d = bare();
    let r = run(d.path(), &["graph-check"]);
    assert_ne!(
        r.code,
        0,
        "graph-check passed outside a repository: {r:?}",
        r = r.stdout
    );
    assert!(
        r.stderr.contains("not a yidam repository"),
        "unhelpful stderr: {}",
        r.stderr
    );
}

#[test]
fn graph_check_fails_in_a_git_repository_yidam_never_bootstrapped() {
    let d = plain_git();
    let r = run(d.path(), &["graph-check"]);
    assert_ne!(
        r.code, 0,
        "graph-check passed on a non-yidam repo: {}",
        r.stdout
    );
    assert!(
        r.stderr.contains(".yidam/"),
        "the message must name what is missing: {}",
        r.stderr
    );
}

#[test]
fn lint_fails_outside_a_yidam_repository() {
    let d = plain_git();
    let r = run(d.path(), &["lint"]);
    assert_ne!(
        r.code, 0,
        "lint reported clean on a non-yidam repo: {}",
        r.stdout
    );
}

/// The exemption is measured, not assumed: assert that the case being excused is real.
///
/// Without this, the check above could be satisfied by rejecting every repository, and a
/// freshly bootstrapped one — which has no nodes yet — would be unable to run its own gate.
#[test]
fn a_bootstrapped_repository_with_an_empty_corpus_still_passes() {
    let d = bootstrapped_but_empty();
    let r = run(d.path(), &["graph-check"]);
    assert_eq!(
        r.code, 0,
        "an empty corpus is a legitimate state and must pass\nstdout: {}\nstderr: {}",
        r.stdout, r.stderr
    );
    let l = run(d.path(), &["lint"]);
    assert_eq!(
        l.code, 0,
        "lint must run in a bootstrapped repo: {}",
        l.stderr
    );
}

/// `--version` must answer, and must name the build — not just the crate version.
///
/// Every correctness story here rests on the binary matching the pin in `.yidam.toml`, and
/// `yidam --version` used to be refused as an unexpected argument. Asserting on the shape
/// keeps it from decaying back into a bare `0.1.0`, which cannot distinguish two builds.
#[test]
fn version_names_the_build_and_its_features() {
    let d = bare();
    let r = run(d.path(), &["--version"]);
    assert_eq!(r.code, 0, "--version failed: {}", r.stderr);
    assert!(
        r.stdout.contains('(') && r.stdout.contains('['),
        "--version must carry the build commit and feature set, got: {}",
        r.stdout.trim()
    );
    assert!(
        r.stdout.contains("reports"),
        "the light default feature must be named: {}",
        r.stdout.trim()
    );
}
