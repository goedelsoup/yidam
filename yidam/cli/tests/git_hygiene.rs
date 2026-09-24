//! What every `git` invocation owes, asserted from outside the crate.
//!
//! #929: `Command::new("git")` stood at fifty production sites across twenty-six files, and
//! the mechanics — which directory, which environment, which config, what to do with a
//! revision that might be an option — were re-decided at each one. The predictable
//! consequence is that a defence written at one site is absent at the others, and all three
//! of the cases below were exactly that.
//!
//! Each was reproduced against git 2.50.1 through the built binary before the runner
//! existed, and each fails on the commit before [`crate::git::run`] landed. They are here
//! rather than beside the runner because none of them is a question about argv: they are
//! questions about what a person standing in a repository gets back, and the argv is only
//! how it goes wrong.

use std::path::Path;
use std::process::Command;

/// Run the built CLI in `dir`, with extra environment.
fn run(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_yidam"));
    cmd.current_dir(dir).args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

/// A corpus of one commit, with whatever nodes `nodes` names.
fn corpus(dir: &Path, genesis: &str, nodes: &[(&str, &str)]) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@t.co"]);
    git(dir, &["config", "user.name", "T"]);
    std::fs::create_dir_all(dir.join(".yidam/corpus/concept")).unwrap();
    for (name, body) in nodes {
        std::fs::write(dir.join(".yidam/corpus/concept").join(name), body).unwrap();
    }
    git(dir, &["add", "-A"]);
    // `--allow-empty` so a decoy repository with no nodes still gets its genesis commit.
    git(
        dir,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "--no-gpg-sign",
            "-m",
            genesis,
        ],
    );
}

// ── 1. a leaked GIT_DIR ───────────────────────────────────────────────────────

/// **`GIT_DIR` in the environment overrides the directory we ask about, and `-C` does not
/// save you.** A git hook, `git rebase -x`, `git bisect run` and any wrapper that exports it
/// put yidam in this position, and the answer that comes back is another repository's,
/// with no error.
///
/// What made it worse than a wrong answer: `rev-parse --show-toplevel` answers about the
/// *working directory* regardless, so yidam's repository detection kept naming the corpus
/// the user was standing in while its reads described the other one. Measured before the
/// fix — the same command, in the same directory, twice:
///
/// ```text
/// $ yidam log                      genesis: INNER the real corpus
/// $ GIT_DIR=$OUTER/.git yidam log  genesis: OUTER DECOY CORPUS
/// ```
#[test]
fn a_leaked_git_dir_does_not_redirect_the_read() {
    let outer = tempfile::tempdir().unwrap();
    corpus(outer.path(), "genesis: OUTER DECOY CORPUS", &[]);
    let inner = tempfile::tempdir().unwrap();
    corpus(
        inner.path(),
        "genesis: INNER the real corpus",
        &[("a.yml", "class: concept\nname: a\n")],
    );

    let outer_git = outer.path().join(".git");
    let (leaked, _) = run(
        inner.path(),
        &["log"],
        &[("GIT_DIR", outer_git.to_str().unwrap())],
    );

    assert!(
        leaked.contains("INNER the real corpus"),
        "a leaked GIT_DIR redirected the read to another repository:\n{leaked}"
    );
    assert!(
        !leaked.contains("OUTER DECOY"),
        "yidam reported the enclosing GIT_DIR's history as this corpus's:\n{leaked}"
    );
}

// ── 2. core.quotepath ─────────────────────────────────────────────────────────

/// **A node whose filename is not ASCII disappeared from `yidam diff`.**
///
/// `core.quotepath` defaults to `true`, so `git diff --name-status` renders such a path as
/// `".yidam/corpus/concept/caf\303\251.yml"` — quoted, escaped, and no longer matching the
/// prefix the parser tests for. The change was not mangled in the output; it was **dropped**,
/// and the report said there was nothing to say.
///
/// The ASCII node is the control. Without it this test cannot tell "the parser handles both"
/// from "the diff found nothing at all", which is the shape the bug arrives in.
#[test]
fn a_non_ascii_node_is_not_dropped_from_the_diff() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    corpus(
        root,
        "genesis: seed",
        &[
            ("ascii.yml", "class: concept\nname: ascii\n"),
            ("café.yml", "class: concept\nname: cafe\n"),
        ],
    );
    std::fs::write(
        root.join(".yidam/corpus/concept/ascii.yml"),
        "class: concept\nname: ascii2\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/corpus/concept/café.yml"),
        "class: concept\nname: cafe2\n",
    )
    .unwrap();
    git(root, &["add", "-A"]);
    git(
        root,
        &["commit", "-q", "--no-gpg-sign", "-m", "revise: both"],
    );

    let (out, _) = run(root, &["diff", "HEAD~1..HEAD"], &[]);

    assert!(
        out.contains("ascii.yml"),
        "the control node is missing, so this test proves nothing about the other one:\n{out}"
    );
    assert!(
        out.contains("café.yml"),
        "a node git reports as modified is absent from the diff:\n{out}"
    );
}

// ── 3. a revision that is an option ───────────────────────────────────────────

/// **A revision reached git's argv unseparated, and a read-only report wrote a file.**
///
/// `cmd/query/at.rs` closed this for `--at` after `--at=--output=<path>` truncated the file
/// it named, and documented it. It closed it for its own three invocations. `log` and `diff`
/// take a revision from the command line the same way and had no separator at all:
///
/// ```text
/// $ yidam log -- --output=$T/pwned
/// No commits in --output=/…/pwned matching the filter.     # exit 0
/// $ ls $T/pwned
/// -rw-r--r--  84  pwned                                    # git wrote it
/// ```
///
/// Both commands are asserted, because the two sites shape the argument differently — `log`
/// passes the revision through as given, `diff` interpolates it into the leading token of a
/// `{before}..{after}` range — and a fix applied to one shape is not a fix to the other.
#[test]
fn a_revision_that_looks_like_an_option_is_not_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    corpus(
        root,
        "genesis: seed",
        &[("a.yml", "class: concept\nname: a\n")],
    );

    let spoil = tmp.path().join("pwned");
    let arg = format!("--output={}", spoil.display());

    let (_, code) = run(root, &["log", "--", &arg], &[]);
    assert!(
        !spoil.exists(),
        "`yidam log` passed a revision to git as an option and it wrote {} (exit {code})",
        spoil.display()
    );

    let ranged = format!("--output={}..HEAD", spoil.display());
    let (_, code) = run(root, &["diff", "--", &ranged], &[]);
    let written: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("pwned"))
        .collect();
    assert!(
        written.is_empty(),
        "`yidam diff` passed a revision to git as an option and it wrote {written:?} (exit {code})"
    );
}
