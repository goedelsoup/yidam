//! The derived-repo release workflow must fail closed.
//!
//! `sadhana/github/workflows/release.yml` publishes a corpus as a `.yiz` bundle. A bundle
//! **travels**: it leaves the repository as one file and is unpacked and read somewhere
//! else, by someone whose access to the repository is no longer what governs it. So the
//! properties below are not stylistic — each one is a way the workflow could publish
//! something it should not, and none of them is visible in a diff that looks reasonable.
//!
//! These are structural assertions on the workflow text. They cannot prove the shell is
//! correct; that was checked by running the guard against fixtures. What they pin is the
//! shape a later edit could quietly remove.

use std::path::PathBuf;

fn workflow() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sadhana/github/workflows/release.yml");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// The bundle job must not run unless the guard passed.
///
/// Without `needs:`, the two jobs run concurrently and a corpus can be built and published
/// while the check that would have stopped it is still deciding.
#[test]
fn the_bundle_job_waits_for_the_guard() {
    let w: serde_yaml::Value = serde_yaml::from_str(&workflow()).expect("release.yml parses");
    let needs = &w["jobs"]["bundle"]["needs"];
    assert!(
        needs.as_str() == Some("guard")
            || needs
                .as_sequence()
                .is_some_and(|s| s.iter().any(|n| n.as_str() == Some("guard"))),
        "the bundle job must depend on the guard, got: {needs:?}"
    );
}

/// The guard **asks** what a bundle carries. It no longer keeps its own copy.
///
/// Until #440 this workflow declared `bundled=".yidam/corpus …"` and a test asserted the list
/// matched `vault::derived_sources(Derived::Bundle)`. That was a mirror, and #443 is what a
/// mirror costs: the workflow named the three directories the archive carries as *files* and
/// omitted `.yidam/catalog`, reasoning that `index/` was generated rather than authored — true
/// of the file, false of its contents, because the bundled `index/corpus.arrow` has a `text`
/// column that `cmd/embed.rs` fills from the catalog.
///
/// The list now reaches the decision from inside the binary that packs the archive, so there is
/// no second copy to keep in step. What this pins is that nobody reintroduces one.
#[test]
fn the_guard_asks_the_policy_rather_than_keeping_its_own_list() {
    let w = workflow();
    assert!(
        w.contains("yidam policy gate disclose/derived --kind bundle"),
        "the guard must ask the policy what a bundle may carry:\n{w}"
    );
    assert!(
        !w.contains("bundled="),
        "a directory list is back in the workflow. That is the shape #443 came from — the \
         list belongs to `vault::derived_sources`, inside the binary that packs the archive"
    );
}

/// The public-repository half is still asked, and still reads the event payload.
///
/// Two decisions, not one: a bundle may not carry declared material *whatever* the repository's
/// visibility, and a public repository may not hold it at all. The first is the stricter and is
/// why this job exists rather than reusing ci.yml's.
#[test]
fn the_guard_asks_about_the_repository_as_well_as_the_bundle() {
    let w = workflow();
    assert!(
        w.contains("yidam policy gate disclose/at_rest"),
        "the at-rest half must still be asked:\n{w}"
    );
    assert!(
        w.contains("github.event.repository.private"),
        "visibility must come from the payload the runner already has — a `gh api` call would \
         make this job non-hermetic"
    );
}

/// **A repository that declared nothing pays a runner-second, as it always did.**
///
/// The job is wired in from genesis so that the day a repository grows material it does not
/// want published, the rule is one file away rather than a workflow edit nobody remembers to
/// make. Installing a binary unconditionally to usually do nothing would undo that, so every
/// step that costs anything is gated on the manifest existing.
#[test]
fn the_guard_installs_nothing_when_no_path_is_declared_private() {
    let w: serde_yaml::Value = serde_yaml::from_str(&workflow()).expect("release.yml parses");
    let steps = w["jobs"]["guard"]["steps"]
        .as_sequence()
        .expect("the guard job has steps");

    let gate: Vec<&serde_yaml::Value> = steps
        .iter()
        .filter(|s| {
            let name = s["name"].as_str().unwrap_or_default();
            let uses = s["uses"].as_str().unwrap_or_default();
            let run = s["run"].as_str().unwrap_or_default();
            // Everything that costs real time: the toolchain, the cache, the install, and the
            // gates themselves.
            uses.contains("rust-toolchain")
                || uses.contains("actions/cache")
                || name.contains("Install the yidam CLI")
                || run.contains("yidam policy gate")
                || name.contains("Read the pinned yidam commit")
        })
        .collect();
    assert!(
        gate.len() >= 5,
        "expected the expensive steps to be discoverable, found {}",
        gate.len()
    );
    for step in gate {
        let cond = step["if"].as_str().unwrap_or_default();
        assert!(
            cond.contains("steps.declared.outputs.any == 'true'"),
            "this step runs even when nothing is declared private: {:?}",
            step["name"].as_str().or(step["uses"].as_str())
        );
    }
}

/// Publishing must be gated on a tag, so `workflow_dispatch` is a rehearsal and not a release.
///
/// A publishing workflow nobody has ever run gets debugged for the first time on the day it
/// matters; a rehearsal that publishes is not a rehearsal.
#[test]
fn manual_runs_build_but_do_not_publish() {
    let w: serde_yaml::Value = serde_yaml::from_str(&workflow()).expect("release.yml parses");
    let steps = w["jobs"]["bundle"]["steps"]
        .as_sequence()
        .expect("bundle steps");
    let publish = steps
        .iter()
        .find(|s| s["name"].as_str() == Some("Publish"))
        .expect("no Publish step");
    let cond = publish["if"].as_str().unwrap_or_default();
    assert!(
        cond.contains("refs/tags/"),
        "the publish step must be gated on a tag ref, got: {cond:?}"
    );
}

/// Opting in is required, and the check must come before anything is built.
#[test]
fn publishing_requires_an_explicit_opt_in() {
    let w = workflow();
    assert!(
        w.contains(".yidam/publishable"),
        "the guard no longer requires an explicit opt-in marker"
    );
    let guard_at = w.find("guard:").expect("guard job");
    let marker_at = w.find(".yidam/publishable").expect("marker");
    let bundle_at = w.find("\n  bundle:").expect("bundle job");
    assert!(
        guard_at < marker_at && marker_at < bundle_at,
        "the opt-in check must live in the guard job, ahead of the bundle job"
    );
}
