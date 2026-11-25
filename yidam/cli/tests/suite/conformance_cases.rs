//! Each conformance case must produce the result its `expected.json` records (RFC-0068 §3).
//!
//! `yidam/sdks/conformance/cases/` is what a producer that never builds yidam runs its own
//! validator over. So a case is only worth shipping if the release it ships with really
//! produces what it says, and this is where that is held: each case is staged per
//! `conformance/stage.toml`, the real binary runs `graph-check` and `lint` over it, and the
//! two reports are projected onto the case's shape and compared.
//!
//! **What is fixed is which checks fire, on which node, at which severity.** A message, a
//! rationale and a `detail` are free to change, so the projection drops them. `graph-check`
//! has no per-issue id, so it is matched by file: whether it passed, and which nodes and
//! which class files had issues.
//!
//! `UPDATE_GOLDENS=1` rewrites every `expected.json`. The recipe is read from the committed
//! `stage.toml` rather than restated here, because it ships in the asset and a consumer's
//! runner stages from the same file.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::common;

fn conformance() -> PathBuf {
    common::repo_root().join("yidam/sdks/conformance")
}

/// Every case directory, by name.
fn cases() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(conformance().join("cases"))
        .expect("conformance/cases")
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
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

/// Copy `cases/<case>/repo/` into a temporary directory and commit it per `stage.toml`.
fn stage(case: &str) -> tempfile::TempDir {
    let recipe: toml::Value = toml::from_str(
        &std::fs::read_to_string(conformance().join("stage.toml")).expect("stage.toml"),
    )
    .expect("stage.toml parses");
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    copy_dir(&conformance().join("cases").join(case).join("repo"), root);

    let branch = recipe["branch"].as_str().expect("branch");
    let date = recipe["date"].as_str().expect("date");
    let name = recipe["author"]["name"].as_str().expect("author.name");
    let email = recipe["author"]["email"].as_str().expect("author.email");
    common::git::git(root, &["init", "-q", "-b", branch]);
    common::git::git(root, &["config", "user.name", name]);
    common::git::git(root, &["config", "user.email", email]);
    for commit in recipe["commits"].as_array().expect("commits") {
        let message = commit["message"].as_str().expect("commits.message");
        common::git::git(root, &["add", "-A"]);
        common::git::git_at(
            root,
            &["commit", "-q", "--allow-empty", "-m", message],
            date,
        );
    }
    tmp
}

fn report(root: &Path, command: &str) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args([command, "--format", "json"])
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "`yidam {command} --format json` printed no JSON ({e}).\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// The paths a `graph-check` issue list names, sorted.
fn paths(rows: &Value) -> Vec<Value> {
    let mut out: Vec<String> = rows
        .as_array()
        .expect("an issue list")
        .iter()
        .map(|r| r["node"].as_str().expect("node").to_string())
        .collect();
    out.sort();
    out.into_iter().map(Value::from).collect()
}

/// The two reports, projected onto the case's shape.
fn observe(root: &Path) -> Value {
    let gc = report(root, "graph-check");
    let lint = report(root, "lint");
    let mut findings: Vec<(String, String, String)> = Vec::new();
    for check in lint["checks"].as_array().expect("lint checks") {
        let id = check["id"].as_str().expect("check id");
        for v in check["violations"].as_array().into_iter().flatten() {
            findings.push((
                id.to_string(),
                v["severity"].as_str().expect("severity").to_string(),
                v["node"].as_str().expect("node").to_string(),
            ));
        }
    }
    findings.sort();
    json!({
        "graph_check": {
            "passed": gc["passed"],
            "nodes": paths(&gc["nodes_with_issues"]),
            "classes": paths(&gc["classes_with_issues"]),
        },
        "lint": findings
            .into_iter()
            .map(|(check, severity, node)| json!({ "check": check, "severity": severity, "node": node }))
            .collect::<Vec<_>>(),
    })
}

#[test]
fn every_case_produces_what_it_records() {
    let names = cases();
    assert!(!names.is_empty(), "conformance/cases holds no case");
    let mut drifted = Vec::new();
    for case in &names {
        let staged = stage(case);
        let actual = format!(
            "{}\n",
            serde_json::to_string_pretty(&observe(staged.path())).unwrap()
        );
        let path = conformance().join("cases").join(case).join("expected.json");
        if std::env::var("UPDATE_GOLDENS").is_ok() {
            std::fs::write(&path, &actual).unwrap();
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(expected) if expected == actual => {}
            Ok(expected) => drifted.push(format!(
                "{case}:\n--- expected\n{expected}--- actual\n{actual}"
            )),
            Err(_) => drifted.push(format!("{case}: no expected.json")),
        }
    }
    assert!(
        drifted.is_empty(),
        "\nconformance cases drifted from what the binary produces. If the change is intended, \
         re-run with UPDATE_GOLDENS=1 and review the diff.\n\n{}",
        drifted.join("\n")
    );
}

