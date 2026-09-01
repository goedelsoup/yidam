//! One toolchain, and every departure from it says why.
//!
//! Before #463 this repository built under three. `mise.toml` pinned 1.88.0, every
//! `rust-toolchain.toml` agreed with it by a comment asking someone to remember, six workflow
//! steps used `dtolnay/rust-toolchain@stable`, and both `rust-version` fields read 1.85 —
//! under a comment claiming they matched the pin. Nothing compared any of them.
//!
//! `rust-version` was the sharp one. It is the floor a crates.io consumer is promised, no gate
//! ever compiled it, and it sits on a published library. An unverified floor is not a lower
//! floor; it is a claim that can stop being true at any dependency bump with nothing going red.
//!
//! **`@stable` is not banned, it is accounted for.** Three of the six were right: the
//! `install-channels.yml` steps simulate a user, who has whatever stable is, and pinning them
//! would test a channel nobody uses. One more was added by this issue — `cargo-semver-checks`
//! needs rustc >= 1.93 to read rustdoc JSON and builds nothing that ships. Each of those is a
//! decision, and the rule here is that a decision is written where it is made. A bare
//! `@stable` with nothing above it is the accident.
//!
//! Every side is discovered. A roster in this file would stop covering the next workflow.

use std::collections::BTreeSet;
use std::path::PathBuf;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()))
}

/// The one pin: `rust = { version = "…" }` in `mise.toml`.
fn mise_pin() -> String {
    let toml = read(&repo_root().join("mise.toml"));
    let line = toml
        .lines()
        .find(|l| l.trim_start().starts_with("rust = "))
        .expect("mise.toml pins no `rust`");
    let after = line
        .split_once("version = \"")
        .expect("the rust pin has no version")
        .1;
    after
        .split_once('"')
        .expect("unterminated version")
        .0
        .to_string()
}

/// Files under a directory with one of the given names or extensions.
fn files_named(dir: &str, matches: impl Fn(&std::path::Path) -> bool) -> Vec<PathBuf> {
    WalkDir::new(repo_root().join(dir))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        // A vendored dependency's manifest is not this repository's pin to keep.
        .filter(|p| !p.components().any(|c| c.as_os_str() == "target"))
        .filter(|p| matches(p))
        .collect()
}

fn rel(p: &std::path::Path) -> String {
    p.strip_prefix(repo_root())
        .unwrap_or(p)
        .display()
        .to_string()
}

/// Every `rust-toolchain.toml` names the toolchain `mise.toml` pins.
///
/// These were kept in step by a comment asking a person to remember, which is the mechanism
/// that had `rust-version` reading 1.85 against a 1.88.0 pin for however long nobody looked.
#[test]
fn every_rust_toolchain_file_names_the_pinned_toolchain() {
    let pin = mise_pin();
    let files = files_named("", |p| {
        p.file_name().is_some_and(|n| n == "rust-toolchain.toml")
    });
    assert!(
        !files.is_empty(),
        "no rust-toolchain.toml anywhere; this test is looking at the wrong tree"
    );
    for f in files {
        let text = read(&f);
        let channel = text
            .lines()
            .find(|l| l.trim_start().starts_with("channel"))
            .and_then(|l| l.split('"').nth(1).map(str::to_string))
            .unwrap_or_else(|| panic!("{} declares no channel", rel(&f)));
        assert_eq!(
            channel,
            pin,
            "{} pins `{channel}` and mise.toml pins `{pin}`. Two toolchains build this \
             repository, and which one a contributor gets depends on where they stand.",
            rel(&f)
        );
    }
}

/// Every `rust-version` is the pinned toolchain.
///
/// A floor *below* the build pin is a coherent thing to want, and it is not what this
/// repository has: nothing ever compiled 1.85, so the promise on a published crate was
/// unverified. Raising it to the pin makes every existing gate the verification. Changing
/// that back is a decision — it needs an MSRV job to go with it, and this test is where it
/// will be argued.
#[test]
fn every_msrv_is_the_pinned_toolchain() {
    let pin = mise_pin();
    let manifests = files_named("yidam", |p| {
        p.file_name().is_some_and(|n| n == "Cargo.toml")
    });
    let mut found = 0;
    for f in &manifests {
        let text = read(f);
        let Some(line) = text
            .lines()
            .find(|l| l.trim_start().starts_with("rust-version"))
        else {
            continue;
        };
        found += 1;
        let declared = line
            .split('"')
            .nth(1)
            .unwrap_or_else(|| panic!("{}: unparseable rust-version", rel(f)));
        assert_eq!(
            declared,
            pin,
            "{} declares MSRV {declared} and nothing builds with it; mise.toml pins {pin}. \
             A floor no gate compiles is a promise to everyone installing from crates.io \
             that can stop being true without anything going red.",
            rel(f)
        );
    }
    assert!(
        found >= 2,
        "only {found} manifests declare rust-version; the two that must are yidam/cli and \
         the published SDK"
    );
}

