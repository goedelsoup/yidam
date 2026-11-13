//! `cycle`'s tool acts, run as given (#1442).
//!
//! A `tool` act says its `argv` *is* the act. That is a claim a string comparison cannot hold:
//! `["yidam", "run"]` matching `["yidam", "run"]` passes whether or not running it does anything,
//! and the failure it would miss is the one a loop pays for — a command that exits nonzero, or
//! one that runs and leaves the act exactly where it was. So every tool act below is executed,
//! from the report's own `root` as the field's description says, and the one whose effect is
//! checkable here is checked: after `yidam run`, `cycle` no longer names it.
//!
//! **The worked example, not a fixture written here**, for `mcp_act_tier.rs`'s reason: it is a
//! corpus somebody would copy, with a manifest and a catalog entry. Two things are changed in
//! the copy and both are stated. The catalog entry is put past its TTL, as `mcp_act_tier.rs`
//! does, so the catalog clock is due and its remedy is a tool act. And the manifest's
//! `travel-tier-typed` table is cut: it needs `calculators-gluon`, outside the default set, and
//! the planner refuses a whole plan holding a step it cannot invoke — so in the build `cargo test`
//! compiles, the example has no runnable plan and `cycle` reports the step half unread.

use serde_json::Value;
use std::path::Path;
use std::process::Command;

use crate::common;

use common::git::git;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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

fn stage() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    copy_dir(&repo_root().join("examples/streamflow"), root);

    let manifest = root.join(".yidam/capabilities.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    let (kept, cut) = text
        .split_once("[capability.travel-tier-typed]")
        .expect("streamflow declares travel-tier-typed; if it moved, this cut must move with it");
    assert!(
        !cut.contains("[capability."),
        "travel-tier-typed is no longer the manifest's last table, so cutting to the end would \
         take another step with it"
    );
    std::fs::write(&manifest, kept).unwrap();

    let entry = root.join(".yidam/catalog/usgs-nwis.md");
    let text = std::fs::read_to_string(&entry).unwrap();
    let (_, rest) = text.split_once("---\n").unwrap();
    let (front, body) = rest.split_once("\n---").unwrap();
    let front: String = front
        .lines()
        .filter(|l| !l.starts_with("retrieved:") && !l.starts_with("ttl_days:"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        &entry,
        format!("---\n{front}\nretrieved: 2020-01-01\nttl_days: 1\n---{body}"),
    )
    .unwrap();

    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@t.co"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "genesis: streamflow"]);
    tmp
}

fn cycle(root: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["cycle", "--format", "json"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "cycle exits zero however much is owed\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Every act in `next` and every remedy under `owed`, as `(kind, argv)`.
fn acts(report: &Value) -> Vec<(String, Option<Vec<String>>, String)> {
    let next = report["next"].as_array().unwrap().iter();
    let owed = report["owed"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| !c["remedy"].is_null());
    next.chain(owed)
        .map(|a| {
            let kind = a["kind"]
                .as_str()
                .expect("every act carries a kind")
                .to_string();
            let argv = a.get("argv").map(|v| {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect()
            });
            let label = a
                .get("act")
                .or_else(|| a.get("remedy"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            (kind, argv, label)
        })
        .collect()
}

fn tool_acts(report: &Value) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = acts(report)
        .into_iter()
        .filter_map(|(kind, argv, _)| (kind == "tool").then(|| argv.unwrap()))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Run `argv` from the report's `root`, with `yidam` meaning the binary under test.
fn execute(report: &Value, argv: &[String]) -> std::process::Output {
    assert_eq!(
        argv[0], "yidam",
        "every tool act this binary emits starts `yidam`: {argv:?}"
    );
    let root = report["root"].as_str().expect("the envelope carries root");
    Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(&argv[1..])
        .current_dir(root)
        .output()
        .unwrap()
}

/// The staged example emits both tool acts this build can reach, and both run.
///
/// Asserted as a set first, because a test that executes "every tool act" over a report that
/// happened to carry none would pass having executed nothing.
#[test]
fn every_tool_act_cycle_names_runs_as_given() {
    let tmp = stage();
    let report = cycle(tmp.path());
    assert!(
        report["stale_steps_unread"].is_null(),
        "the staged manifest should plan: {}",
        report["stale_steps_unread"]
    );

    let tools = tool_acts(&report);
    assert_eq!(
        tools,
        [vec!["yidam", "propose"], vec!["yidam", "run"]],
        "the staged example should owe a catalog proposal and a run; acts were {:#?}",
        acts(&report)
    );
    for argv in &tools {
        let out = execute(&report, argv);
        assert!(
            out.status.success(),
            "`{}` was named as a tool act and failed:\n{}",
            argv.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Running `yidam run` is the act, so after it `cycle` no longer names it.
///
/// The other half of "argv is the act": a command that exits zero and leaves the corpus owing
/// exactly what it owed would pass the test above.
#[test]
fn the_run_act_is_discharged_by_running_it() {
    let tmp = stage();
    let report = cycle(tmp.path());
    let run = vec!["yidam".to_string(), "run".to_string()];
    assert!(tool_acts(&report).contains(&run), "{:#?}", acts(&report));

    assert!(execute(&report, &run).status.success());

    let after = cycle(tmp.path());
    assert!(
        !tool_acts(&after).contains(&run),
        "`yidam run` ran and cycle still names it: {:#?}",
        after["stale_steps"]
    );
    assert_eq!(after["stale_steps"], serde_json::json!([]));
}

/// A person act never carries a command, and every act carries a kind.
#[test]
fn a_person_act_carries_no_argv() {
    let tmp = stage();
    let report = cycle(tmp.path());
    let all = acts(&report);
    assert!(
        all.iter().any(|(k, _, _)| k == "person"),
        "the staged example should owe at least one person act, or this asserts nothing: {all:#?}"
    );
    for (kind, argv, label) in &all {
        match kind.as_str() {
            "tool" => assert!(argv.is_some(), "a tool act with no argv: {label}"),
            "person" => assert!(argv.is_none(), "a person act with argv: {label}"),
            other => panic!("`{other}` is not an act kind"),
        }
    }
}
