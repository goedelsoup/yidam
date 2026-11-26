//! RFC-0066 end to end: a declined `doctor` offer, and a lint baseline whose `expire_after` is
//! argued — or not — by the decision record it names. Also a set `[lint] escalate_after`,
//! which `lint` names (#1584). Each emits fields the basic fixture's goldens cannot reach
//! without moving every runner graded against them, so this file stages each arm and holds
//! every emitted path to the committed report schema.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common;

fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sdks/parity/fixtures/reports/basic/repo");
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
    let git = |args: &[&str]| common::git::git_at(tmp.path(), args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: upgrade offers fixture"]);
    tmp
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn yidam(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn json(root: &Path, args: &[&str]) -> serde_json::Value {
    let out = yidam(root, &[args, &["--format", "json"]].concat());
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout))
        .unwrap_or_else(|e| panic!("`yidam {}` did not emit JSON: {e}", args.join(" ")))
}

fn schema() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../sdks/parity/fixtures/reports/report.schema.json"),
        )
        .unwrap(),
    )
    .unwrap()
}

/// Whether `schema` declares `path`, descending every branch of a `oneOf`/`anyOf`.
///
/// `common::declares` walks `properties` alone, and `checks[]` is a `oneOf` of a lint check
/// and a doctor check, so it could not see a doctor field at all.
fn declares(node: &serde_json::Value, path: &str) -> bool {
    let (segment, rest) = path.split_once('.').unwrap_or((path, ""));
    let (key, arrays) = match segment.split_once("[]") {
        Some((key, tail)) => (key, tail.matches("[]").count() + 1),
        None => (segment, 0),
    };
    branches(node).into_iter().any(|n| {
        let Some(mut child) = n.get("properties").and_then(|p| p.get(key)) else {
            return false;
        };
        for _ in 0..arrays {
            match child.get("items") {
                Some(items) => child = items,
                None => return false,
            }
        }
        rest.is_empty() || declares(child, rest)
    })
}

fn branches(node: &serde_json::Value) -> Vec<&serde_json::Value> {
    let mut out = vec![node];
    for k in ["oneOf", "anyOf", "allOf"] {
        if let Some(arms) = node.get(k).and_then(|v| v.as_array()) {
            out.extend(arms.iter().flat_map(branches));
        }
    }
    out
}

/// Every path `doc` emits under `scope` is declared, and each witness is among them.
///
/// Scoped because the walk above follows no `$ref`, and the lint checks reach a `span` by one;
/// what this change emits sits under `gate` and under a doctor check, which need none.
fn declared(doc: &serde_json::Value, command: &str, scope: &str, witnesses: &[&str]) {
    let schema = schema();
    let mut emitted = std::collections::BTreeSet::new();
    common::paths_of(doc, "", &mut emitted);
    for path in emitted.iter().filter(|p| p.starts_with(scope)) {
        assert!(
            declares(&schema, path),
            "`{command}` emits `{path}`, which report.schema.json does not declare"
        );
    }
    for witness in witnesses {
        assert!(
            emitted.contains(*witness),
            "never emitted `{witness}`: {emitted:?}"
        );
    }
}

