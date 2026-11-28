//! The golden helper's rule, and the claim that it is the only one (#1634).
//!
//! `common::golden` is one file compiled into three targets, so its rule is tested once,
//! here, against an environment passed in: setting `UPDATE_GOLDENS` or `CI` in a test would
//! change what every other test running beside it in this process decides.
//!
//! The last two tests hold what the helper cannot hold by existing: that nothing else in this
//! crate reads the variable, and that the retired names are written nowhere a person would
//! copy them from.

use std::ffi::OsString;
use std::path::Path;

use crate::common::golden::{compare, decide, diff, RETIRED, VARIABLE};
use crate::common::{repo_root, repo_walk};

fn env<'a>(set: &'a [&'a str]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| set.contains(&name).then(|| OsString::from("1"))
}

#[test]
fn the_variable_alone_turns_rewriting_on() {
    assert_eq!(decide(env(&[])), Ok(false));
    assert_eq!(
        decide(env(&["CI"])),
        Ok(false),
        "CI alone is an ordinary comparison"
    );
    assert_eq!(decide(env(&[VARIABLE])), Ok(true));
}

#[test]
fn rewriting_is_refused_where_ci_is_set() {
    let refusal = decide(env(&[VARIABLE, "CI"])).expect_err("refused under CI");
    assert!(
        refusal.contains("CI"),
        "the refusal names the rule: {refusal}"
    );
}

#[test]
fn a_retired_name_is_refused_with_the_name_that_replaced_it() {
    for old in RETIRED {
        for set in [vec![*old], vec![*old, VARIABLE]] {
            let refusal = decide(env(&set)).expect_err("a retired name is refused");
            assert!(
                refusal.contains(old) && refusal.contains(&format!("{VARIABLE}=1")),
                "{set:?}: the refusal names both variables: {refusal}"
            );
        }
    }
}

#[test]
fn a_missing_golden_fails_and_says_how_to_write_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent.txt");
    let failure = compare(&path, "x\n", false).expect_err("missing is a failure");
    assert!(failure.contains(&format!("{VARIABLE}=1")), "{failure}");
    assert!(!path.exists(), "a comparison writes nothing");
}

#[test]
fn rewriting_writes_the_golden_its_directory_and_nothing_when_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/deeper/golden.txt");
    compare(&path, "one\n", true).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\n");
    assert_eq!(compare(&path, "one\n", false), Ok(()));

    // Unchanged content is not rewritten: a regeneration over a clean tree touches nothing.
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    compare(&path, "one\n", true).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn a_drift_is_reported_as_a_line_diff() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("golden.txt");
    std::fs::write(&path, "a\nb\nc\nd\ne\nf\ng\n").unwrap();
    let failure = compare(&path, "a\nb\nc\nD\ne\nf\ng\nh\n", false).expect_err("drifted");
    assert!(failure.contains(&format!("{VARIABLE}=1")), "{failure}");
    // Changes within twice the context of each other share one hunk.
    let body: Vec<&str> = failure
        .lines()
        .skip_while(|l| !l.starts_with("@@"))
        .collect();
    assert_eq!(
        body,
        [
            "@@ committed line 2, produced line 2 @@",
            "  b",
            "  c",
            "- d",
            "+ D",
            "  e",
            "  f",
            "  g",
            "+ h",
        ]
    );
    // Further apart, two hunks, each headed by where it starts in both texts.
    let long: String = (0..20).map(|i| format!("{i}\n")).collect();
    let moved = long
        .replacen("3\n", "three\n", 1)
        .replacen("16\n", "sixteen\n", 1);
    let headers: Vec<String> = diff(&long, &moved)
        .lines()
        .filter(|l| l.starts_with("@@"))
        .map(String::from)
        .collect();
    assert_eq!(
        headers,
        [
            "@@ committed line 2, produced line 2 @@",
            "@@ committed line 15, produced line 15 @@"
        ]
    );
}

#[test]
fn a_difference_in_the_last_newline_is_shown() {
    let shown = diff("a\nb\n", "a\nb");
    assert!(shown.contains("- b") && shown.contains("+ b"), "{shown}");
    assert!(shown.contains("(no newline at end of file)"), "{shown}");
}

#[test]
fn a_long_diff_is_cut_and_counted() {
    let old: String = (0..400).map(|i| format!("{i}\n")).collect();
    let shown = diff(&old, "");
    assert!(
        shown.lines().count() <= 121,
        "{} lines",
        shown.lines().count()
    );
    assert!(shown.contains("more diff lines"), "{shown}");
}

/// Every reading of the variable in this crate goes through `golden::updating`, so the rule
/// above is the rule everywhere. A site reading the environment itself would rewrite under
/// CI, or ignore a retired name, without anything here noticing.
#[test]
fn nothing_else_in_the_crate_reads_the_variable() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let helper = Path::new("tests/common/golden.rs");
    let quoted = format!("\"{VARIABLE}\"");
    let mut readers = Vec::new();
    for dir in ["src", "tests"] {
        for entry in repo_walk(&crate_dir.join(dir)) {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let rel = path.strip_prefix(crate_dir).unwrap_or(path);
            let text = std::fs::read_to_string(path).unwrap_or_default();
            if rel != helper && text.contains(&quoted) {
                readers.push(rel.display().to_string());
            }
        }
    }
    assert!(
        readers.is_empty(),
        "{readers:?} spell {quoted} as a string, so they read it without `golden::updating` \
         and its refusals. Call `crate::common::golden::updating()` (or `assert`/`check`)."
    );
}

/// A retired name in a README or a comment is an instruction that now fails; this keeps the
/// only mentions the helper and this file.
#[test]
fn the_retired_names_are_written_nowhere_else() {
    let root = repo_root();
    let allowed = [
        "yidam/cli/tests/common/golden.rs",
        "yidam/cli/tests/gates/golden_helper.rs",
    ];
    let mut found = Vec::new();
    let mut scanned = 0usize;
    for entry in repo_walk(&root) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(&root).unwrap_or(entry.path());
        let rel = rel.to_string_lossy().to_string();
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        scanned += 1;
        if allowed.contains(&rel.as_str()) {
            continue;
        }
        for old in RETIRED {
            if text.contains(old) {
                found.push(format!("{rel}: {old}"));
            }
        }
    }
    assert!(
        scanned > 1000,
        "only {scanned} files read; the walk is looking at the wrong tree"
    );
    assert!(
        found.is_empty(),
        "retired golden variables are still written: {found:?}"
    );
}
