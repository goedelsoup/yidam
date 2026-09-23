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

mod common;

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
    // A vendored dependency's manifest is not this repository's pin to keep. That used to
    // be a hand-written `target` filter here — a ninth copy of the list #900 is about, and
    // one that hid from the guard because `target` is also an ordinary parameter name.
    common::repo_walk(&repo_root().join(dir))
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
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

/// An action whose ref *is* the pin is not a dependency dependabot may bump.
///
/// `dtolnay/rust-toolchain` publishes no version tags. `@1.88.0` is not a version of the
/// action — it is the compiler, and dependabot's `actions` group reads it as a number to
/// raise. #629 arrived with three steps rewritten to `@1.120.0`, a Rust that does not exist,
/// and the cross-compile job 404ed on the tarball.
///
/// The 404 is not the failure this guards. Two of those three steps were in `release.yml` and
/// `publish-crates.yml`, so a semver bump on that ref proposes a silent change to which
/// compiler builds the published artifact —
/// [`every_workflow_toolchain_is_the_pin_or_carries_its_reason`] is the only reason that was a
/// red gate rather than a release nobody chose the compiler for. That test catches the
/// proposal; the `ignore` in `.github/dependabot.yml` is what stops it being made again every
/// Monday, and #629 reverted three steps and left the proposal free to return.
///
/// **Discovered from the workflows, not a name written here.** Any action carrying the pin as
/// its ref has the same problem, so the set is every `uses:` whose ref is `mise.toml`'s
/// version. Swap `dtolnay/rust-toolchain` for another such action and the rule follows it.
#[test]
fn every_action_pinned_to_the_toolchain_is_ignored_by_dependabot() {
    let pin = mise_pin();

    // Actions whose ref is the toolchain version itself.
    let mut carriers: BTreeSet<String> = BTreeSet::new();
    for f in files_named(".github", |p| p.extension().is_some_and(|e| e == "yml")) {
        for line in read(&f).lines() {
            let Some((_, spec)) = line.split_once("uses:") else {
                continue;
            };
            let Some((action, reference)) = spec.trim().rsplit_once('@') else {
                continue;
            };
            if reference.trim() == pin {
                carriers.insert(action.trim().to_string());
            }
        }
    }
    assert!(
        !carriers.is_empty(),
        "no workflow pins an action to `{pin}`. Either the release path went back to a \
         channel — which is what the rest of this file is about — or this test is reading the \
         wrong tree."
    );

    // The `ignore` list of every github-actions entry in dependabot.yml, and whether each
    // entry is a whole-dependency ignore or a version-range one.
    let config: serde_yaml::Value =
        serde_yaml::from_str(&read(&repo_root().join(".github/dependabot.yml")))
            .expect(".github/dependabot.yml is not parseable YAML");
    let updates = config["updates"]
        .as_sequence()
        .expect("dependabot.yml declares no `updates`");
    let mut ecosystems = 0;
    // name → whether the ignore names `update-types`, i.e. leaves some bumps proposable.
    let mut ignored: Vec<(String, bool)> = Vec::new();
    for entry in updates {
        if entry["package-ecosystem"].as_str() != Some("github-actions") {
            continue;
        }
        ecosystems += 1;
        let empty = Vec::new();
        for rule in entry["ignore"].as_sequence().unwrap_or(&empty) {
            if let Some(name) = rule["dependency-name"].as_str() {
                ignored.push((name.to_string(), !rule["update-types"].is_null()));
            }
        }
    }
    assert!(
        ecosystems >= 1,
        "dependabot.yml has no `github-actions` entry, so nothing proposes action updates at \
         all and this test is asserting against an ecosystem that is not configured"
    );

    for action in &carriers {
        let rule = ignored.iter().find(|(name, _)| name == action);
        let Some((_, narrowed)) = rule else {
            panic!(
                "a workflow pins `{action}@{pin}` and `.github/dependabot.yml` does not \
                 ignore `{action}` under `github-actions`. That ref is the compiler, not a \
                 version of the action, so the `actions` group will propose a Rust release \
                 that does not exist — and the two steps it rewrites in release.yml and \
                 publish-crates.yml decide which compiler builds what ships."
            );
        };
        assert!(
            !narrowed,
            "`{action}` is ignored under `github-actions` only for some `update-types`. There \
             is no version of that ref dependabot can propose that means what it thinks it \
             means — a minor or patch bump names a Rust that does not exist exactly as a major \
             one does. Ignore the dependency whole."
        );
    }
}
