//! `yidam dispatch` end to end: registry first, the proposal route, and the exit code (#477).
//!
//! The library's own tests in `src/cmd/dispatch/tests.rs` hold the definition of done line by
//! line. This is what `report_goldens.rs`'s `NO_REPORT` entry points at: the report contract as
//! the binary emits it, against a repository built for it, because the shared fixture every
//! golden reads holds no seat and must not be handed a branch.
//!
//! The flow is the one a person follows. A seat whose row leaves `Config` blank is refused and
//! told the digest its declaration hashes to; the person records it on the seat's branch; the
//! next dispatch proposes, and the seat moves only when someone fast-forwards it.

mod common;

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use common::git::{git, git_at, out, FIXTURE_DATE};
use serde_json::Value;

const ELECTOR: &str =
    "#!/bin/sh\nset -eu\nmkdir -p \"$YIDAM_OUT/$(dirname \"$YIDAM_POSITION\")\"\n\
                       printf '# %s\\n\\nA position.\\n' \"$YIDAM_SEAT\" > \
                       \"$YIDAM_OUT/$YIDAM_POSITION\"\n";

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn registry(config: &str) -> String {
    let mut s = String::from(
        "# Electors\n\n| Name | Branch | Role | Kind | Model | Version | Config |\n\
         |------|--------|------|------|-------|---------|--------|\n",
    );
    for name in ["auditor", "advocate"] {
        let _ = writeln!(
            s,
            "| `{name}` | `ma/{name}` | A seat. | agent | `claude-opus-4-8` | `1` | `{config}` |"
        );
    }
    s
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    write(root, ".yidam/corpus/topic.ont.yml", "class: topic\n");
    write(root, "elector.sh", ELECTOR);
    write(root, ".yidam/sangha/electors.md", &registry(""));
    for name in ["auditor", "advocate"] {
        write(
            root,
            &format!(".yidam/sangha/dispatch/{name}.toml"),
            "model = \"claude-opus-4-8\"\nversion = \"1\"\nrun = [\"sh\", \"elector.sh\"]\n\
             reads = [\"elector.sh\", \".yidam/corpus/**\"]\nconfig = [\"elector.sh\"]\n",
        );
    }
    git(root, &["add", "-A"]);
    git_at(
        root,
        &["commit", "-q", "-m", "genesis: a sangha"],
        FIXTURE_DATE,
    );
    git(root, &["branch", "ma/auditor"]);
    git(root, &["branch", "ma/advocate"]);
    dir
}

fn yidam(root: &Path, args: &[&str]) -> (String, String, i32) {
    let o = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
        o.status.code().unwrap_or(-1),
    )
}

const ARGS: &[&str] = &[
    "dispatch", "budget", "--seat", "auditor", "--seat", "advocate", "--format", "json",
];

#[test]
fn a_blank_row_is_refused_then_recorded_then_proposed() {
    let dir = repo();
    let root = dir.path();

    let (stdout, stderr, code) = yidam(root, ARGS);
    assert_ne!(
        code, 0,
        "a refused seat fails the command:\n{stdout}{stderr}"
    );
    let v: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}\n{stdout}"));
    assert_eq!(v["kind"], "dispatch");
    assert_eq!(v["format_version"], "1");
    let seats = v["seats"].as_array().unwrap();
    assert_eq!(seats.len(), 2);
    for s in seats {
        assert_eq!(s["outcome"], "refused", "{s}");
        assert!(
            s["reason"].as_str().unwrap().contains("`Config` is blank"),
            "{s}"
        );
    }
    assert!(v["independence"].is_null());
    assert!(out(root, &["branch", "--list", "propose/*"]).is_empty());

    // Registry first: the person records the digest on each seat, and only then does it run.
    let digest = seats[0]["config"].as_str().unwrap().to_string();
    for name in ["auditor", "advocate"] {
        git(root, &["switch", "-q", &format!("ma/{name}")]);
        write(root, ".yidam/sangha/electors.md", &registry(&digest[..12]));
        git(root, &["add", "-A"]);
        git_at(
            root,
            &["commit", "-q", "-m", "update: record the occupant"],
            FIXTURE_DATE,
        );
    }
    git(root, &["switch", "-q", "main"]);
    let auditor_tip = out(root, &["rev-parse", "ma/auditor"]);

    let (stdout, stderr, code) = yidam(root, ARGS);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let v: Value = serde_json::from_str(&stdout).unwrap();
    for s in v["seats"].as_array().unwrap() {
        assert_eq!(s["outcome"], "proposed", "{s}");
        assert_eq!(s["verb"], "open");
        assert!(s["proposal"]
            .as_str()
            .unwrap()
            .starts_with("propose/elector/"));
    }
    assert_eq!(v["independence"]["value"], "shared-configuration");

    // Nothing moved but two proposal refs.
    assert_eq!(out(root, &["rev-parse", "ma/auditor"]), auditor_tip);
    assert_eq!(out(root, &["status", "--porcelain"]), "");
    assert_eq!(out(root, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");

    let (text, _, code) = yidam(
        root,
        &[
            "dispatch", "budget", "--seat", "auditor", "--seat", "advocate",
        ],
    );
    assert_eq!(code, 0);
    assert!(text.contains("unchanged"), "{text}");
    assert!(
        text.contains("independence: shared-configuration"),
        "{text}"
    );
}

#[test]
fn a_question_that_is_not_a_slug_is_an_error_before_any_seat_is_read() {
    let dir = repo();
    let (_, stderr, code) = yidam(
        dir.path(),
        &["dispatch", "Budget Premise", "--seat", "auditor"],
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("not a question slug"), "{stderr}");
}
