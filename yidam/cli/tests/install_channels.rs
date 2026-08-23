//! Every documented way to get the binary is checked against reality.
//!
//! `cargo binstall yidam` stood in the README for a release cycle while `yidam` did not
//! exist on crates.io. Nothing caught it, because nothing here had ever asked the question
//! CI does not ask: not "does the artifact build" but "can anyone obtain one". The answer
//! lives in `.github/workflows/install-channels.yml`, which runs each documented line in a
//! clean container weekly.
//!
//! This file is the join between the two. A workflow that checks three channels while the
//! README documents four is the same failure one channel later, and the diff that adds the
//! fourth line to the README is the diff that looks completely fine.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// A line the README tells someone to run, and the substring the channel check must run to
/// prove it works. The two are separate because the workflow resolves the repository and the
/// released tag rather than hardcoding them — it is the *channel* that must correspond, not
/// the spelling.
const CHANNELS: &[(&str, &str)] = &[
    ("install.sh | sh", "install.sh | sh"),
    (
        "brew install goedelsoup/tap/yidam",
        "brew install goedelsoup/tap/yidam",
    ),
    ("cargo binstall yidam", "cargo binstall"),
    ("cargo install --git", "cargo install --git"),
];

/// The prefixes that make a line an instruction to install this binary rather than prose
/// about one.
const INSTALL_PREFIXES: &[&str] = &[
    "curl -fsSL",
    "brew install",
    "cargo binstall",
    "cargo install",
];

fn documented_install_lines(doc: &str) -> Vec<String> {
    read(doc)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| INSTALL_PREFIXES.iter().any(|p| l.starts_with(p)))
        .filter(|l| l.contains("yidam"))
        .collect()
}

/// Nothing may be documented as an install path without something running it.
///
/// The assertion runs in the direction that matters. A check with no documented line is
/// harmless — extra coverage. A documented line with no check is the bug this whole issue
/// was: an instruction that cannot succeed, on the front page, indefinitely.
#[test]
fn every_documented_install_line_is_checked_end_to_end() {
    let workflow = read(".github/workflows/install-channels.yml");

    for doc in ["README.md", "docs/quickstart.md"] {
        for line in documented_install_lines(doc) {
            let channel = CHANNELS.iter().find(|(marker, _)| line.contains(marker));
            let (_, probe) = channel.unwrap_or_else(|| {
                panic!(
                    "{doc} documents an install path this test does not know:\n  {line}\n\
                     Add it to CHANNELS *and* to .github/workflows/install-channels.yml. A \
                     channel nobody runs is how `cargo binstall yidam` came to be documented \
                     and impossible."
                )
            });
            assert!(
                workflow.contains(probe),
                "{doc} documents `{line}` and install-channels.yml never runs `{probe}` — \
                 the README is making a promise no job checks"
            );
        }
    }
}

/// Each channel must prove it installed the *released* version, not merely something.
///
/// A tap that lags a release, or a binstall that quietly compiled from source, both end with
/// a working `yidam` on PATH. Only the version tells them apart.
#[test]
fn each_channel_asserts_the_released_version() {
    let workflow = read(".github/workflows/install-channels.yml");
    let checks = workflow.matches("yidam --version").count();
    assert!(
        checks >= CHANNELS.len(),
        "install-channels.yml runs `yidam --version` {checks} time(s) for {} documented \
         channel(s); every channel must assert what it installed",
        CHANNELS.len()
    );
    assert!(
        workflow.contains("needs.released.outputs.version"),
        "install-channels.yml must compare against the latest release's version; asserting \
         that *a* binary runs is what let a stale tap look healthy"
    );
}

/// The check must run without anyone remembering to run it, and must run when it lands.
///
/// `publish-crates.yml` was committed after the only tag this repository had ever cut. A
/// workflow fires only if it exists at the ref being pushed, so it never ran once — the
/// exact silence a weekly job added today would inherit.
#[test]
fn the_channel_check_runs_on_a_schedule_and_when_it_changes() {
    let workflow = read(".github/workflows/install-channels.yml");
    assert!(
        workflow.contains("schedule:") && workflow.contains("cron:"),
        "install-channels.yml must run on a schedule; a channel breaks between releases, \
         not during one"
    );
    assert!(
        workflow.contains(".github/workflows/install-channels.yml"),
        "install-channels.yml must trigger on pushes that touch itself, so it runs at least \
         once at the commit that adds it"
    );
}

