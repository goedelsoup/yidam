//! `yidam doctor` — the one command you can point at a repository you did not write.
//!
//! Two properties are asserted here rather than in the unit tests, because both are about
//! the *process*: what it leaves behind on disk, and what it returns to a shell.
//!
//! The first is the reason this command exists in the shape it does. Ten subcommands
//! rewrite a README block in whatever repository they are run against — `status` most
//! notably, which reads like a read and is not. `doctor` calls the same generators, in the
//! same order, through `regen --check`'s non-writing mode. If that mode ever stops holding,
//! `doctor` silently becomes a writer, and the first person to find out will be someone who
//! ran it against a checkout they only meant to inspect.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

/// A derived repository with a deliberately stale `yidam status` REGEN block — the fixture
/// ships a placeholder, so a fresh copy is stale by construction. That is the case where a
/// writing `doctor` would be caught.
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

/// Every file's content, keyed by relative path. `.git/` is excluded — running any git
/// command churns it, and it is not what "writes nothing" is about.
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

/// The property the command is built around.
#[test]
fn doctor_changes_nothing_on_disk() {
    let tmp = stage();
    let before = contents(tmp.path());
    assert!(!before.is_empty(), "the fixture staged nothing");

    let r = run(tmp.path(), &["doctor"]);
    assert_eq!(
        contents(tmp.path()),
        before,
        "doctor wrote to the repository it was pointed at"
    );

    // And it did find the stale block — otherwise the assertion above passes for the
    // uninteresting reason that nothing needed writing.
    assert!(r.stdout.contains("regen"), "{}", r.stdout);
    assert_eq!(r.code, 1, "a stale REGEN block is a failing check");
}

/// `regen --check` and `doctor` must reach the same verdict about the same repository.
/// They share a generator list precisely so they cannot drift; this is the assertion that
/// the sharing survived.
#[test]
fn doctor_and_regen_check_agree_about_staleness() {
    let tmp = stage();
    let stale = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(stale.code, 1, "the fixture's status block ships stale");

    let doctor = run(tmp.path(), &["doctor"]);
    assert!(
        doctor.stdout.contains("fail ") && doctor.stdout.contains("block(s) stale"),
        "doctor did not report the staleness regen --check found:\n{}",
        doctor.stdout
    );

    // Refresh, and both must go green on the regen question.
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    assert_eq!(run(tmp.path(), &["regen", "--check"]).code, 0);
    let after = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&after.stdout).unwrap();
    let regen = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "regen")
        .expect("a regen check");
    assert_eq!(regen["verdict"], "ok", "{}", after.stdout);
}

/// The question a new collaborator asks first, asked from the wrong directory. One answer,
/// not seven — and a nonzero exit, because a gate that cannot see its repository must never
/// report that the repository is fine.
#[test]
fn outside_a_derived_repository_it_says_so_and_exits_nonzero() {
    let tmp = tempfile::tempdir().unwrap();
    let r = run(tmp.path(), &["doctor"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains("fail  repository"), "{}", r.stdout);
    // The build line is answerable anywhere, and is what a person debugging this needs.
    assert!(r.stdout.contains("build"), "{}", r.stdout);
    assert!(r.stdout.contains("skip"), "{}", r.stdout);
}

/// The report contract, not the prose. A consumer keys on `id` and `verdict`.
#[test]
fn the_json_report_carries_the_envelope_and_every_check() {
    let tmp = stage();
    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("valid JSON");
    assert_eq!(v["format_version"], "1");
    assert!(v["yidam"]["commit"].is_string());
    assert_eq!(v["passed"], false);

    let ids: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    let mut want = [
        "repository",
        "provenance",
        "binary",
        "path",
        "prelude",
        "index",
        "regen",
        "catalog",
        "corpora",
        "build",
    ];
    let mut got = ids.clone();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want, "the set of questions changed: {ids:?}");
}

/// A warning is not a failure, and `--strict` is how a CI job says it wants it to be.
/// Asserted on a repository whose only complaints are warnings.
#[test]
fn strict_is_what_turns_a_warning_into_a_nonzero_exit() {
    let tmp = stage();
    // Clear the one real failure so warnings are all that remain.
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    std::fs::write(
        tmp.path().join(".yidam.toml"),
        "[yidam]\ncommit = \"0123456789abcdef0123456789abcdef01234567\"\n\
         template = \"untagged\"\ncommitted = \"2020-01-01\"\n",
    )
    .unwrap();

    let lenient = run(tmp.path(), &["doctor"]);
    assert_eq!(
        lenient.code, 0,
        "warnings alone must not gate:\n{}",
        lenient.stdout
    );
    assert!(
        lenient.stdout.contains("nothing broken"),
        "{}",
        lenient.stdout
    );

    let strict = run(tmp.path(), &["doctor", "--strict"]);
    assert_eq!(strict.code, 1, "{}", strict.stdout);
}
