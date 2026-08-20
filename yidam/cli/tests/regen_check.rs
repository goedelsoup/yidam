//! `yidam regen --check` — REGEN freshness as a verdict.
//!
//! Its own file rather than a row in the golden matrix, and the reason is the defect this
//! command exists to close. The matrix stages one fixture and runs every command over it in
//! order; `yidam status` **writes** the REGEN block it renders, so by the time a shared
//! `regen --check` ran it would find the block current — and report the passing arm no
//! matter what the fixture held. A golden that flips when somebody reorders a list is worse
//! than no golden.
//!
//! So: stage per case, assert both arms.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for entry in walkdir::WalkDir::new(fixture_dir().join("repo"))
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry
            .path()
            .strip_prefix(fixture_dir().join("repo"))
            .unwrap()
            .to_path_buf();
        let dest = tmp.path().join(&rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(tmp.path())
            .args(args)
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap();
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: reports fixture"]);
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

fn readme(root: &Path) -> String {
    std::fs::read_to_string(root.join("README.md")).unwrap()
}

/// The fixture ships a placeholder in its `yidam status` block, so a fresh copy is stale.
#[test]
fn a_stale_block_is_named_and_gates() {
    let tmp = stage();
    let r = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(r.code, 1, "a stale block has to fail, or it is not a gate");
    assert!(r.stdout.contains("REGEN block(s) stale"), "{}", r.stdout);
    assert!(r.stdout.contains("README.md"), "{}", r.stdout);
    assert!(
        r.stdout.contains("(status)"),
        "names the generator: {}",
        r.stdout
    );
}

/// The whole promise: it reports without writing.
///
/// This is what lets it run against a tree with work in flight, which the run-and-diff step
/// it replaces cannot do — that one needs a clean tree to have anything to diff against.
#[test]
fn checking_writes_nothing() {
    let tmp = stage();
    let before = readme(tmp.path());
    run(tmp.path(), &["regen", "--check"]);
    assert_eq!(
        readme(tmp.path()),
        before,
        "--check rewrote the file it checked"
    );

    // And the generators' own output is suppressed, or a JSON report would have thirty
    // lines of corpus index in front of it.
    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    assert!(
        json.stdout.trim_start().starts_with('{'),
        "not a JSON document: {}",
        &json.stdout[..json.stdout.len().min(200)]
    );
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(doc["passed"], false);
    assert_eq!(doc["stale"][0]["generator"], "status");
}

/// Run the generators for real, then ask again. The gate has to be satisfiable.
#[test]
fn regen_then_check_passes() {
    let tmp = stage();
    let wrote = run(tmp.path(), &["regen"]);
    assert_eq!(wrote.code, 0, "{}", wrote.stdout);
    assert_ne!(readme(tmp.path()), "", "regen wrote something");

    let r = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(
        r.stdout.contains("Every REGEN block is current"),
        "{}",
        r.stdout
    );

    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(doc["passed"], true);
    assert_eq!(doc["stale"].as_array().unwrap().len(), 0);
}

/// Both formats agree on the verdict, as every other gate in this contract does.
#[test]
fn exit_codes_are_identical_across_formats() {
    let tmp = stage();
    for expected in [1, 0] {
        let text = run(tmp.path(), &["regen", "--check"]);
        let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
        assert_eq!(text.code, expected);
        assert_eq!(text.code, json.code);
        if expected == 1 {
            run(tmp.path(), &["regen"]);
        }
    }
}

/// Every field the report emits is declared in the committed schema.
///
/// The golden matrix's own version of this check only sees fields that appear in a golden,
/// and this report has none — see the module comment for why.
#[test]
fn every_emitted_field_is_declared_in_the_schema() {
    let tmp = stage();
    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();

    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().parent().unwrap().join("report.schema.json"))
            .unwrap(),
    )
    .unwrap();
    let declared = schema["properties"].as_object().unwrap();

    for key in doc.as_object().unwrap().keys() {
        assert!(
            declared.contains_key(key),
            "`regen --check` emits `{key}`, which report.schema.json does not declare"
        );
    }
    for required in schema["required"].as_array().unwrap() {
        assert!(doc.get(required.as_str().unwrap()).is_some(), "{required}");
    }
}
