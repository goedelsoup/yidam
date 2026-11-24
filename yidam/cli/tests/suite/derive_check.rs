//! `yidam derive check`, end to end (RFC-0045).
//!
//! Its own file rather than a second arm in the golden matrix, for `kuten-check`'s reason: the
//! matrix's fixture is read by thirty goldens and three SDK runners, and an artifact directory
//! and a `[derive]` section in it would be a fact every one of them had to carry. The matrix pins
//! the arm every repository is in — nothing declared, nothing read, a pass — and this stages the
//! other two: an argument that holds, and one that routes around a declared refusal.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common;

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

// ── span records (RFC-0063 §3) ───────────────────────────────────────────────

const RECORDS: &str = "\
[derive]
paths = [\"dossier/**\"]

[[derive.records]]
paths = [\"packs/**\"]
records = [\"/r/*\"]
tier = \"0/tier\"
";

/// Two records: one the node's verified paragraph carries, and one declaring a standing its
/// paragraph does not.
const PACK: &str = r#"{"r": [
  {"node": "reach/tailwater", "span": "read weekly by the district", "tier": "verified"},
  {"node": "reach/tailwater", "span": "Discharge tracks the release schedule", "tier": "verified"}
]}"#;

fn stage_records(config: &str, pack: &str) -> tempfile::TempDir {
    let tmp = stage(
        &[("dossier/weekly.yml", HOLDS), ("packs/a.json", pack)],
        true,
    );
    std::fs::write(tmp.path().join(".yidam/config.toml"), config).unwrap();
    tmp
}

#[test]
fn span_records_are_citations_counted_and_named_by_pointer() {
    let tmp = stage_records(RECORDS, PACK);
    let text = run(tmp.path(), "text");
    assert_ne!(text.code, 0, "{}", text.stdout);
    assert!(
        text.stdout.contains("packs/a.json /r/1") && text.stdout.contains("derive-tag-drift"),
        "{}",
        text.stdout
    );
    assert!(text.stdout.contains("2 record(s)"), "{}", text.stdout);

    let doc = json(tmp.path());
    assert_eq!(doc["records"], 2);
    let ds = doc["derivations"].as_array().unwrap();
    assert_eq!(ds.len(), 3, "one artifact and two records");
    assert_eq!(ds[1]["record"], "/r/0");
    assert_eq!(ds[1]["tier"], "verified");
    assert_eq!(ds[1]["findings"], serde_json::json!([]));
    assert_eq!(ds[2]["tier"], "inference");
    assert_eq!(ds[2]["claim"], serde_json::Value::Null);
}

#[test]
fn a_pointer_matching_nothing_fails_rather_than_reading_nothing() {
    let tmp = stage_records(&RECORDS.replace("/r/*", "/renamed/*"), PACK);
    let r = run(tmp.path(), "json");
    assert_ne!(r.code, 0, "{}", r.stdout);
    let doc: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(doc["records"], 0);
    assert_eq!(
        doc["derivations"][1]["findings"][0]["check"],
        "derive-record-unmatched"
    );
}

#[test]
fn an_unknown_derive_key_is_refused() {
    let tmp = stage_records(
        "[derive]
paths = [\"dossier/**\"]
record = []\n",
        PACK,
    );
    let r = run(tmp.path(), "text");
    assert_ne!(r.code, 0, "{}", r.stdout);
}

#[test]
fn a_mapping_naming_both_sources_is_refused() {
    let both = RECORDS.replace(
        "paths = [\"packs/**\"]",
        "paths = [\"packs/**\"]\nexport = \"x\"",
    );
    let tmp = stage_records(&both, PACK);
    let r = run(tmp.path(), "text");
    assert_ne!(r.code, 0, "{}", r.stdout);
    // A refused mapping is a finding in the report, not an empty stdout.
    let r = run(tmp.path(), "json");
    assert_ne!(r.code, 0, "{}", r.stdout);
    let report: serde_json::Value = serde_json::from_str(&r.stdout).expect("a JSON report");
    let refused = report["derivations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["path"] == ".yidam/config.toml")
        .expect("the mapping's refusal is reported against the config");
    assert_eq!(refused["findings"][0]["check"], "derive-record-unmatched");
}

/// Globs that name no JSON file read nothing, and that is a failure, not a pass.
#[test]
fn a_mapping_whose_paths_match_no_file_fails() {
    let moved = RECORDS.replace("packs/**", "out/packs/**");
    let tmp = stage_records(&moved, PACK);
    let r = run(tmp.path(), "text");
    assert_ne!(r.code, 0, "{}", r.stdout);
    assert!(r.stdout.contains("match no .json file"), "{}", r.stdout);
    assert_eq!(json(tmp.path())["records"], 0);
}

/// A mapping with no pointers reads nothing either.
#[test]
fn a_mapping_with_no_records_fails() {
    let empty = RECORDS.replace("records = [\"/r/*\"]", "records = []");
    let r = run(stage_records(&empty, PACK).path(), "text");
    assert_ne!(r.code, 0, "{}", r.stdout);
    assert!(r.stdout.contains("no `records` pointers"), "{}", r.stdout);
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sdks/parity/fixtures/reports")
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
            ("packs/a.json", PACK),
        ],
        true,
    );
    std::fs::write(tmp.path().join(".yidam/config.toml"), RECORDS).unwrap();
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
        "records",
        "derivations[].record",
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