/// Every workflow's Rust toolchain is the pin, or says why it is not.
///
/// The two halves are the whole of the rule. A pinned version must be *the* pin — a second
/// version is a second toolchain wearing a number. A `@stable` must carry a comment, because
/// `@stable` is sometimes exactly right (simulating a user, running an analyzer that needs a
/// newer rustc) and the difference between that and an accident is whether anybody wrote it
/// down.
#[test]
fn every_workflow_toolchain_is_the_pin_or_carries_its_reason() {
    let pin = mise_pin();
    let files = files_named(".github", |p| p.extension().is_some_and(|e| e == "yml"));
    let mut pinned = 0;
    let mut explained = 0;
    let mut wrong = Vec::new();

    for f in &files {
        let text = read(f);
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some(spec) = line.split("dtolnay/rust-toolchain@").nth(1) else {
                continue;
            };
            let spec = spec.trim();
            if spec == "stable" {
                // The contiguous comment block immediately above, if any.
                let reason: Vec<&str> = lines[..i]
                    .iter()
                    .rev()
                    .take_while(|l| l.trim_start().starts_with('#'))
                    .copied()
                    .collect();
                if reason.is_empty() {
                    wrong.push(format!(
                        "  {}:{}: bare `@stable` with no comment. It may well be right — \
                         say so, the way the others do.",
                        rel(f),
                        i + 1
                    ));
                } else {
                    explained += 1;
                }
            } else if spec != pin {
                wrong.push(format!(
                    "  {}:{}: pins `{spec}`, and mise.toml pins `{pin}`",
                    rel(f),
                    i + 1
                ));
            } else {
                pinned += 1;
            }
        }
    }

    assert!(
        pinned >= 3,
        "only {pinned} workflow steps use the pinned toolchain; either the release path went \
         back to `@stable` or this test is reading the wrong files"
    );
    assert!(
        explained >= 3,
        "only {explained} `@stable` steps carry a reason; install-channels.yml has three that \
         must, because they simulate a user rather than build an artifact"
    );
    assert!(
        wrong.is_empty(),
        "these do not agree with the one pin in mise.toml:\n{}",
        wrong.join("\n")
    );
}

/// The pin is a real toolchain version, not a channel.
///
/// `rust = { version = "stable" }` would satisfy every comparison above by making them all
/// vacuously equal, and would reintroduce exactly what this file exists to prevent.
#[test]
fn the_pin_is_an_exact_version() {
    let pin = mise_pin();
    let parts: Vec<&str> = pin.split('.').collect();
    assert!(
        parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()),
        "mise.toml pins `{pin}`, which is a channel rather than a version. Every other \
         assertion in this file would then compare two moving targets and pass."
    );
}

/// Every cargo workspace commits its lockfile.
///
/// Two of three did not, hidden by a `Cargo.lock` entry in a global gitignore, and nobody
/// chose that. It cost `--locked` on the summary step #462 added, and it meant `cargo deny`
/// would have audited a dependency resolution that existed only on the runner.
#[test]
fn every_workspace_commits_its_lockfile() {
    let out = std::process::Command::new("git")
        .current_dir(repo_root())
        .args(["ls-files", "--", "*Cargo.lock"])
        .output()
        .expect("git should be runnable");
    let tracked: BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .collect();

    // A workspace root is a Cargo.toml with a [workspace] table, or one cargo builds alone.
    let roots = [
        "yidam/cli",
        "yidam/prelude/sdks/rust",
        "yidam/tests/harness",
    ];
    let missing: Vec<&str> = roots
        .iter()
        .filter(|r| !tracked.contains(&format!("{r}/Cargo.lock")))
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "these workspaces have no committed Cargo.lock, so `--locked` cannot be passed \
         against them and `cargo deny` audits a resolution that exists only on the runner: \
         {missing:?}"
    );
}
