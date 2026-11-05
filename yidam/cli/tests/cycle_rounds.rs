//! `cycle --record`, `--since-last` and `--brief`, through the binary (#1443).
//!
//! The delta's own behaviour is held in `cmd/cycle/round/tests.rs`, against the library. What
//! only the CLI can get wrong is held here: the flags reach the functions, a record made by one
//! process is read by the next, and the two combinations that mean nothing are refused rather
//! than half-honoured.

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

mod common;

use common::git::{git, git_at, FIXTURE_DATE};

fn repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let dir = root.join(".yidam/corpus/concept");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lonely.yml"), "class: concept\nlabel: Lonely\n").unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@t.co"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["add", "."]);
    git_at(
        root,
        &["commit", "-q", "-m", "establish: lonely"],
        FIXTURE_DATE,
    );
    tmp
}

fn cycle(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yidam"))
        .arg("cycle")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

fn json(out: &Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

/// One process records, the next compares, and the comparison is a delta — not the "no
/// record" sentence a lost write would produce.
#[test]
fn a_round_recorded_by_one_run_is_read_by_the_next() {
    let tmp = repo();
    let root = tmp.path();

    let first = json(&cycle(root, &["--since-last", "--format", "json"]));
    assert!(first.get("since_last").is_none());
    assert!(first["since_last_unread"]
        .as_str()
        .unwrap()
        .contains("no round is recorded"));

    json(&cycle(root, &["--record", "--format", "json"]));
    assert!(root.join(".yidam/loop/main.json").exists());

    std::fs::write(
        root.join(".yidam/corpus/concept/stray.yml"),
        "class: concept\nlabel: Stray\n",
    )
    .unwrap();
    let second = json(&cycle(root, &["--since-last", "--format", "json"]));
    assert!(second.get("since_last_unread").is_none(), "{second:#}");
    let arrived = second["since_last"]["findings"]["arrived"]
        .as_array()
        .unwrap();
    assert!(
        !arrived.is_empty()
            && arrived
                .iter()
                .all(|f| f["node"].as_str().unwrap().ends_with("stray.yml")),
        "{second:#}"
    );

    // And a plain run carries neither field: absent is "not asked".
    let plain = json(&cycle(root, &["--format", "json"]));
    assert!(plain.get("since_last").is_none() && plain.get("since_last_unread").is_none());
}

/// `--brief` is a text budget, and a JSON consumer asking for it would get the full report
/// while believing it was cut. Refused, saying so.
#[test]
fn a_brief_in_json_is_refused() {
    let tmp = repo();
    let out = cycle(tmp.path(), &["--brief", "--format", "json"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--brief"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A budget with no brief to spend it on is a usage error, not silently ignored.
#[test]
fn a_budget_needs_a_brief() {
    let tmp = repo();
    let out = cycle(tmp.path(), &["--budget", "100"]);
    assert!(!out.status.success());

    let out = cycle(tmp.path(), &["--brief", "--budget", "100"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("yidam cycle --brief"), "{text}");
    assert!(text.len() / 4 <= 100, "{} tokens:\n{text}", text.len() / 4);
}

/// `yidam hook --root` reads the corpus it names. `root_flag.rs` cannot hold this: a hook is
/// silent and exits zero on a directory holding no corpus, which is the one answer that test
/// tells a dropped flag by. So the proof is the write: run from a directory that is not a
/// corpus, the round record lands in the corpus named, and the brief is on stdout.
#[test]
fn a_hook_reads_the_corpus_its_root_names() {
    let tmp = repo();
    let root = tmp.path();
    let elsewhere = tempfile::TempDir::new().unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["hook", "session-start", "--root"])
        .arg(root)
        .current_dir(elsewhere.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("yidam cycle --brief"), "{text}");
    assert!(root.join(".yidam/loop/main.json").exists());
    assert!(!elsewhere.path().join(".yidam").exists());
}