/// A publish that published nothing must not be green.
///
/// `CARGO_REGISTRY_TOKEN` has never been configured. With the earlier warn-and-exit-0, the
/// first tag to reach that workflow would have run green and uploaded nothing, while the
/// README went on telling people to run `cargo binstall yidam`.
#[test]
fn a_publish_without_a_token_fails_rather_than_passing() {
    let workflow = read(".github/workflows/publish-crates.yml");
    let guard = workflow
        .split("CARGO_REGISTRY_TOKEN:-")
        .nth(1)
        .expect("publish-crates.yml must guard on an unset CARGO_REGISTRY_TOKEN");
    let body: String = guard.lines().take(6).collect::<Vec<_>>().join("\n");
    assert!(
        body.contains("exit 1"),
        "publish-crates.yml must fail when no registry token is configured, not exit 0:\n{body}"
    );
    assert!(
        !body.contains("exit 0"),
        "publish-crates.yml still exits 0 on a missing token — a channel that did not ship \
         must not look like one that did:\n{body}"
    );
}

/// The Linux tarballs must be linked against a glibc old enough to be worth serving.
///
/// A glibc binary runs on the version it was linked against or newer, and never on older.
/// `ubuntu-latest` moved to 24.04 / glibc 2.39, so `cli/v0.2.0`'s Linux tarball downloaded,
/// checksummed and installed on Debian 12 and then would not start. Every step of the
/// install reported success; only running it failed.
///
/// The runner image *is* the floor — nothing else in the repository declares one — so a diff
/// putting `ubuntu-latest` back would raise it silently.
#[test]
fn the_linux_release_builds_do_not_float_to_the_newest_glibc() {
    let text = read(".github/workflows/release.yml");
    let mut linux_targets = 0;
    let mut os = None;
    for line in text.lines().map(str::trim) {
        if let Some(t) = line.strip_prefix("- target: ") {
            os = None;
            if t.contains("linux-gnu") {
                linux_targets += 1;
                os = Some(t.to_string());
            }
        } else if let (Some(target), Some(runner)) = (&os, line.strip_prefix("os: ")) {
            assert_ne!(
                runner, "ubuntu-latest",
                "{target} builds on ubuntu-latest, whose glibc rises without a diff; pin the \
                 runner so the binary's minimum glibc is a decision and not a default"
            );
        }
    }
    assert!(
        linux_targets > 0,
        "release.yml builds no *-linux-gnu target; this test is guarding nothing"
    );
}

/// The cross-compile CI runs must be the one the release runs.
///
/// `release.yml` builds `aarch64-unknown-linux-gnu` and fires on `cli/v*` and
/// workflow_dispatch only, so no pull request had ever compiled that target — and `tonpa`
/// joining the default set put a TLS stack, and `ring`'s vendored C, into the light build
/// after the only release that exists. `ci.yml`'s cross job exists to compile it on every
/// PR instead of on the tag.
///
/// It is worth exactly as much as its resemblance to the real thing. A cross job that stops
/// passing `--locked`, or names a different linker, is a green check for a build nobody
/// ships — which is the same shape as a documented install path nobody runs.
#[test]
fn the_cross_compile_check_mirrors_the_release_build() {
    let ci = read(".github/workflows/ci.yml");
    let release = read(".github/workflows/release.yml");

    let cross = ci
        .split("  cross:")
        .nth(1)
        .expect(
            "ci.yml must have a cross-compile job; release.yml's aarch64 target is the \
                 one nothing else builds",
        )
        .split("\n  cli-full:")
        .next()
        .unwrap();

    for setting in [
        "aarch64-unknown-linux-gnu",
        "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
        "CC_aarch64_unknown_linux_gnu",
        "aarch64-linux-gnu-gcc",
        "gcc-aarch64-linux-gnu",
        "--locked",
    ] {
        assert!(
            cross.contains(setting),
            "ci.yml's cross job does not set `{setting}`, which release.yml's build for this \
             target does — the rehearsal has to be the performance"
        );
        assert!(
            release.contains(setting),
            "release.yml no longer sets `{setting}`; ci.yml's cross job is now rehearsing a \
             build that does not happen"
        );
    }

    // No `--features`: the default IS the light set, and spelling it in either place makes
    // a second definition for the two to drift apart on. Read from the commands rather than
    // the whole block, since the comments above them discuss the flag by name.
    let commanded: String = cross
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !commanded.contains("--features"),
        "ci.yml's cross job names features explicitly; the release build does not, so the \
         two would be compiling different things"
    );
    assert!(
        cross.contains("ARM aarch64"),
        "ci.yml's cross job must assert the artifact IS aarch64 — `it compiled` and `it \
         compiled for aarch64` are different claims, and cc-rs falling back to the host \
         compiler is what makes them differ"
    );
}
