//! The two hooks, held to #1444: silent outside a corpus, a brief at session start, a delta at
//! stop only when something moved, and nothing read when the fingerprint says nothing could
//! have.

use super::*;
use crate::git::fixture::{commit_at, git, init};
use serde_json::{json, Value};

fn node(root: &Path, rel: &str, body: &str) {
    let p = root.join(".yidam/corpus").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn today() -> i64 {
    crate::dates::days_from_civil_str("2026-06-01").unwrap()
}

fn repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    init(root);
    node(
        root,
        "concept/lonely.yml",
        "class: concept\nlabel: Lonely\n",
    );
    commit_at(root, "establish: lonely", "2026-01-01T00:00:00Z");
    tmp
}

fn record(root: &Path) -> Round {
    round::load(root, Some("main")).unwrap().unwrap()
}

fn message(out: &str) -> String {
    let v: Value = serde_json::from_str(out).unwrap();
    v["systemMessage"].as_str().unwrap().to_string()
}

/// Outside a corpus — no directory at all, or a git repository with no `.yidam/` — both hooks
/// print nothing and write nothing.
#[test]
fn outside_a_corpus_both_hooks_are_silent() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert_eq!(session_start(tmp.path(), today()), None);
    assert_eq!(stop(tmp.path(), &Value::Null, today()), None);
    init(tmp.path());
    assert_eq!(session_start(tmp.path(), today()), None);
    assert_eq!(stop(tmp.path(), &Value::Null, today()), None);
    assert!(!tmp.path().join(round::LOOP_DIR).exists());
}

/// The session's first context is the brief, saying there was no round before it, and the
/// round it read is recorded with a fingerprint for the first stop to compare against.
#[test]
fn session_start_prints_the_brief_and_records_the_round() {
    let tmp = repo();
    let root = tmp.path();
    let out = session_start(root, today()).unwrap();
    assert!(out.starts_with("yidam cycle --brief"), "{out}");
    assert!(out.contains("no round is recorded"), "{out}");
    assert!(record(root).fingerprint.is_some());

    // The second session is told what moved, not that nothing is recorded.
    node(root, "concept/stray.yml", "class: concept\nlabel: Stray\n");
    let out = session_start(root, today()).unwrap();
    assert!(!out.contains("no round is recorded"), "{out}");
    assert!(out.contains("stray.yml"), "{out}");
}

/// A turn that adds a finding is told so, once: the stop records the round, and the stop
/// after it, with nothing changed again, says nothing.
#[test]
fn a_stop_says_what_moved_and_then_nothing() {
    let tmp = repo();
    let root = tmp.path();
    session_start(root, today()).unwrap();

    node(root, "concept/stray.yml", "class: concept\nlabel: Stray\n");
    let out = stop(root, &json!({"stop_hook_active": false}), today()).unwrap();
    let msg = message(&out);
    assert!(msg.starts_with("yidam: since the round at"), "{msg}");
    assert!(
        msg.contains("+ finding") && msg.contains("stray.yml"),
        "{msg}"
    );

    assert_eq!(stop(root, &Value::Null, today()), None);
}

/// A fingerprint that matches skips the read. Shown by a record whose findings are a lie: a
/// read would report the difference, so silence means the read did not happen.
#[test]
fn a_matching_fingerprint_reads_nothing() {
    let tmp = repo();
    let root = tmp.path();
    session_start(root, today()).unwrap();
    let mut lie = record(root);
    lie.findings.clear();
    lie.due.push("invented".to_string());
    round::store(root, &lie).unwrap();

    assert_eq!(stop(root, &Value::Null, today()), None);

    // And with the fingerprint gone, the same record is read against, and the lie reported.
    lie.fingerprint = None;
    round::store(root, &lie).unwrap();
    let msg = message(&stop(root, &Value::Null, today()).unwrap());
    assert!(msg.contains("- due invented"), "{msg}");
}

/// Claude Code continuing already: return at once, and touch no record.
#[test]
fn a_stop_already_continuing_does_nothing() {
    let tmp = repo();
    let root = tmp.path();
    session_start(root, today()).unwrap();
    let before = record(root);
    node(root, "concept/stray.yml", "class: concept\nlabel: Stray\n");
    assert_eq!(
        stop(root, &json!({"stop_hook_active": true}), today()),
        None
    );
    assert_eq!(record(root), before);
}

/// A first stop has nothing to compare with. It records, and does not report the whole
/// round as having arrived.
#[test]
fn a_first_stop_records_quietly() {
    let tmp = repo();
    let root = tmp.path();
    assert_eq!(stop(root, &Value::Null, today()), None);
    assert!(record(root).fingerprint.is_some());
}

/// A long delta is cut, and says where the rest is.
#[test]
fn a_long_delta_is_cut() {
    let tmp = repo();
    let root = tmp.path();
    session_start(root, today()).unwrap();
    for i in 0..(STOP_LINES + 3) {
        node(
            root,
            &format!("concept/stray{i}.yml"),
            "class: concept\nlabel: Stray\n",
        );
    }
    let msg = message(&stop(root, &Value::Null, today()).unwrap());
    assert!(msg.contains("more — `yidam cycle --since-last`"), "{msg}");
    assert_eq!(
        msg.lines().filter(|l| l.starts_with("  + ")).count(),
        STOP_LINES
    );
}

/// What the fingerprint sees: an edit to a tracked file, a new untracked file, a new ref, a
/// commit, the day. And what it does not: an ignored file.
#[test]
fn the_fingerprint_moves_with_what_a_round_reads() {
    let tmp = repo();
    let root = tmp.path();
    let fp = || round::fingerprint(root, today()).unwrap();
    let base = fp();
    assert_eq!(fp(), base, "stable when nothing changes");
    assert_ne!(
        round::fingerprint(root, today() + 1).unwrap(),
        base,
        "the day"
    );

    std::fs::write(root.join(".gitignore"), "scratch/\n").unwrap();
    let ignored_rule = fp();
    assert_ne!(ignored_rule, base, "an untracked file");
    std::fs::create_dir_all(root.join("scratch")).unwrap();
    std::fs::write(root.join("scratch/x"), "x").unwrap();
    assert_eq!(fp(), ignored_rule, "an ignored file");

    let lonely = root.join(".yidam/corpus/concept/lonely.yml");
    std::fs::write(&lonely, "class: concept\nlabel: Lonely, edited\n").unwrap();
    let edited = fp();
    assert_ne!(edited, ignored_rule, "an edit to a tracked file");
    std::fs::write(&lonely, "class: concept\nlabel: Lonely, edited again\n").unwrap();
    assert_ne!(fp(), edited, "a second edit to an already-modified file");

    let before_ref = fp();
    git(root, &["branch", "propose/x"]);
    assert_ne!(fp(), before_ref, "a ref");

    // A round's own record is not something it reads.
    let before_store = fp();
    session_start(root, today()).unwrap();
    assert_eq!(fp(), before_store, "the loop's own record");
}
