//! The `quality-report.json` contract, end to end and goldened.
//!
//! The unit tests in `quality.rs` grade the model. This grades the *document*: the binary's
//! arguments, the file it writes, the merge across fragments that separate runners produced,
//! and the committed golden a consumer versioned independently of this repository can write
//! tests against. That is why every other report here is goldened, and it is the reason
//! RFC-0025 asks for this one.
//!
//! Run with `UPDATE_GOLDENS=1` to rewrite the expected file after an intended change. The
//! diff is the review.
//!
//! # Where the golden lives
//!
//! Beside this crate, not in `yidam/prelude/sdks/parity/fixtures/reports/`. #467 said "beside
//! `tests/goldens/`, as every other report is goldened", and the report goldens are not
//! there: they are parity fixtures, and every one of them is a document three language SDKs
//! must reproduce byte for byte. Nothing reimplements a CI report, and putting this in that
//! directory would enlist the TypeScript and Python SDKs in a contract they have no part in.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn goldens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

/// A scratch repository the reporter can read provenance out of.
///
/// `git init` and a real manifest rather than a bare directory: `Provenance::read` walks to
/// the `.git` above the JUnit it was given and reads `yidam/cli/Cargo.toml` from there, and a
/// test that stubbed both would be asserting that its own stub works.
struct Stage {
    root: PathBuf,
}

impl Stage {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("ci-report-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("yidam/cli")).expect("scratch dir");
        std::fs::write(
            root.join("yidam/cli/Cargo.toml"),
            "[package]\nname = \"yidam\"\nversion = \"0.0.0-fixture\"\nedition = \"2021\"\n",
        )
        .expect("manifest");
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "fixture@yidam.test"],
            vec!["config", "user.name", "Fixture"],
            vec!["add", "-A"],
            vec!["commit", "-qm", "chore: genesis — quality report fixture"],
        ] {
            let ok = Command::new("git")
                .current_dir(&root)
                .args(&args)
                .status()
                .expect("git")
                .success();
            assert!(ok, "git {args:?} failed");
        }
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Copy a fixture in, so the reporter reads it from inside the scratch repository and the
    /// `root` it records is the scratch repository rather than this one.
    fn place(&self, fixture: &str, rel: &str) -> PathBuf {
        let dest = self.path(rel);
        std::fs::create_dir_all(dest.parent().unwrap()).expect("fixture dir");
        std::fs::copy(fixtures().join(fixture), &dest).expect("copy fixture");
        dest
    }

    /// Copy a fixture directory in, for the source root `absences` walks.
    fn place_dir(&self, fixture: &str, rel: &str) {
        let from = fixtures().join(fixture);
        let to = self.path(rel);
        std::fs::create_dir_all(&to).expect("fixture dir");
        for entry in std::fs::read_dir(&from).expect("fixture tree") {
            let entry = entry.expect("entry");
            std::fs::copy(entry.path(), to.join(entry.file_name())).expect("copy");
        }
    }

    fn run(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_ci-report"))
            .current_dir(&self.root)
            .args(args)
            .output()
            .expect("ci-report runs");
        assert!(
            out.status.success(),
            "ci-report {args:?} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn read_json(&self, rel: &str) -> serde_json::Value {
        let text = std::fs::read_to_string(self.path(rel))
            .unwrap_or_else(|e| panic!("{rel} was not written: {e}"));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Replace the two fields that cannot be golden — and fail if they were not there to replace.
///
/// The second half is the part that matters. A redaction that silently matches nothing turns
/// a golden into a test of the redactor, which is how `report_goldens.rs` came to need the
/// same assertion: it would keep passing after the field it was hiding stopped being emitted.
fn redact(value: &mut serde_json::Value) {
    let yidam = value
        .get_mut("yidam")
        .expect("the envelope has no `yidam` block");
    let commit = yidam["commit"]
        .as_str()
        .expect("`commit` is not a string")
        .to_string();
    assert!(
        commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit()),
        "`commit` is {commit:?}, which is not a short sha — the reporter did not read the \
         scratch repository, and this redaction is hiding nothing"
    );
    yidam["commit"] = serde_json::Value::String("<commit>".into());

    let root = value["root"].as_str().expect("`root` is not a string");
    assert!(
        Path::new(root).is_absolute(),
        "`root` is {root:?}, not an absolute path"
    );
    value["root"] = serde_json::Value::String("<root>".into());
}

fn assert_golden(name: &str, mut value: serde_json::Value) {
    redact(&mut value);
    let mut text = serde_json::to_string_pretty(&value).expect("serializes");
    text.push('\n');

    let path = goldens().join(name);
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::create_dir_all(goldens()).expect("goldens dir");
        std::fs::write(&path, &text).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} is unreadable ({e}). Run with UPDATE_GOLDENS=1 to create it.",
            path.display()
        )
    });
    assert_eq!(
        text,
        expected,
        "{} has moved. If the change is intended, re-run with UPDATE_GOLDENS=1 and review \
         the diff.",
        path.display()
    );
}

