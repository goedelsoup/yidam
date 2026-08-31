//! The derived-repo index workflow must fail closed, and must not drift from the CLI.
//!
//! `sadhana/github/workflows/index.yml` builds a vector index on a runner that can, and puts
//! it in a vault. It therefore holds credentials, writes to the repository, and publishes an
//! artifact that **outlives the access that produced it** — three properties that make a
//! reasonable-looking diff dangerous.
//!
//! These are structural assertions on the workflow text. They cannot prove the shell is
//! correct; nothing here has been run against a real store. What they pin is the shape a later
//! edit could quietly remove, and — the one that matters most — that the workflow's idea of
//! what an index encodes is the same as [`yidam::vault::derived_sources`]'s. Two lists of the
//! same fact, each pinned only by itself, is how one of them silently stops matching.

use std::path::PathBuf;

fn workflow() -> String {
    let p =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sadhana/github/workflows/index.yml");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

fn parsed() -> serde_yaml::Value {
    serde_yaml::from_str(&workflow()).expect("index.yml parses")
}

fn steps() -> Vec<serde_yaml::Value> {
    parsed()["jobs"]["index"]["steps"]
        .as_sequence()
        .expect("the index job has steps")
        .clone()
}

fn step_index(name_fragment: &str) -> usize {
    steps()
        .iter()
        .position(|s| {
            s["name"]
                .as_str()
                .is_some_and(|n| n.to_lowercase().contains(&name_fragment.to_lowercase()))
        })
        .unwrap_or_else(|| panic!("no step whose name contains {name_fragment:?}"))
}

/// **The anti-drift assertion, now that there is nothing to drift.**
///
/// Until #440 this workflow declared `derived_from=".yidam/corpus .yidam/catalog"` and this
/// test asserted the list matched `vault::derived_sources(Derived::Index)`. Two lists of one
/// fact, each pinned only by the other — the shape #443 came out of.
///
/// The list now reaches the decision from inside the binary that builds the index, so a
/// directory `cmd/embed.rs` learns to read is one this guard inspects without anybody editing
/// a workflow. What is pinned here is that nobody puts a second copy back.
#[test]
fn the_workflow_asks_the_policy_what_an_index_is_built_from() {
    let w = workflow();
    assert!(
        w.contains("yidam policy gate disclose/derived --kind index"),
        "the guard must ask the policy what an index encodes:\n{w}"
    );
    assert!(
        !w.contains("derived_from="),
        "a directory list is back in the workflow. It belongs to `vault::derived_sources`, \
         inside the binary that walks those directories"
    );
}

/// The privacy guard must come before the build, not after it.
///
/// Not only to save twenty minutes: a job that builds first has the private material encoded
/// on the runner's disk before anything has decided whether it may be.
#[test]
fn the_privacy_guard_runs_before_the_index_is_built() {
    assert!(
        step_index("private") < step_index("Embed and build"),
        "the privacy guard must precede the build"
    );
}

/// Nothing is stored before an index has actually been built. `set -eu` in each step plus the
/// order here is what makes "pushes after a *successful* index build" true.
#[test]
fn the_store_step_comes_after_the_build() {
    assert!(step_index("Embed and build") < step_index("Store the index"));
    assert!(step_index("Store the index") < step_index("Commit the lock"));
}

/// A repository with no vault must be told before it spends a build, not after.
#[test]
fn a_repository_with_no_vault_is_refused_first() {
    let first = steps()
        .iter()
        .position(|s| {
            s["name"]
                .as_str()
                .is_some_and(|n| n.contains("vault must be configured"))
        })
        .expect("no vault-configured check");
    assert!(
        first < step_index("Embed and build"),
        "the vault check must precede the build"
    );
}

/// **It ships as a rehearsal.** A publishing workflow nobody has ever run is one that gets
/// debugged on the day it matters, so the default must send nothing.
#[test]
fn the_default_run_sends_nothing() {
    let dry = &parsed()["on"]["workflow_dispatch"]["inputs"]["dry_run"];
    assert_eq!(
        dry["default"].as_bool(),
        Some(true),
        "dry_run must default to true, got: {dry:?}"
    );
}

/// A commit is made only on a real run. A rehearsal that writes to the repository is not a
/// rehearsal.
#[test]
fn the_lock_is_committed_only_when_something_was_actually_stored() {
    let commit = &steps()[step_index("Commit the lock")];
    let cond = commit["if"].as_str().unwrap_or_default();
    assert!(
        cond.contains("dry_run") && cond.contains("false"),
        "the commit step must be gated on a real run, got: {cond:?}"
    );
}

/// Credentials come from secrets and from nowhere else. A literal key in a committed workflow
/// is the failure `.yidam/config.toml` is kept free of one to prevent.
#[test]
fn credentials_come_only_from_secrets() {
    let store = &steps()[step_index("Store the index")];
    let env = store["env"].as_mapping().expect("the store step sets env");
    assert!(!env.is_empty(), "no credentials are passed at all");
    for (k, v) in env {
        let (k, v) = (k.as_str().unwrap_or(""), v.as_str().unwrap_or(""));
        assert!(
            v.contains("secrets."),
            "{k} must come from secrets, got: {v:?}"
        );
        assert!(
            k.starts_with("YIDAM_VAULT_"),
            "{k} is not a per-vault credential variable"
        );
    }
}

/// The workflow is manual only. `pull_request` would expose the credentials above to any fork
/// that opened one.
#[test]
fn the_workflow_never_runs_on_a_pull_request() {
    let on = &parsed()["on"];
    assert!(
        on.get("pull_request").is_none(),
        "this workflow holds credentials and must not run on a pull request"
    );
    assert!(
        on.get("workflow_dispatch").is_some(),
        "it has to be startable somehow"
    );
}
