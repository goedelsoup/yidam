//! `yidam gather` end to end: the envelope, the peers, the branch, and a repeat that writes
//! nothing (#476).
//!
//! The library's own tests in `src/cmd/gather/tests.rs` hold the definition of done line by
//! line. This is what `report_goldens.rs`'s `NO_REPORT` entry points at: the report contract as
//! the binary emits it, against a repository built for it, because the shared fixture every
//! golden reads holds no peer and must not be handed a branch.

mod common;

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use common::git::{git, git_at, out, FIXTURE_DATE};
use serde_json::Value;

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn peer(root: &Path, name: &str, commit: &str, peak: &str) {
    let base = format!(".yidam/tonpa/{name}");
    write(
        root,
        &format!("{base}/manifest.yml"),
        &format!("commit: \"{commit}\"\n"),
    );
    write(
        root,
        &format!("{base}/corpus/station.ont.yml"),
        "class: station\nproperties:\n  - name: usgs_id\n    type: string\n  - name: flood_peak\n    \
         type: number\n",
    );
    write(
        root,
        &format!("{base}/corpus/station/lf.yml"),
        &format!("class: station\nlabel: lf\nproperties:\n  usgs_id: \"01646500\"\n  flood_peak: {peak}\n"),
    );
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    write(
        root,
        ".yidam/corpus/gage.ont.yml",
        "class: gage\nproperties:\n  - name: station\n    type: string\n  - name: peak_cfs\n    \
         type: number\n",
    );
    write(root, ".yidam/corpus/question.ont.yml", "class: question\n");
    peer(root, "alpha", "aaa1111", "12000");
    peer(root, "beta", "bbb2222", "15500");
    let mut spec = String::from(
        "question = \"What peak has each gage recorded?\"\nquery = \"gage\"\nanswer = \
         \"peak_cfs\"\nkey = \"station\"\nlands_as = \"question\"\n",
    );
    for p in ["alpha", "beta", "omega"] {
        let _ = write!(
            spec,
            "\n[peers.{p}]\nclasses = {{ gage = \"station\" }}\nproperties = {{ station = \
             \"usgs_id\", peak_cfs = \"flood_peak\" }}\n"
        );
    }
    write(root, ".yidam/gathers/peaks.toml", &spec);
    git(root, &["add", "-A"]);
    git_at(root, &["commit", "-q", "-m", "genesis: seed"], FIXTURE_DATE);
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

fn json(root: &Path, args: &[&str]) -> Value {
    let (stdout, stderr, code) = yidam(root, args);
    assert_eq!(code, 0, "yidam {args:?} failed:\n{stdout}{stderr}");
    serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("not JSON: {e}\n{stdout}"))
}

#[test]
fn the_report_names_every_peer_and_the_branch_it_landed() {
    let dir = repo();
    let root = dir.path();
    let v = json(root, &["gather", "peaks", "--format", "json"]);
    assert_eq!(v["format_version"], "1");
    assert_eq!(v["kind"], "gather");
    let outcomes: Vec<(String, String)> = v["peers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["package"].as_str().unwrap().into(),
                p["outcome"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        outcomes,
        [
            ("alpha", "answered"),
            ("beta", "answered"),
            ("omega", "missing")
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
    );
    assert_eq!(v["disagreements"].as_array().unwrap().len(), 1);
    let branch = v["landed"]["branch"].as_str().unwrap();
    assert!(branch.starts_with("propose/gather/peaks/"), "{branch}");
    assert_eq!(v["landed"]["commits"].as_array().unwrap().len(), 2);
    // Written as git objects and one ref; the checkout is where it was.
    assert_eq!(out(root, &["status", "--porcelain"]), "");
    assert_eq!(out(root, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");

    let again = json(root, &["gather", "peaks", "--format", "json"]);
    assert_eq!(again["landed"]["unchanged"], true);
    let (text, _, code) = yidam(root, &["gather", "peaks"]);
    assert_eq!(code, 0);
    assert!(text.contains("unchanged"), "{text}");
}

#[test]
fn a_gather_that_does_not_exist_fails_and_says_where_it_looked() {
    let dir = repo();
    let (_, stderr, code) = yidam(dir.path(), &["gather", "nope"]);
    assert_eq!(code, 1);
    assert!(stderr.contains(".yidam/gathers/nope.toml"), "{stderr}");
}
