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

/// The same fixture checksums, written as the single combined `SHA256SUMS` the release
/// uploads as an asset. Returns the path to the file itself, which is what the generator is
/// given — the tap renders from the release rather than from a directory of run artifacts.
fn combined(dir: &Path, version: &str, targets: &[&str]) -> PathBuf {
    let mut body = String::new();
    for (n, t) in targets.iter().enumerate() {
        let hash = format!("{}", n + 1).repeat(64);
        body.push_str(&format!("{hash}  yidam-{version}-{t}.tar.gz\n"));
    }
    let path = dir.join("SHA256SUMS");
    std::fs::write(&path, body).unwrap();
    path
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
///
/// The secret is also declared optional on `workflow_call` for the sake of this message. A
/// required one the caller cannot supply fails the job before its first step, with GitHub's
/// wording about an unset input, and the branch below — which names the PAT, its scope, and
/// the repository it needs — never runs.
#[test]
fn an_absent_tap_token_fails_loudly() {
    let w = std::fs::read_to_string(root().join(".github/workflows/tap.yml")).unwrap();
    let workflow: serde_yaml::Value = serde_yaml::from_str(&w).expect("tap.yml parses");
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

/// `SHA256SUMS` and the directory of per-asset files must render the identical formula.
///
/// They are the same checksums in two packagings, and the tap now reads the combined one
/// because a release keeps its assets forever while a run's artifacts expire. That only
/// helps if the two agree: a formula that differs by the road its hashes travelled is a
/// formula whose correctness depends on which entrance the tap workflow was called from.
#[test]
fn a_combined_sha256sums_renders_the_same_formula_as_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    checksums(dir.path(), "1.2.3", &TARGETS);
    let from_dir = render_in(dir.path(), "1.2.3");
    assert!(
        from_dir.status.success(),
        "directory render failed: {}",
        String::from_utf8_lossy(&from_dir.stderr)
    );

    let other = tempfile::tempdir().unwrap();
    let sums = combined(other.path(), "1.2.3", &TARGETS);
    let from_file = render_in(&sums, "1.2.3");
    assert!(
        from_file.status.success(),
        "SHA256SUMS render failed: {}",
        String::from_utf8_lossy(&from_file.stderr)
    );

    assert_eq!(
        String::from_utf8_lossy(&from_dir.stdout),
        String::from_utf8_lossy(&from_file.stdout),
        "the two checksum packagings render different formulae"
    );
}

/// A platform missing from `SHA256SUMS` is a hard failure, exactly as it is for a directory.
///
/// The combined file is the form a repair renders from, months after the release. If a
/// truncated or partially-downloaded manifest merely omitted an `on_arm` block, the repair
/// would push a formula that installs for most people and reports "no available formula" to
/// the rest — a worse state than the stale tap it was fixing.
#[test]
fn a_platform_missing_from_the_combined_file_is_a_hard_failure() {
    let dir = tempfile::tempdir().unwrap();
    let sums = combined(dir.path(), "1.2.3", &TARGETS[1..]);
    let out = render_in(&sums, "1.2.3");
    assert!(
        !out.status.success(),
        "rendered a formula from a SHA256SUMS with a platform missing:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains(TARGETS[0]),
        "the failure must name the platform it lacks: {err}"
    );
}

/// The asset name in `SHA256SUMS` is matched whole, not as a substring.
///
/// The manifest covers everything a release uploads, and a release may one day upload more
/// than four tarballs — a signature, a `.zip`, a differently-suffixed target. A substring
/// match would take the first line that merely *contains* the asset name, pairing a real url
/// with some other file's hash. That formula is valid Ruby pointing at a real download, and
/// it fails verification on the user's machine as a corrupted archive.
#[test]
fn the_combined_file_matches_the_asset_name_whole() {
    let dir = tempfile::tempdir().unwrap();
    let sums = dir.path().join("SHA256SUMS");
    let decoy = "d".repeat(64);
    let mut body = String::new();
    // First, so a substring matcher stopping at its first hit takes this one.
    body.push_str(&format!("{decoy}  yidam-1.2.3-{}.tar.gz.sig\n", TARGETS[0]));
    for (n, t) in TARGETS.iter().enumerate() {
        body.push_str(&format!(
            "{}  yidam-1.2.3-{t}.tar.gz\n",
            format!("{}", n + 1).repeat(64)
        ));
    }
    std::fs::write(&sums, body).unwrap();

    let out = render_in(&sums, "1.2.3");
    assert!(
        out.status.success(),
        "extra assets in SHA256SUMS must be inert: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let f = String::from_utf8(out.stdout).unwrap();
    assert!(
        !f.contains(&decoy),
        "a `.sig` line supplied the hash for {}:\n{f}",
        TARGETS[0]
    );
    assert!(
        f.contains(&"1".repeat(64)),
        "the tarball's own hash is not in the formula:\n{f}"
    );
}

/// The tap must be pushable for a release that is already out.
///
/// It was reachable only by pushing a `cli/v*` tag, and its checksums came from the
/// artifacts of that run. So when the push failed on `cli/v0.2.1` the only repair was
/// re-running that job — which stops working once the run's artifacts age out, after which
/// the tap could be corrected only by editing the formula by hand. That is the failure the
/// generated formula exists to prevent, reached from the other side.
///
/// Two properties keep the repair available: a person can start it, and it reads the
/// release's own assets rather than a run's.
#[test]
fn the_tap_can_be_pushed_for_a_release_that_is_already_out() {
    let text = std::fs::read_to_string(root().join(".github/workflows/tap.yml"))
        .expect(".github/workflows/tap.yml is unreadable");
    assert!(
        text.contains("workflow_dispatch:"),
        "tap.yml is not dispatchable, so a failed push can only be repaired by re-running \
         the release that failed"
    );
    assert!(
        text.contains("workflow_call:"),
        "tap.yml is not callable, so the release cannot use the same steps a repair does"
    );
    assert!(
        !text.contains("download-artifact"),
        "tap.yml reads the run's artifacts; those expire, and a formula must stay \
         renderable for a release tagged months ago"
    );
    assert!(
        text.contains("gh release download") && text.contains("SHA256SUMS"),
        "tap.yml must take its checksums from the published release's SHA256SUMS"
    );

    // And the release delegates to it rather than carrying a second copy of the steps.
    let w = std::fs::read_to_string(root().join(".github/workflows/release.yml")).unwrap();
    let release: serde_yaml::Value = serde_yaml::from_str(&w).expect("release.yml parses");
    assert_eq!(
        release["jobs"]["tap"]["uses"].as_str(),
        Some("./.github/workflows/tap.yml"),
        "release.yml no longer calls tap.yml; a recovery path that is a separate \
         implementation is a second thing to be wrong"
    );
}

/// The tap must refuse a tag that is not the latest release.
///
/// The repair path is "dispatch it with a tag", and the tap serves exactly one formula, so a
/// mistyped older tag is a downgrade that reports success. The same is true of the repair
/// this file's neighbour describes: re-running an old release's tap job after a newer
/// release shipped would quietly put the tap behind again — which is the state #246 is.
#[test]
fn the_tap_refuses_a_tag_that_is_not_the_latest_release() {
    let text = std::fs::read_to_string(root().join(".github/workflows/tap.yml")).unwrap();
    let workflow: serde_yaml::Value = serde_yaml::from_str(&text).expect("tap.yml parses");
    let steps = workflow["jobs"]["tap"]["steps"]
        .as_sequence()
        .expect("the tap job has steps");
    let guard = steps
        .iter()
        .find_map(|s| s["run"].as_str())
        .filter(|r| r.contains("releases/latest"))
        .expect("no step compares the requested tag against the latest release");
    assert!(
        guard.contains("exit 1"),
        "the latest-release check must fail the job: {guard}"
    );
    // Before the push, or it is a check on something already done.
    let latest = steps
        .iter()
        .position(|s| {
            s["run"]
                .as_str()
                .is_some_and(|r| r.contains("releases/latest"))
        })
        .unwrap();
    let push = steps
        .iter()
        .position(|s| s["run"].as_str().is_some_and(|r| r.contains("TAP_TOKEN")))
        .expect("no step pushes to the tap");
    assert!(latest < push, "the tap is pushed before the tag is checked");
}
