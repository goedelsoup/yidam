//! `yidam due` — the properties that are about the process rather than the arithmetic.
//!
//! Three of them, and each is a way the command could quietly become something else.
//!
//! **It writes nothing.** It reads the same `index_status_data` and catalog walk `status`
//! reads, and `status` is in `cmd::regen::GENERATORS` and rewrites a README block in whatever
//! repository it is pointed at. A command meant to be run from a cron job against a live
//! checkout must never acquire that.
//!
//! **Being owed does not fail a run.** The whole argument for a separate report is that a
//! corpus with three expired sources is doing what it is meant to do. A `due` that exits
//! nonzero teaches people to write `|| true`, and then the report is off.
//!
//! **The report says the same thing in both formats.** The prose and the JSON are read by
//! different consumers and must not be able to disagree about how much is owed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

/// The reports fixture, committed, with every clock declared and every one of them due.
///
/// A phase branch that has not settled, a catalog entry retrieved long before the TTL it is
/// given, and an open question — so the run under test is the loud one. A fixture where
/// nothing is due would pass the "exits zero" assertion for the wrong reason.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let src = fixture_dir().join("repo");
    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(&src).unwrap();
        let dest = tmp.path().join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
    let root = tmp.path();
    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(root)
            .args(args)
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap();
    };

    std::fs::create_dir_all(root.join(".yidam/catalog")).unwrap();
    std::fs::write(
        root.join(".yidam/catalog/gauge.md"),
        "---\nid: gauge\nretrieved: \"2020-01-01\"\nttl_days: 30\n\
         location:\n  - kind: url\n    value: https://example.test/gauge\n---\n\nA gauge record.\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join(".yidam/corpus/concept")).unwrap();
    std::fs::write(
        root.join(".yidam/corpus/concept/datum.yml"),
        "class: concept\nlabel: Datum\ndescription: |\n  Which datum the gauge reads is `[open]`.\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/config.toml"),
        "[catalog]\nttl_days = 30\n\n[due]\nquestions_after = 1\nphases_after = 1\nindex_after = 1\n",
    )
    .unwrap();

    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: reports fixture"]);
    // A phase that has not settled, so the phases clock has something to be late about.
    git(&["checkout", "-q", "-b", "phase/survey"]);
    std::fs::write(
        root.join(".yidam/corpus/concept/inflight.yml"),
        "class: concept\nlabel: In flight\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "scope: the survey"]);
    git(&["checkout", "-q", "main"]);
    tmp
}

struct Run {
    stdout: String,
    code: i32,
}

fn run(root: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// Every file's content, keyed by relative path. `.git/` is excluded — any git command
/// churns it, and it is not what "writes nothing" is about.
fn contents(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path().strip_prefix(root).unwrap().to_path_buf(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

fn json(root: &Path) -> serde_json::Value {
    let r = run(root, &["due", "--format", "json"]);
    serde_json::from_str(&r.stdout).expect("valid JSON")
}

/// The property a command run on a schedule against a live checkout has to keep.
#[test]
fn due_changes_nothing_on_disk() {
    let tmp = stage();
    let before = contents(tmp.path());
    assert!(!before.is_empty(), "the fixture staged nothing");

    let r = run(tmp.path(), &["due"]);
    assert_eq!(
        contents(tmp.path()),
        before,
        "due wrote to the repository it was pointed at"
    );
    // And it did have something to say — otherwise this passes for the uninteresting reason
    // that nothing needed reporting.
    assert!(r.stdout.contains("due   "), "{}", r.stdout);
}

/// Four clocks due, and it still exits zero. The reading this report exists to prevent is
/// that a number in it is a defect.
#[test]
fn a_repository_that_owes_on_every_clock_still_exits_zero() {
    let tmp = stage();
    let v = json(tmp.path());
    assert_eq!(v["format_version"], "1");
    assert_eq!(v["due"], 4, "{}", serde_json::to_string_pretty(&v).unwrap());
    assert_eq!(v["passed"], true);
    assert_eq!(run(tmp.path(), &["due"]).code, 0);

    // `--strict` is the only thing that changes that.
    assert_eq!(run(tmp.path(), &["due", "--strict"]).code, 1);
}

/// The set of clocks, keyed the way a consumer keys on them.
#[test]
fn the_json_report_carries_the_envelope_and_every_clock() {
    let tmp = stage();
    let v = json(tmp.path());
    let mut got: Vec<&str> = v["clocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    got.sort_unstable();
    assert_eq!(
        got,
        ["catalog", "index", "phases", "questions"],
        "the set of clocks changed"
    );
    for c in v["clocks"].as_array().unwrap() {
        assert!(c["question"].is_string(), "{c}");
        assert!(c["detail"].is_string(), "{c}");
        assert!(c["overdue"].is_u64(), "{c}");
    }
}

/// Two formats, one verdict. A report that gates differently depending on how it was asked
/// is the failure RFC-0016 states as absolute.
#[test]
fn text_and_json_agree_on_the_exit_code() {
    let tmp = stage();
    for args in [&["due"][..], &["due", "--strict"][..]] {
        let text = run(tmp.path(), args);
        let mut a = args.to_vec();
        a.extend_from_slice(&["--format", "json"]);
        let js = run(tmp.path(), &a);
        assert_eq!(
            text.code, js.code,
            "{args:?}: text exited {} and json exited {}",
            text.code, js.code
        );
    }
}

/// The questions clock counts what `open-questions` lists.
///
/// One predicate, two commands, and the remedy printed on an overdue questions clock sends
/// the reader to the other one. A clock counting a set that command does not list would send
/// somebody looking for a question that is not there.
#[test]
fn the_questions_clock_counts_what_open_questions_lists() {
    let tmp = stage();
    let listed = run(tmp.path(), &["open-questions", "--format", "json"]);
    let listed: serde_json::Value = serde_json::from_str(&listed.stdout).unwrap();
    let n = listed["open_questions"].as_array().unwrap().len();
    assert!(n > 0, "the fixture has no open question to compare against");

    let v = json(tmp.path());
    let clock = v["clocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "questions")
        .expect("a questions clock");
    // Stated before the count is read: on any other state the detail does not carry an
    // "of N", and `contains` would pass by finding nothing to disagree with.
    assert_eq!(clock["state"], "due", "{clock}");
    assert!(
        clock["detail"]
            .as_str()
            .unwrap()
            .contains(&format!("of {n} ")),
        "the clock counted a different set than open-questions listed: {clock}"
    );
}

/// Outside a derived repository it says so, rather than reporting four clean clocks.
#[test]
fn outside_a_derived_repository_it_refuses_rather_than_reporting_zero() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new("git")
        .current_dir(tmp.path())
        .args(["init", "-q", "-b", "main"])
        .status()
        .unwrap();
    let r = run(tmp.path(), &["due"]);
    assert_ne!(r.code, 0, "{}", r.stdout);
    assert!(
        !r.stdout.contains("Nothing is due"),
        "a directory that is not a corpus must not read as a corpus that owes nothing: {}",
        r.stdout
    );
}