/// Every fixture is tracked in git, not merely present on disk.
///
/// `cli.lcov` was not. `*.lcov` is a coverage *output* everywhere else and sits in the same
/// global gitignore that once hid `.config/nextest.toml` from this repository — so the file
/// existed locally, every test passed locally, and the two that read it failed on the runner
/// and nowhere else. `git status` showed nothing either time.
///
/// Discovered from the directory rather than listed: a fixture added tomorrow is covered.
#[test]
fn every_fixture_is_tracked_in_git() {
    let dir = fixtures();
    let mut untracked = Vec::new();
    let mut seen = 0usize;
    let mut stack = vec![dir.clone()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).expect("fixtures are readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            seen += 1;
            let tracked = Command::new("git")
                .current_dir(&dir)
                .args(["ls-files", "--error-unmatch"])
                .arg(&path)
                .output()
                .expect("git")
                .status
                .success();
            if !tracked {
                untracked.push(path.display().to_string());
            }
        }
    }
    assert!(
        seen > 3,
        "only {seen} fixtures found; the walk is looking at the wrong tree"
    );
    assert!(
        untracked.is_empty(),
        "these fixtures are on this machine and not in the repository, so every test that \
         reads them passes here and fails on a runner:\n  {}\n\nA global gitignore is the \
         usual cause and `git status` will not show it. `git check-ignore -v <path>` names \
         the rule; a negation in this repository's own .gitignore outranks it.",
        untracked.join("\n  ")
    );
}

/// Two gates, merged the way CI merges them, against a committed golden.
///
/// The fixtures are the three shapes RFC-0025 names — a failing suite, a fully-skipped
/// suite, and a suite with no cases at all — plus the ordinary green gate a page has to
/// render differently from the second.
#[test]
fn the_merged_report_matches_its_golden() {
    let stage = Stage::new("golden");
    stage.place("cli.junit.xml", "yidam/cli/target/nextest/ci/junit.xml");
    stage.place("cli.list.json", "list.json");
    stage.place("cli.lcov", "yidam/cli/target/coverage.lcov");
    stage.place("change.diff", "change.diff");
    stage.place_dir("src", "yidam/cli/src");
    stage.place(
        "harness.junit.xml",
        "yidam/tests/harness/target/nextest/ci/junit.xml",
    );
    // The run's job list (#516). Deliberately not all-green, and deliberately not a
    // one-to-one match with the fragments — it carries the two shapes that a report of test
    // outcomes alone cannot express:
    //
    //   `ci (harness)`             every test passed and the job failed anyway, which is what
    //                              a step outside the tests does — `clippy -D warnings`, a
    //                              coverage render, a packaging check.
    //   `ci (cli · full features)` failed without writing a fragment at all, so no gate in
    //                              this document is about it. Before #516 it was absent, and
    //                              absent read exactly like "not configured".
    //
    // and two jobs still running, one of them the reporting job itself.
    stage.place("jobs.json", "jobs.json");

    // Relative, as the workflow passes them. `--src` is also the prefix `diff_coverage`
    // filters on, so an absolute path here would match no diff entry and the coverage block
    // would come back empty while every assertion about it still passed.
    stage.run(&[
        "--gate",
        "ci (cli)",
        "--junit",
        "yidam/cli/target/nextest/ci/junit.xml",
        "--list",
        "list.json",
        "--lcov",
        "yidam/cli/target/coverage.lcov",
        "--diff",
        "change.diff",
        "--src",
        "yidam/cli/src",
        "--features",
        "reports,tonpa,vault-s3",
        "--json",
        "fragments/cli.json",
    ]);
    stage.run(&[
        "--gate",
        "ci (harness)",
        "--junit",
        "yidam/tests/harness/target/nextest/ci/junit.xml",
        "--json",
        "fragments/harness.json",
    ]);
    stage.run(&[
        "merge",
        "--jobs",
        "jobs.json",
        "--from",
        "fragments",
        "--out",
        "quality-report.json",
    ]);

    assert_golden(
        "quality-report.json",
        stage.read_json("quality-report.json"),
    );
}

