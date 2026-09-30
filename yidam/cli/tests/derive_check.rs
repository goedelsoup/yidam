//! `yidam derive check`, end to end (RFC-0045).
//!
//! Its own file rather than a second arm in the golden matrix, for `kuten-check`'s reason: the
//! matrix's fixture is read by thirty goldens and three SDK runners, and an artifact directory
//! and a `[derive]` section in it would be a fact every one of them had to carry. The matrix pins
//! the arm every repository is in — nothing declared, nothing read, a pass — and this stages the
//! other two: an argument that holds, and one that routes around a declared refusal.

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

const NODE: &str = "\
class: reach
label: Tailwater
description: |
  The gauge sits at the riffle and is read weekly by the district [verified].

  Discharge tracks the release schedule [inference]. The record does not say
  the dam caused the 2019 avulsion.
refuses:
  - span: The record does not say the dam caused the 2019 avulsion.
    inference: the dam caused the 2019 avulsion
";

/// Rests on the verified paragraph only, at a reach it admits.
const HOLDS: &str = "\
claim: The gauge is read weekly.
reach: public
cites:
  - node: reach/tailwater
    span: read weekly by the district
";

/// Rests on the refusing paragraph, answers nothing, and claims more reach than it has.
const ROUTES_AROUND: &str = "\
---
claim: The dam's schedule drove the avulsion.
reach: public
cites:
  - node: reach/tailwater
    span: Discharge tracks the release schedule
---

# Memo
";

fn stage(artifacts: &[(&str, &str)], derive: bool) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let write = |rel: &str, text: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    write(
        ".yidam/corpus/reach.ont.yml",
        "class: reach\nlabel: Reach\ndescription: A stretch of river.\n",
    );
    write(".yidam/corpus/reach/tailwater.yml", NODE);
    if derive {
        write(".yidam/config.toml", "[derive]\npaths = [\"dossier/**\"]\n");
    }
    write(
        "dossier/README.md",
        "# Dossier\n\nNo frontmatter, so not an artifact.\n",
    );
    for (rel, text) in artifacts {
        write(rel, text);
    }
    let git = |args: &[&str]| common::git::git_at(root, args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: derive fixture"]);
    tmp
}

struct Run {
    stdout: String,
    code: i32,
}

fn run(root: &Path, format: &str) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(["derive", "check", "--format", format])
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn json(root: &Path) -> serde_json::Value {
    serde_json::from_str(&run(root, "json").stdout).unwrap()
}

#[test]
fn no_declared_paths_reads_nothing_and_passes() {
    let tmp = stage(&[("dossier/memo.md", ROUTES_AROUND)], false);
    let r = run(tmp.path(), "text");
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(r.stdout.contains("no `[derive] paths`"), "{}", r.stdout);
    assert_eq!(json(tmp.path())["derivations"], serde_json::json!([]));
}

#[test]
fn an_argument_that_holds_passes() {
    let tmp = stage(&[("dossier/weekly.yml", HOLDS)], true);
    let r = run(tmp.path(), "text");
    assert_eq!(r.code, 0, "{}", r.stdout);
    let doc = json(tmp.path());
    let d = &doc["derivations"][0];
    assert_eq!(d["path"], "dossier/weekly.yml");
    assert_eq!(d["tier"], "verified");
    assert_eq!(d["findings"], serde_json::json!([]));
    assert_eq!(
        doc["derivations"].as_array().unwrap().len(),
        1,
        "the README is not an artifact"
    );
}

#[test]
fn routing_around_a_declared_refusal_fails_in_both_formats() {
    let tmp = stage(
        &[
            ("dossier/weekly.yml", HOLDS),
            ("dossier/memo.md", ROUTES_AROUND),
        ],
        true,
    );
    let text = run(tmp.path(), "text");
    let json_run = run(tmp.path(), "json");
    assert_ne!(text.code, 0, "{}", text.stdout);
    assert_eq!(
        text.code, json_run.code,
        "the verdict is the same in both formats"
    );
    assert!(
        text.stdout.contains("derive-unanswered-refusal"),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout.contains("derive-beyond-reach"),
        "{}",
        text.stdout
    );

    let doc: serde_json::Value = serde_json::from_str(&json_run.stdout).unwrap();
    let memo = &doc["derivations"][0];
    assert_eq!(memo["path"], "dossier/memo.md");
    assert_eq!(memo["tier"], "inference");
    let checks: Vec<&str> = memo["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["check"].as_str().unwrap())
        .collect();
    assert_eq!(
        checks,
        ["derive-unanswered-refusal", "derive-beyond-reach"],
        "{memo}"
    );
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports")
}

/// Every path the report emits is declared in the committed schema, and the populated arm is
/// what emits them. `report_goldens.rs`'s `UNREACHED` points here for `derivations`.
#[test]
fn every_emitted_field_is_declared_in_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().join("report.schema.json")).unwrap(),
    )
    .unwrap();
    let tmp = stage(
        &[
            ("dossier/weekly.yml", HOLDS),
            ("dossier/memo.md", ROUTES_AROUND),
        ],
        true,
    );
    let doc = json(tmp.path());
    let mut emitted = std::collections::BTreeSet::new();
    paths_of(&doc, "", &mut emitted);
    for path in &emitted {
        assert!(
            declares(&schema, path),
            "`derive check` emits `{path}`, which report.schema.json does not declare"
        );
    }
    for absent in ["nonesuch", "derivations[].nonesuch"] {
        assert!(!declares(&schema, absent), "the walk declares `{absent}`");
    }
    for witness in [
        "derive_paths",
        "derivations[].tier",
        "derivations[].spans[].standing",
        "derivations[].findings[].node",
        "derivations[].findings[].severity",
    ] {
        assert!(
            emitted.contains(witness),
            "never emitted `{witness}`: {emitted:?}"
        );
    }
}

/// `regen_check.rs`'s walk: every path a document carries a value at.
fn paths_of(node: &serde_json::Value, path: &str, out: &mut std::collections::BTreeSet<String>) {
    match node {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let here = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                out.insert(here.clone());
                paths_of(child, &here, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                paths_of(item, &format!("{path}[]"), out);
            }
        }
        _ => {}
    }
}

/// `regen_check.rs`'s schema lookup, in the same notation.
fn declares(schema: &serde_json::Value, path: &str) -> bool {
    let mut node = schema;
    for segment in path.split('.') {
        let (key, arrays) = match segment.split_once("[]") {
            Some((key, rest)) => (key, rest.matches("[]").count() + 1),
            None => (segment, 0),
        };
        node = match node.get("properties").and_then(|p| p.get(key)) {
            Some(child) => child,
            None => return false,
        };
        for _ in 0..arrays {
            node = match node.get("items") {
                Some(items) => items,
                None => return false,
            };
        }
    }
    true
}