/// A decline against a record the repository holds reads `declined`, names the record, and
/// is counted at the top level — and `--strict` does not fail on it.
#[test]
fn a_declined_offer_reads_declined_and_names_its_record() {
    let tmp = stage();
    let root = tmp.path();
    write(root, "AGENTS.md", "# Agents\n");
    write(
        root,
        ".yidam/config.toml",
        "[doctor.declined]\nroutes = \"keep-our-agents\"\n",
    );
    write(
        root,
        ".yidam/decisions/keep-our-agents.yml",
        "summary: This repository writes its own AGENTS.md.\nrationale: It routes by hand.\n",
    );

    let doc = json(root, &["doctor"]);
    let routes = doc["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "routes")
        .unwrap_or_else(|| panic!("no routes check: {doc:#}"));
    assert_eq!(routes["verdict"], "declined", "{routes:#}");
    assert_eq!(
        routes["offers"][0]["record"], ".yidam/decisions/keep-our-agents.yml",
        "{routes:#}"
    );
    assert!(doc["declined"].as_u64().unwrap() >= 1, "{doc:#}");
    assert_eq!(doc["unknown_declines"], serde_json::json!([]), "{doc:#}");
    declared(
        &doc,
        "doctor",
        "",
        &["checks[].offers[].record", "declined"],
    );
}

/// A key naming no offer is reported, and is the one thing about declines `--strict` fails on.
#[test]
fn a_decline_naming_no_offer_is_listed() {
    let tmp = stage();
    write(
        tmp.path(),
        ".yidam/config.toml",
        "[doctor.declined]\nroute = \"keep-our-agents\"\n",
    );
    let doc = json(tmp.path(), &["doctor"]);
    assert_eq!(
        doc["unknown_declines"],
        serde_json::json!(["route"]),
        "{doc:#}"
    );
}

fn expiring(root: &Path, record: Option<&str>) {
    write(
        root,
        ".yidam/lint-baseline.yml",
        "expire_after: 2\nexpire_record: baseline-expiry\nviolations: {}\n",
    );
    if let Some(body) = record {
        write(
            root,
            ".yidam/decisions/baseline-expiry.yml",
            &format!("summary: Two commits.\nrationale: The import.\n{body}"),
        );
    }
}

/// The record states the baseline's number: the gate reports it argued, by name.
#[test]
fn an_expiry_its_record_states_is_argued() {
    let tmp = stage();
    expiring(tmp.path(), Some("expire_after: 2\n"));
    let doc = json(tmp.path(), &["lint"]);
    assert_eq!(
        doc["gate"]["expiry_record"],
        serde_json::json!({
            "state": "argued",
            "record": "baseline-expiry",
            "expire_after": 2,
            "record_states": 2
        }),
        "{:#}",
        doc["gate"]
    );
    declared(
        &doc,
        "lint",
        "gate",
        &[
            "gate.expiry_record.record",
            "gate.expiry_record.record_states",
        ],
    );
}

/// A raise made in the baseline alone disagrees with the record, and fails naming both numbers.
#[test]
fn a_raise_the_record_does_not_state_fails_and_names_both_numbers() {
    let tmp = stage();
    expiring(tmp.path(), Some("expire_after: 1\n"));
    let doc = json(tmp.path(), &["lint"]);
    assert_eq!(doc["gate"]["expiry_record"]["state"], "disagrees");
    assert_eq!(doc["gate"]["passed"], false);

    let out = yidam(tmp.path(), &["lint"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("expire_after is 2 and baseline-expiry states 1"),
        "{stderr}"
    );
}

/// A record that is not held fails the ratchet and names it.
#[test]
fn an_expire_record_that_is_not_held_fails_and_names_it() {
    let tmp = stage();
    expiring(tmp.path(), None);
    let doc = json(tmp.path(), &["lint"]);
    assert_eq!(
        doc["gate"]["expiry_record"],
        serde_json::json!({
            "state": "not_held",
            "record": "baseline-expiry",
            "expire_after": 2
        })
    );
    let stderr = String::from_utf8_lossy(&yidam(tmp.path(), &["lint"]).stderr).to_string();
    assert!(
        stderr.contains("expire_record names baseline-expiry"),
        "{stderr}"
    );
}

/// A set `escalate_after` opens the text report and sits in the JSON gate, so a failed build
/// names the number that failed it without anyone reading `config.toml` (#1584).
#[test]
fn a_set_escalate_after_is_named_by_the_report_and_the_gate() {
    let tmp = stage();
    write(
        tmp.path(),
        ".yidam/config.toml",
        "[lint]\nescalate_after = 7\n",
    );

    let doc = json(tmp.path(), &["lint"]);
    assert_eq!(doc["gate"]["escalate_after"], 7, "{:#}", doc["gate"]);
    declared(&doc, "lint", "gate", &["gate.escalate_after"]);

    let out = yidam(tmp.path(), &["lint"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first = stdout.lines().next().unwrap_or_default();
    assert!(
        first.starts_with("lint: `[lint] escalate_after = 7` in .yidam/config.toml"),
        "the report does not open with the threshold: {stdout}"
    );
}

/// Unset, neither surface says anything: absent is the state where nothing escalates, and a
/// line saying so on every run would be noise.
#[test]
fn an_unset_escalate_after_is_named_nowhere() {
    let tmp = stage();
    let doc = json(tmp.path(), &["lint"]);
    assert!(
        doc["gate"].get("escalate_after").is_none(),
        "{:#}",
        doc["gate"]
    );
    let stdout = String::from_utf8_lossy(&yidam(tmp.path(), &["lint"]).stdout).to_string();
    assert!(!stdout.contains("escalate_after"), "{stdout}");
}

/// A malformed config is refused before any check runs, so no report names a number from it.
///
/// The lenient read that hands the checks no threshold is for in-process callers such as
/// `serve --lsp`, and the unit test `the_opening_line_names_only_a_threshold_that_can_act`
/// holds it. From the command line the parse error is the answer (#1584).
#[test]
fn a_malformed_config_names_no_escalate_after() {
    let tmp = stage();
    write(
        tmp.path(),
        ".yidam/config.toml",
        "[lint\nescalate_after = 7\n",
    );

    for args in [&["lint"][..], &["lint", "--format", "json"]] {
        let out = yidam(tmp.path(), args);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "`yidam {}` accepted it",
            args.join(" ")
        );
        assert!(stderr.contains("config.toml"), "{stderr}");
        assert!(!stdout.contains("escalate_after"), "{stdout}");
    }
}
