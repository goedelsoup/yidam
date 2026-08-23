//! The tap is the one distribution channel nobody in this repository ever runs.
//!
//! `install.sh` is exercised by piping it to `sh`; `cargo binstall` is exercised by the
//! crates.io release. A Homebrew formula is rendered by CI, pushed to a second repository,
//! and read by `brew` on a stranger's laptop — so a defect in it surfaces as *their*
//! failed install, days later, with nothing in this repository having gone red.
//!
//! These drive `render-formula.sh` against fixture checksums and assert on what it
//! writes. The generator is a script and not a heredoc inside the workflow precisely so
//! that this file can exist: a step inlined in `release.yml` is testable only by cutting a
//! release.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One distinct 64-hex checksum per target, so a formula that pairs a url with the wrong
/// platform's hash is visible rather than self-consistent.
fn checksums(dir: &Path, version: &str, targets: &[&str]) {
    for (n, t) in targets.iter().enumerate() {
        let hash = format!("{}", n + 1).repeat(64);
        std::fs::write(
            dir.join(format!("yidam-{version}-{t}.tar.gz.sha256")),
            format!("{hash}  yidam-{version}-{t}.tar.gz\n"),
        )
        .unwrap();
    }
}

fn render_in(dir: &Path, version: &str) -> Output {
    Command::new(root().join("render-formula.sh"))
        .arg(version)
        .arg(dir)
        .output()
        .expect("running render-formula.sh")
}