/// The document says the fully-skipped suite asserted nothing, and the golden above is not
/// the only thing that would notice.
///
/// A golden fails on *any* change, including the ones that are fine, so it is a poor place to
/// keep the one assertion this whole phase turns on. This says it directly: two suites, both
/// green to a runner, and only one of them exercised anything.
#[test]
fn a_fully_skipped_suite_is_distinguishable_from_a_passing_one() {
    let stage = Stage::new("skipped");
    let cli = stage.place("cli.junit.xml", "junit.xml");
    stage.run(&[
        "--gate",
        "ci (cli)",
        "--junit",
        cli.to_str().unwrap(),
        "--json",
        "fragment.json",
    ]);
    let report = stage.read_json("fragment.json");
    let suites = report["quality"]["gates"][0]["suites"]
        .as_array()
        .expect("no suites");

    let s3 = suites
        .iter()
        .find(|s| s["suite"] == "yidam::vault_s3")
        .expect("the skipped suite is not in the document");
    assert_eq!(s3["totals"]["failed"], 0);
    assert_eq!(s3["totals"]["passed"], 2, "the runner called both a pass");
    assert_eq!(
        s3["totals"]["asserted"], 0,
        "a suite that announced two skips and asserted nothing reports assertions"
    );

    let tokens = suites
        .iter()
        .find(|s| s["suite"] == "yidam::design_tokens")
        .expect("the failing suite is not in the document");
    assert_eq!(tokens["totals"]["asserted"], 1);
    assert_eq!(tokens["totals"]["failed"], 1);
}

/// `#[ignore]`d tests reach the document, and they are not counted as runs.
///
/// nextest omits them from JUnit entirely, so `yidam::example_corpus` exists in the listing
/// and nowhere else. A report built on the XML alone would not have the suite at all.
#[test]
fn an_ignored_only_suite_is_in_the_report_and_ran_nothing() {
    let stage = Stage::new("ignored");
    let cli = stage.place("cli.junit.xml", "junit.xml");
    let list = stage.place("cli.list.json", "list.json");
    stage.run(&[
        "--gate",
        "ci (cli)",
        "--junit",
        cli.to_str().unwrap(),
        "--list",
        list.to_str().unwrap(),
        "--json",
        "fragment.json",
    ]);
    let report = stage.read_json("fragment.json");
    let suites = report["quality"]["gates"][0]["suites"]
        .as_array()
        .expect("no suites");
    let example = suites
        .iter()
        .find(|s| s["suite"] == "yidam::example_corpus")
        .expect("a suite that exists only in the listing was dropped from the document");
    assert_eq!(example["totals"]["cases"], 0);
    assert_eq!(example["totals"]["ignored"], 1);
    assert_eq!(example["totals"]["asserted"], 0);
}

/// The gated file is unmeasured and contributes to no coverage arithmetic.
///
/// `embedding.rs` sits behind `#[cfg(feature = "index")] mod embedding;` and the measured
/// build did not compile it. Three added lines; counted as uncovered they would take the
/// percentage from 50% to 20% and name the index path untested.
#[test]
fn the_coverage_block_separates_unmeasured_from_uncovered() {
    let stage = Stage::new("coverage");
    stage.place("cli.junit.xml", "junit.xml");
    stage.place("cli.lcov", "coverage.lcov");
    stage.place("change.diff", "change.diff");
    stage.place_dir("src", "yidam/cli/src");
    stage.run(&[
        "--gate",
        "ci (cli)",
        "--junit",
        "junit.xml",
        "--lcov",
        "coverage.lcov",
        "--diff",
        "change.diff",
        "--src",
        "yidam/cli/src",
        "--features",
        "reports",
        "--json",
        "fragment.json",
    ]);
    let cov = &stage.read_json("fragment.json")["quality"]["gates"][0]["coverage"];
    assert_eq!(
        cov["added"], 4,
        "the gated file's lines entered the denominator"
    );
    assert_eq!(cov["uncovered"], 2);
    assert_eq!(cov["features"][0], "reports");

    let unmeasured = cov["unmeasured"].as_array().expect("no unmeasured list");
    let gated = unmeasured
        .iter()
        .find(|f| f["path"] == "yidam/cli/src/embedding.rs")
        .expect("the gated file is not in the unmeasured list");
    assert_eq!(gated["reason"], "gated");
    assert_eq!(gated["feature"], "index");
    assert_eq!(gated["added"], 3);
    assert!(
        !unmeasured.iter().any(|f| f["reason"] == "unexplained"),
        "a fixture file was classified as an unexplained absence: {unmeasured:?}"
    );
}

/// A merge that finds nothing fails rather than publishing an empty page.
#[test]
fn merging_an_empty_directory_is_refused() {
    let stage = Stage::new("empty");
    std::fs::create_dir_all(stage.path("nothing")).expect("dir");
    let out = Command::new(env!("CARGO_BIN_EXE_ci-report"))
        .current_dir(&stage.root)
        .args(["merge", "--from", "nothing", "--out", "report.json"])
        .output()
        .expect("runs");
    assert!(!out.status.success(), "an empty merge exited zero");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no fragments"), "unhelpful failure: {err}");
    assert!(
        !stage.path("report.json").exists(),
        "a refused merge still wrote a report"
    );
}
