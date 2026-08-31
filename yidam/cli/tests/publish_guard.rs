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

/// Every directory the bundle carries must be one the guard inspects.
///
/// The guard refuses when declared-private material sits inside a bundled directory. That
/// rule is only as good as its list: a directory added to the bundle and not added here is
/// one the guard does not know it is publishing, and the failure is silent — the workflow
/// goes green and ships the material.
#[test]
fn the_guard_inspects_every_directory_the_bundle_carries() {
    let w = workflow();
    let line = w
        .lines()
        .find(|l| l.trim_start().starts_with("bundled="))
        .expect("the guard no longer declares which directories a bundle carries");

    // Derived rather than transcribed. `vault::derived_sources` answers the same question for
    // `yidam vault push --bundle`, and two lists of one fact, each pinned only by itself, is
    // how one of them silently stops matching. This one did: until #443 it named the three
    // directories the archive carries as files and reasoned that `index/` was "generated
    // rather than authored" — true of the file, false of its contents. A bundle carries
    // `index/corpus.arrow`, that index has a `text` column, and `cmd/embed.rs` fills it from
    // the catalog as well as the corpus. So a private catalog entry's prose ships inside the
    // archive while no catalog file does.
    for dir in yidam::vault::derived_sources(yidam::vault::Derived::Bundle) {
        assert!(
            line.contains(dir),
            "a bundle publishes {dir} and the guard does not inspect it: {line}"
        );
    }
}

/// The catalog is the entry that was missing, and it is the one a reader is most likely to
/// remove again — no catalog *file* is in the archive, so the omission looks correct.
#[test]
fn the_guard_inspects_the_catalog_even_though_no_catalog_file_is_bundled() {
    let w = workflow();
    let line = w
        .lines()
        .find(|l| l.trim_start().starts_with("bundled="))
        .expect("the guard no longer declares which directories a bundle carries");
    assert!(
        line.contains(".yidam/catalog"),
        "a bundle carries the vector index, and that index encodes catalog text: {line}"
    );
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