/// What each case is for, from RFC-0068's table, held against the recorded result.
///
/// `expected.json` is written by the gate, so on its own it would accept whatever the binary
/// did. These are the properties that made each case worth adding; a regeneration that loses
/// one is a regression, not a golden update.
#[test]
fn each_case_still_shows_what_it_was_added_for() {
    let read = |case: &str| -> Value {
        let path = conformance().join("cases").join(case).join("expected.json");
        serde_json::from_str(&std::fs::read_to_string(&path).expect("expected.json"))
            .expect("expected.json parses")
    };
    const TRAVERSAL: &str = ".yidam/corpus/concept/traversal.yml";
    let fired = |v: &Value, check: &str| {
        v["lint"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["check"] == check && f["node"] == TRAVERSAL)
    };
    // The same, at a stated severity: a regeneration that demotes the finding loses the case.
    let fired_at = |v: &Value, check: &str, severity: &str| {
        v["lint"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["check"] == check && f["node"] == TRAVERSAL && f["severity"] == severity)
    };
    let errors = |v: &Value| {
        v["lint"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["severity"] == "error")
            .count()
    };
    let fails_on_traversal = |v: &Value| {
        v["graph_check"]["passed"] == false && v["graph_check"]["nodes"] == json!([TRAVERSAL])
    };

    let minimal = read("minimal");
    assert_eq!(minimal["graph_check"]["passed"], true, "minimal must pass");
    assert_eq!(errors(&minimal), 0, "minimal must carry no lint error");

    let orphan = read("orphan");
    assert!(
        fails_on_traversal(&orphan) && fired_at(&orphan, "orphan-out", "error"),
        "orphan must fail on {TRAVERSAL} with an `orphan-out` error"
    );

    let optional = read("optional-outgoing");
    assert!(
        fails_on_traversal(&optional) && !fired(&optional, "orphan-out"),
        "graph-check does not read `outgoing: optional` and lint does"
    );

    // Each fails graph-check on the node, and lint names the same defect by its own id.
    for (case, check, severity) in [
        ("broken-link", "dangling-edge", "error"),
        ("missing-label", "missing-label", "warn"),
        ("unknown-class", "unknown-class", "error"),
    ] {
        let v = read(case);
        assert!(
            fails_on_traversal(&v),
            "{case} must fail graph-check on {TRAVERSAL}"
        );
        assert!(
            fired_at(&v, check, severity),
            "{case} must carry a `{check}` {severity} on {TRAVERSAL}"
        );
    }

    let required = read("missing-required-property");
    assert!(
        fired_at(&required, "missing-property", "error"),
        "an omitted `required: true` property is a lint error, not a warning"
    );

    let claims = read("claim-standings");
    assert_eq!(
        claims["graph_check"]["passed"], true,
        "claim-standings must pass graph-check"
    );
    assert!(
        !claims["lint"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["check"].as_str().unwrap().starts_with("claim-")),
        "bare, bracketed and qualified standings are all readable"
    );
}

/// The shape a consumer's runner reads at each `conformance/VERSION`: every key path in
/// `expected.json` and in `stage.toml`, with `[]` for an array's items.
///
/// `VERSION` is a number a consumer pins, so it has to move when this shape does. Changing the
/// projection in [`observe`] or a key in `stage.toml` fails here until `VERSION` is bumped and
/// the new shape is recorded beside it. A message, a case or a finding changes no key path, so
/// they need no bump.
const SHAPE: (&str, &[&str]) = (
    "1",
    &[
        "expected.json:graph_check",
        "expected.json:graph_check.classes",
        "expected.json:graph_check.nodes",
        "expected.json:graph_check.nodes[]",
        "expected.json:graph_check.passed",
        "expected.json:lint",
        "expected.json:lint[]",
        "expected.json:lint[].check",
        "expected.json:lint[].node",
        "expected.json:lint[].severity",
        "stage.toml:author",
        "stage.toml:author.email",
        "stage.toml:author.name",
        "stage.toml:branch",
        "stage.toml:commits",
        "stage.toml:commits[]",
        "stage.toml:commits[].message",
        "stage.toml:date",
    ],
);

fn key_paths(prefix: &str, v: &Value, out: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                let path = if prefix.ends_with(':') {
                    format!("{prefix}{k}")
                } else {
                    format!("{prefix}.{k}")
                };
                out.insert(path.clone());
                key_paths(&path, child, out);
            }
        }
        Value::Array(items) => {
            let path = format!("{prefix}[]");
            for item in items {
                out.insert(path.clone());
                key_paths(&path, item, out);
            }
        }
        _ => {}
    }
}

#[test]
fn the_conformance_version_names_the_shape_it_ships() {
    let mut shape = std::collections::BTreeSet::new();
    // An empty list contributes no `[]` path, so the union over cases is the shape: every
    // case is the same projection, and one with findings is enough to name the item keys.
    for case in cases() {
        let path = conformance()
            .join("cases")
            .join(&case)
            .join("expected.json");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        key_paths("expected.json:", &v, &mut shape);
    }
    let recipe: toml::Value =
        toml::from_str(&std::fs::read_to_string(conformance().join("stage.toml")).unwrap())
            .expect("stage.toml parses");
    let recipe = serde_json::to_value(recipe).expect("stage.toml as JSON");
    key_paths("stage.toml:", &recipe, &mut shape);

    let recorded: std::collections::BTreeSet<String> =
        SHAPE.1.iter().map(|s| s.to_string()).collect();
    let added: Vec<_> = shape.difference(&recorded).collect();
    let dropped: Vec<_> = recorded.difference(&shape).collect();
    assert!(
        added.is_empty() && dropped.is_empty(),
        "\nthe shape a consumer reads changed (added {added:?}, dropped {dropped:?}). Bump \
         yidam/sdks/conformance/VERSION and record the new shape and version in `SHAPE`.\n"
    );

    let version = std::fs::read_to_string(conformance().join("VERSION")).expect("VERSION");
    assert_eq!(
        version.trim(),
        SHAPE.0,
        "conformance/VERSION says {}, and `SHAPE` records the shape of {}. Record the shape \
         beside the version that ships it.",
        version.trim(),
        SHAPE.0
    );
}
