//! The release ordering is enforced rather than described.
//!
//! `sdk/rust/v*` must publish before `cli/v*` or the CLI's publish fails on a missing
//! `yidam-core`. That was true, documented, and written at the top of a workflow file — and
//! then not followed by the person who had written it a few hours earlier, because a comment
//! in a workflow is not in front of anyone at the moment they type `git tag`.
//!
//! `release.sh` is where that ordering lives now. These tests check the two ways it could
//! quietly stop being true: a layer VERSIONING.md names that the script cannot release, and
//! a document that goes back to telling people to type `git tag` directly.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// Run `./release.sh` from the repository root and return (stdout + stderr, success).
///
/// Always `--dry-run`: these tests must never be able to create a tag, whatever else they
/// get wrong.
fn release(args: &[&str]) -> (String, bool) {
    let out = Command::new("./release.sh")
        .args(args)
        .arg("--dry-run")
        .current_dir(repo_root())
        .output()
        .expect("release.sh is executable");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (text, out.status.success())
}

/// A version the manifest does not declare is refused, not tagged.
///
/// The tag and the manifest disagreeing is the mistake that cannot be undone once a publish
/// has run: crates.io keeps a version forever, and a yanked one is still downloadable by
/// anything that already resolved it.
#[test]
fn a_version_the_manifest_does_not_declare_is_refused() {
    let (out, ok) = release(&["sdk/rust", "9.9.9"]);
    assert!(
        !ok,
        "release.sh accepted a version nothing declares:\n{out}"
    );
    assert!(
        out.contains("version-mismatch"),
        "expected a version-mismatch refusal, got:\n{out}"
    );
    assert!(
        out.contains("Nothing was tagged"),
        "release.sh must say plainly that it did not tag:\n{out}"
    );
}

/// A layer that is not a layer is refused before anything else is inspected.
#[test]
fn a_layer_that_is_not_a_layer_is_refused() {
    let (out, ok) = release(&["sdk/haskell", "1.0.0"]);
    assert!(
        !ok,
        "release.sh accepted a layer that does not exist:\n{out}"
    );
    assert!(
        out.contains("unknown-layer"),
        "expected an unknown-layer refusal, got:\n{out}"
    );
}

/// Every layer VERSIONING.md gives a tag pattern must be one the script can cut.
///
/// A layer the script does not know is a layer released by hand, and a layer released by
/// hand is the state this whole file is a response to.
#[test]
fn every_layer_versioning_md_names_is_one_the_script_can_release() {
    let script = read("release.sh");
    // (the tag pattern as VERSIONING.md writes it, the layer argument release.sh takes)
    const LAYERS: &[(&str, &str)] = &[
        ("`sdk/rust/v{major}.{minor}.{patch}`", "sdk/rust)"),
        ("`cli/v{major}.{minor}.{patch}`", "cli)"),
        ("`bootstrap/v{major}.{minor}.{patch}`", "bootstrap)"),
        ("`editor/v{major}.{minor}.{patch}`", "editor)"),
        ("`v{major}.{minor}.{patch}`", "template)"),
    ];
    let versioning = read("VERSIONING.md");
    for (pattern, branch) in LAYERS {
        assert!(
            versioning.contains(pattern),
            "VERSIONING.md no longer documents the tag pattern {pattern}; this test is \
             checking a layer that may not exist any more"
        );
        assert!(
            script.contains(branch),
            "VERSIONING.md documents {pattern} and release.sh has no `{branch}` branch — \
             that layer would be tagged by hand"
        );
    }
}