fn formula(version: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    checksums(dir.path(), version, &TARGETS);
    let out = render_in(dir.path(), version);
    assert!(
        out.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("the formula is UTF-8")
}

/// Every target the release workflow builds gets a url, and every url gets that target's
/// own checksum.
///
/// The failure this rules out is the quiet one: a formula in which all four blocks carry
/// the same hash installs correctly on the platform the hash belongs to and fails checksum
/// verification on the other three, which reads to the user as a corrupted download.
#[test]
fn each_platform_gets_its_own_url_and_its_own_checksum() {
    let f = formula("1.2.3");
    for (n, t) in TARGETS.iter().enumerate() {
        let url = format!(
            "https://github.com/goedelsoup/yidam/releases/download/cli/v1.2.3/yidam-1.2.3-{t}.tar.gz"
        );
        assert!(f.contains(&url), "no url for {t}:\n{f}");
        let hash = format!("{}", n + 1).repeat(64);
        let block = f
            .split(&url)
            .nth(1)
            .expect("the url appears once")
            .lines()
            .nth(1)
            .expect("a sha256 line follows the url");
        assert!(
            block.contains(&hash),
            "the sha256 under {t}'s url is not {t}'s: {block}"
        );
    }
}

/// The release workflow's matrix and this generator must name the same four targets.
///
/// They are two transcriptions of one list. A target added to the matrix and not here ships
/// a release the tap silently cannot serve; one removed from the matrix and left here
/// renders a formula with a url to an asset that was never uploaded.
#[test]
fn the_generator_covers_exactly_the_targets_the_release_builds() {
    let w = std::fs::read_to_string(root().join(".github/workflows/release.yml")).unwrap();
    let workflow: serde_yaml::Value = serde_yaml::from_str(&w).expect("release.yml parses");
    let matrix: Vec<String> = workflow["jobs"]["build"]["strategy"]["matrix"]["include"]
        .as_sequence()
        .expect("the build matrix")
        .iter()
        .map(|e| e["target"].as_str().expect("a target").to_string())
        .collect();

    let script = std::fs::read_to_string(root().join("render-formula.sh")).unwrap();
    for t in &matrix {
        assert!(
            script.contains(t.as_str()),
            "release.yml builds {t} and render-formula.sh never mentions it"
        );
    }
    for t in TARGETS {
        assert!(
            matrix.iter().any(|m| m == t),
            "the formula renders a url for {t} and the release matrix does not build it"
        );
    }
}

/// A missing platform must fail the render, not omit a block.
///
/// An `on_arm` that simply is not there is a formula Homebrew accepts: it installs for
/// everyone else and reports "yidam: no available formula" to exactly the people who cannot
/// tell whether they are unsupported or the release is broken.
#[test]
fn a_missing_platform_is_a_hard_failure() {
    let dir = tempfile::tempdir().unwrap();
    checksums(dir.path(), "1.2.3", &TARGETS[1..]);
    let out = render_in(dir.path(), "1.2.3");
    assert!(
        !out.status.success(),
        "rendered a formula with a platform missing:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains(TARGETS[0]),
        "the failure must name the platform it lacks: {err}"
    );
}

/// A truncated or empty checksum must fail here rather than on the user's machine.
///
/// `awk '{print $1}'` over a malformed file yields something — the empty string, a partial
/// hash — and the formula it renders is syntactically perfect. The error surfaces as
/// `SHA256 mismatch` during someone else's install.
#[test]
fn a_malformed_checksum_is_refused() {
    for bad in ["", "not-a-hash  yidam.tar.gz\n", "abc123  yidam.tar.gz\n"] {
        let dir = tempfile::tempdir().unwrap();
        checksums(dir.path(), "1.2.3", &TARGETS);
        std::fs::write(
            dir.path()
                .join(format!("yidam-1.2.3-{}.tar.gz.sha256", TARGETS[0])),
            bad,
        )
        .unwrap();
        let out = render_in(dir.path(), "1.2.3");
        assert!(
            !out.status.success(),
            "accepted the checksum {bad:?} and rendered:\n{}",
            String::from_utf8_lossy(&out.stdout)
        );
    }
}

/// The version is an argument, never a literal.
///
/// A version baked into the generator renders the same formula for every release and the
/// tap serves an old binary under a new name — the failure the whole generated-formula
/// approach exists to prevent.
#[test]
fn no_version_is_hardcoded() {
    let script = std::fs::read_to_string(root().join("render-formula.sh")).unwrap();
    for (n, line) in script.lines().enumerate() {
        let l = line.trim();
        if l.starts_with('#') {
            continue;
        }
        assert!(
            !l.contains("cli/v0.") && !l.contains("yidam-0."),
            "line {}: a version is baked in, so the formula expires: {l}",
            n + 1
        );
    }
    // And the rendered output moves with the argument.
    assert!(formula("9.9.9").contains("version \"9.9.9\""));
}

/// The formula must run the binary it installed and check what it says.
///
/// `brew test` is the only place anything verifies that a downloaded artifact executes on
/// the platform it was selected for. Asserting the *version* rather than merely exiting 0
/// is what catches a url and a `version` field that disagree — the one error this generator
/// is most able to make on its own.
#[test]
fn the_formula_tests_the_binary_it_installs() {
    let f = formula("1.2.3");
    let test_block = f
        .split("test do")
        .nth(1)
        .expect("the formula has no `test do` block");
    assert!(
        test_block.contains("--version"),
        "the test block must run the binary:\n{test_block}"
    );
    assert!(
        test_block.contains("assert_match version.to_s"),
        "the test block must assert on the version, not merely on exit status:\n{test_block}"
    );
}

/// The tap job must not run before the assets it points at exist.
///
/// The formula pins checksums of release assets by url. Rendered concurrently with the
/// publish, it is correct in every visible respect and 404s for everyone.
#[test]
fn the_tap_job_waits_for_the_publish() {
    let w = std::fs::read_to_string(root().join(".github/workflows/release.yml")).unwrap();
    let workflow: serde_yaml::Value = serde_yaml::from_str(&w).expect("release.yml parses");
    let tap = &workflow["jobs"]["tap"];
    assert!(!tap.is_null(), "release.yml no longer updates the tap");
    let needs = &tap["needs"];
    assert!(
        needs.as_str() == Some("publish")
            || needs
                .as_sequence()
                .is_some_and(|s| s.iter().any(|n| n.as_str() == Some("publish"))),
        "the tap job must depend on publish, got: {needs:?}"
    );
    assert!(
        tap["if"]
            .as_str()
            .is_some_and(|c| c.contains("refs/tags/cli/v")),
        "the tap job must be gated on a CLI tag; workflow_dispatch publishes nothing"
    );
}

/// A missing tap token must fail the job, not skip it.
///
/// The tempting shape is a conditional skip. That leaves the tap serving the previous
/// version while the README, the docs, and the release notes all say `brew install` gets
/// you the new one — and nothing anywhere is red. By the time this job runs the release is
/// already published, so failing costs nothing that was not already delivered.
#[test]
fn an_absent_tap_token_fails_loudly() {
    let w = std::fs::read_to_string(root().join(".github/workflows/release.yml")).unwrap();
    let workflow: serde_yaml::Value = serde_yaml::from_str(&w).expect("release.yml parses");
    let steps = workflow["jobs"]["tap"]["steps"]
        .as_sequence()
        .expect("the tap job has steps");
    let push = steps
        .iter()
        .find(|s| {
            s["run"]
                .as_str()
                .is_some_and(|r| r.contains("HOMEBREW_TAP_TOKEN") || r.contains("TAP_TOKEN"))
        })
        .expect("no step pushes to the tap");
    let run = push["run"].as_str().unwrap();
    let guard = run
        .lines()
        .position(|l| l.contains("-z \"${TAP_TOKEN:-}\""))
        .expect("the push step does not check whether the token is present");
    // `l.trim() == "fi"` and not `contains` — the branch's own message mentions a
    // "fine-grained PAT", and a substring match ends the branch on that line.
    let exits: Vec<&str> = run
        .lines()
        .skip(guard)
        .take_while(|l| l.trim() != "fi")
        .collect();
    assert!(
        exits.iter().any(|l| l.trim() == "exit 1"),
        "the no-token branch must fail: {exits:?}"
    );
    assert!(
        exits.iter().any(|l| l.contains("HOMEBREW_TAP_TOKEN")),
        "the failure must name the secret to create: {exits:?}"
    );
}
