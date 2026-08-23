//! The installer is the first thing a stranger runs, so its refusals matter more than its
//! happy path.
//!
//! It is piped to `sh` from a URL: whoever runs it has not read it, cannot inspect what it
//! will do, and has no way to tell a partial install from a complete one. Every property
//! below is a way it could quietly leave a machine worse than it found it.
//!
//! The happy path was verified by running it — it fetched `cli/v0.2.0`, verified the
//! checksum, and installed a binary reporting `yidam 0.2.0 (ee3a7f6) [reports]`. These tests
//! pin the parts a later edit could remove without the happy path noticing.

use std::path::PathBuf;

fn script() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../install.sh");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// An unverified binary must never be installed.
///
/// The tempting shape is "verify if a checksum tool is available". That installs silently
/// on any machine without one, which is exactly the machine least able to notice.
#[test]
fn a_missing_checksum_tool_refuses_rather_than_skips() {
    let s = script();
    let refusal = s
        .lines()
        .find(|l| l.contains("refusing to install an unverified binary"))
        .expect("the installer no longer refuses when it cannot verify");
    assert!(
        refusal.contains("fail"),
        "the no-checksum branch must abort, not warn: {refusal}"
    );
}

/// The version must be resolved, never baked in.
///
/// A hardcoded version works the day it is written and 404s on the next release — the
/// failure mode that does not announce itself.
#[test]
fn the_release_is_resolved_not_hardcoded() {
    let s = script();
    assert!(
        s.contains("releases/latest"),
        "the installer must resolve the latest release"
    );
    // A literal `cli/v<digits>` outside the pattern-match arms would be a pinned version.
    for line in s.lines() {
        let l = line.trim();
        if l.starts_with('#') || l.contains("cli/v*") || l.contains("cli/v}") {
            continue;
        }
        assert!(
            !l.contains("cli/v0."),
            "a version is hardcoded, so this script expires: {l}"
        );
    }
}

/// `set -eu` — an installer that continues past a failed step is how half-installs happen.
#[test]
fn the_script_stops_at_the_first_failure() {
    assert!(
        script().lines().any(|l| l.trim() == "set -eu"),
        "install.sh must set -eu"
    );
}

/// POSIX sh, not bash. It is piped to whatever `/bin/sh` is on a machine the reader did not
/// choose, and `[[`, arrays, and `pipefail` are not available there.
#[test]
fn the_script_is_posix_sh() {
    let s = script();
    assert!(
        s.starts_with("#!/bin/sh"),
        "install.sh must declare #!/bin/sh"
    );
    for (n, line) in s.lines().enumerate() {
        let l = line.trim();
        if l.starts_with('#') {
            continue;
        }
        for bashism in ["[[", "set -o pipefail", "declare ", "local "] {
            assert!(
                !l.contains(bashism),
                "line {}: `{bashism}` is not POSIX sh: {l}",
                n + 1
            );
        }
    }
}

/// An unsupported platform must say what to do instead, not fail bare.
#[test]
fn an_unsupported_platform_names_the_alternative() {
    let s = script();
    let arm = s
        .lines()
        .find(|l| l.contains("no prebuilt binary for"))
        .expect("no unsupported-platform message");
    assert!(
        arm.contains("cargo install"),
        "tell the reader how to build instead: {arm}"
    );
}
