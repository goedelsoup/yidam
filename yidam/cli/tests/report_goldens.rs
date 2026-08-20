//! Golden fixtures for the report contract — RFC-0001's `reports/` family, RFC-0016 Phase 0.
//!
//! Two obligations, and the first is the one that constrains the design:
//!
//! 1. **`--format text` is byte-identical to what these commands have always printed.**
//!    JSON was added beside the prose path, not through it. A repository that never passes
//!    `--format` must not be able to tell this work happened.
//!
//! 2. **`--format json` matches a committed golden**, so a consumer versioned independently
//!    of the binary has something to write tests against — which is the whole point of
//!    promoting the reports from CLI behaviour to a contract.
//!
//! Run with `UPDATE_GOLDENS=1` to rewrite the expected files after an intended change. The
//! diff is the review.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

/// Copy the fixture repo into a tempdir and make it a git repository.
///
/// `repo_root()` shells out to `git rev-parse --show-toplevel`, so the reports cannot run
/// against a bare directory. Committing also gives `status` a genesis date to report.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&fixture_dir().join("repo"), tmp.path());
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .current_dir(tmp.path())
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    // A fixed author date keeps `status`'s genesis field stable across runs.
    Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-q", "-m", "genesis: reports fixture"])
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .status()
        .unwrap();
    tmp
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(from).unwrap();
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

/// Replace what belongs to the run rather than to the corpus.
///
/// The absolute root, the binary's version and its build commit all vary by machine and by
/// checkout. Redacting them is what lets the rest — every field name, every value, and the
/// order they appear in — be compared literally.
fn redact(out: &str, root: &Path) -> String {
    let root_s = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    out.replace(&root_s.display().to_string(), "<ROOT>")
        .replace(&root.display().to_string(), "<ROOT>")
        .replace(
            &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
            "\"version\": \"<VERSION>\"",
        )
        .replace(
            &format!("\"commit\": \"{}\"", env!("YIDAM_BUILD_COMMIT")),
            "\"commit\": \"<COMMIT>\"",
        )
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
        stdout: redact(&String::from_utf8_lossy(&out.stdout), root),
        code: out.status.code().unwrap_or(-1),
    }
}

fn assert_golden(name: &str, actual: &str) {
    let path = fixture_dir().join("expected").join(name);
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {}. Run with UPDATE_GOLDENS=1 to create it.",
            path.display()
        )
    });
    assert_eq!(
        expected, actual,
        "\n{name} drifted from its golden. If the change is intended, \
         re-run with UPDATE_GOLDENS=1 and review the diff.\n"
    );
}

/// Every command that grew a `--format` flag, in both formats.
const COMMANDS: &[(&str, &[&str])] = &[("lint", &["lint"])];

#[test]
fn text_output_is_byte_identical_to_its_golden() {
    let tmp = stage();
    for (name, args) in COMMANDS {
        let r = run(tmp.path(), args);
        assert_golden(&format!("{name}.txt"), &r.stdout);
    }
}

#[test]
fn json_output_matches_its_golden() {
    let tmp = stage();
    for (name, args) in COMMANDS {
        let mut a = args.to_vec();
        a.extend_from_slice(&["--format", "json"]);
        let r = run(tmp.path(), &a);
        let parsed: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or_else(|e| {
            panic!("{name} --format json is not valid JSON: {e}\n{}", r.stdout)
        });
        assert_eq!(parsed["format_version"], "1", "{name} handshake");
        assert_golden(&format!("{name}.json"), &r.stdout);
    }
}

/// The gate must not depend on how the answer was asked for.
#[test]
fn exit_codes_are_identical_across_formats() {
    let tmp = stage();
    for (name, args) in COMMANDS {
        let text = run(tmp.path(), args);
        let mut a = args.to_vec();
        a.extend_from_slice(&["--format", "json"]);
        let json = run(tmp.path(), &a);
        assert_eq!(
            text.code, json.code,
            "{name}: text exited {} and json exited {} — a gate that gates differently \
             depending on the output format is not a gate",
            text.code, json.code
        );
    }
}
