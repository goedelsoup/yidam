//! `yidam cohort` — the upstream evidence loop, held to two things it claims.
//!
//! The command reads a set of derived repositories and reports what they say about the
//! prelude. Two of its claims are worth a test and the rest is arithmetic:
//!
//! - **Every norm it scores is a rule the prelude actually states.** A list of rules in a
//!   Rust file is a second copy of the prelude, and a second copy drifts. Each row carries
//!   the sentence it is derived from, and [`every_norm_quotes_the_document_it_names`] holds
//!   the quote to the file. A norm reworded upstream fails here rather than quietly measuring
//!   a rule nobody states — which is the failure the first draft of `cohort.rs` actually
//!   made: it scored a rule GRAPH.md argues *against*, two sections below the one it cited.
//!
//! - **It is read-only against live repositories.** That is the property that made the
//!   hand-run safe, and the one most easily lost. Asserted by running it against a dirty
//!   tree and diffing, never by inspection.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::Example;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every run of whitespace as one space.
///
/// Prelude documents are prose and get rewrapped. A quote matched line by line would fail on
/// a reflow that changed nothing — the exact trap `kuten::Vintage::read` records, where a
/// sentence match read `false` against this template's own prelude and all eighteen derived
/// corpora because the paragraph had been rewrapped.
fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The claim that keeps the norm list from becoming a second copy of the prelude.
#[test]
fn every_norm_quotes_the_document_it_names() {
    let prelude = repo_root().join("yidam/prelude");
    assert!(
        !yidam::PRELUDE_NORMS.is_empty(),
        "no norms are declared, so this test asserts nothing"
    );
    for n in yidam::PRELUDE_NORMS {
        let path = prelude.join(n.document);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "`{}` names {} and it is unreadable: {e}",
                n.id,
                path.display()
            )
        });
        assert!(
            collapsed(&text).contains(&collapsed(n.statement)),
            "`{}` quotes a sentence that is not in {}:\n  {}\n\nThe rule moved or was \
             reworded. Re-read the document and either re-quote it or drop the norm — a \
             check scoring a rule its own source no longer states is worse than no check.",
            n.id,
            n.document,
            n.statement
        );
    }
}

/// Discovery's failure one level up: a roster that matches nothing passes everything.
#[test]
fn every_norm_is_scored_by_the_reading() {
    let e = Example::materialize("streamflow");
    let (out, err, code) = e.run(&[
        "cohort",
        &e.path().display().to_string(),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();

    let scored: Vec<String> = v["members"][0]["readings"]
        .as_array()
        .expect("readings")
        .iter()
        .map(|r| r["norm"].as_str().unwrap_or_default().to_string())
        .collect();
    for n in yidam::PRELUDE_NORMS {
        assert!(
            scored.contains(&n.id.to_string()),
            "`{}` is declared and nothing reads it — the roster and the reading disagree",
            n.id
        );
    }
    let declared: Vec<&str> = yidam::PRELUDE_NORMS.iter().map(|n| n.id).collect();
    for s in &scored {
        assert!(
            declared.contains(&s.as_str()),
            "the reading emits `{s}`, which no norm declares"
        );
    }
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != std::ffi::OsStr::new(".git") || e.depth() == 0)
    {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            out.insert(rel, std::fs::read(entry.path()).unwrap());
        }
    }
    out
}

/// The property that made the hand-run safe against repositories somebody was working in.
#[test]
fn reading_a_cohort_touches_no_working_tree() {
    let subject = Example::materialize("streamflow");
    let root = subject.path();
    std::fs::write(
        root.join("README.md"),
        "an edit nobody asked this command to keep\n",
    )
    .unwrap();
    std::fs::write(root.join("scratch.txt"), "untracked\n").unwrap();

    let before = tree(&root);
    let head_before = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();

    // Run from a *different* repository, which is how a cohort is read: the subject is named
    // on the command line and is never the working directory.
    let reader = Example::materialize("journalism");
    let (out, err, code) = reader.run(&["cohort", &root.display().to_string()]);
    assert_eq!(code, 0, "{out}{err}");

    assert_eq!(
        tree(&root),
        before,
        "`yidam cohort` changed the tree it read"
    );
    assert_eq!(
        Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
        head_before.stdout,
        "`yidam cohort` moved the ref of the repository it read"
    );
}

/// Absence is a result. A path that is not a corpus is reported, never dropped.
#[test]
fn a_path_that_is_not_a_derived_repository_is_reported_rather_than_skipped_in_silence() {
    let e = Example::materialize("streamflow");
    let nowhere = e.path().join("not-a-corpus-at-all");
    std::fs::create_dir_all(&nowhere).unwrap();

    let (out, err, code) = e.run(&[
        "cohort",
        &e.path().display().to_string(),
        "/definitely/not/here",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["read"], 1);
    assert_eq!(
        v["skipped"].as_array().map(Vec::len),
        Some(1),
        "a path that read nothing is missing from the report: {out}"
    );
}

/// Lettered, not named — the disclosure rule, asserted rather than documented.
#[test]
fn a_repository_is_not_named_unless_the_caller_asks() {
    let e = Example::materialize("streamflow");
    let path = e.path().display().to_string();

    let (quiet, _, _) = e.run(&["cohort", &path]);
    assert!(
        !quiet.contains(&path),
        "the default report names the repository it read:\n{quiet}"
    );
    assert!(quiet.contains("  A "), "no member was lettered:\n{quiet}");

    let (named, _, _) = e.run(&["cohort", &path, "--paths"]);
    assert!(named.contains(&path), "--paths did not name it:\n{named}");
}

/// A cohort of one is not evidence about a prelude, and the report must not read as if it is.
#[test]
fn one_repository_produces_no_finding_about_the_prelude() {
    let e = Example::materialize("streamflow");
    let (out, _, code) = e.run(&["cohort", &e.path().display().to_string()]);
    assert_eq!(code, 0);
    assert!(
        !out.contains("What this says about the prelude"),
        "a single repository was generalised into a finding about the prelude:\n{out}"
    );
}

/// The envelope every other report carries.
#[test]
fn the_json_report_carries_the_contract_envelope() {
    let e = Example::materialize("streamflow");
    let (out, err, code) = e.run(&[
        "cohort",
        &e.path().display().to_string(),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["format_version"], "1");
    assert!(v["yidam"]["version"].is_string());
    assert!(v["norms"].as_array().is_some_and(|n| !n.is_empty()));
}