/// The CLI's release must ask crates.io whether `yidam-core` is there yet.
///
/// This is the precondition that actually failed. The manifest says which version is
/// *required*; only the registry says which exists, so the check has to be a request. The
/// assertion is on the script rather than on a run of it, because a run would need the
/// network and this suite must not.
#[test]
fn the_cli_release_checks_that_yidam_core_is_published() {
    let script = read("release.sh");
    assert!(
        script.contains("crates.io/api/v1/crates/yidam-core/"),
        "release.sh must ask crates.io whether the required yidam-core exists; the CLI's \
         publish fails on a missing one, and the manifest cannot answer it"
    );
    assert!(
        script.contains("dependency-unpublished"),
        "release.sh must name the unpublished-dependency refusal, so the message says `tag \
         sdk/rust first` rather than reporting a cargo error from inside CI"
    );
    // 404 and "crates.io is down" want opposite responses — tag the SDK first, or try again
    // later. Collapsing them is how a precondition check becomes one people learn to skip.
    assert!(
        script.contains("registry-unreachable"),
        "release.sh must distinguish an unreachable registry from an unpublished version"
    );
}

/// The CLI's release must ask whether the Homebrew tap's credential exists.
///
/// `cli/v0.2.1` published four tarballs, both crates, and every checksum, and went red at
/// one job: the tap push, on a `HOMEBREW_TAP_TOKEN` that had never been created. The
/// failure was loud and the log said exactly what to do — and "loud" and "fixed" are
/// different states. In between them the tap served 0.2.0, which is the release whose Linux
/// binary does not run on Debian 12, which is why 0.2.1 exists (#246).
///
/// A missing credential is knowable before the tag. After it, the assets are out and the
/// release notes already claim `brew install` works.
#[test]
fn the_cli_release_checks_that_the_tap_token_exists() {
    let script = read("release.sh");
    assert!(
        script.contains("HOMEBREW_TAP_TOKEN"),
        "release.sh must ask whether the tap's token exists; without it a cli/v* tag \
         publishes every channel but the tap, and finds out afterwards"
    );
    assert!(
        script.contains("tap-token-missing"),
        "release.sh must name the missing-token refusal, so the message says which PAT to \
         create rather than reporting a red job from inside a release that already shipped"
    );
    // Secrets are listable only with admin. "Not there" and "you cannot see" want opposite
    // responses — create the PAT, or ask someone who can look — and collapsing them is how
    // a precondition check becomes one people learn to skip.
    assert!(
        script.contains("tap-token-unknown"),
        "release.sh must distinguish an absent token from one it is not allowed to see"
    );
}

/// A `cli/v*` tag must require tap.yml at HEAD, not only release.yml.
///
/// release.yml calls the tap by path, and a local `uses:` resolves at the caller's ref. A
/// tag carrying one file and not the other fires a release whose tap job cannot start —
/// the same silence as tagging before a workflow exists, which is the failure the
/// no-workflow refusal was written for.
#[test]
fn the_cli_release_requires_every_workflow_the_tag_fires() {
    let script = read("release.sh");
    let cli = script
        .lines()
        .find(|l| l.contains("TAG=\"cli/v$VERSION\""))
        .expect("release.sh no longer has a cli layer");
    for wf in [
        ".github/workflows/release.yml",
        ".github/workflows/tap.yml",
        ".github/workflows/publish-crates.yml",
    ] {
        assert!(
            cli.contains(wf),
            "a cli tag fires {wf} and release.sh does not require it at HEAD: {cli}"
        );
        assert!(
            repo_root().join(wf).exists(),
            "release.sh requires {wf} and it does not exist"
        );
    }
}

/// The release process must point at the script, not at `git tag`.
///
/// Every fact this script enforces was already written down somewhere. Being written down is
/// what failed; a document that goes back to spelling out `git tag -s` restores exactly the
/// state that produced the problem.
#[test]
fn the_documented_release_process_uses_the_script() {
    let versioning = read("VERSIONING.md");
    let process = versioning
        .split("## Release process")
        .nth(1)
        .expect("VERSIONING.md has a Release process section");
    assert!(
        process.contains("release.sh"),
        "the release process does not mention release.sh; the ordering is back to being \
         something the releaser has to remember"
    );
    assert!(
        !process.contains("git tag -s v"),
        "the release process spells out `git tag` again — that is the instruction that was \
         followed in the wrong order"
    );
    let mise = read("mise.toml");
    assert!(
        mise.contains("[tasks.release]"),
        "mise.toml has no `release` task, so `mise tasks` does not list the one command a \
         releaser needs"
    );
}
