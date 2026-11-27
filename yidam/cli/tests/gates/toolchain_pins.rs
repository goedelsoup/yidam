//! One toolchain, stated once, and every departure from it says why.
//!
//! Before #463 this repository built under three. `mise.toml` pinned 1.88.0, every
//! `rust-toolchain.toml` agreed with it by a comment asking someone to remember, six workflow
//! steps used `dtolnay/rust-toolchain@stable`, and both `rust-version` fields read 1.85 —
//! under a comment claiming they matched the pin. Nothing compared any of them.
//!
//! #1632 moved the statement. #463 made the copies agree; they were still copies — `mise.toml`,
//! a harness `rust-toolchain.toml`, eight workflow lines — and a bare `cargo` at the root
//! obeyed none of them, building with whatever rustup's default was. The pin is now the root
//! `rust-toolchain.toml`, which rustup reads from any directory beneath it, mise reads through
//! `idiomatic_version_file_enable_tools`, and the workflows read with `sed`. What cannot read
//! it keeps a copy, and each copy has a test here: a Dockerfile's `FROM`, the source-build task
//! in `mise.yidam.toml` (it runs in a derived repository, which has no such file), and the two
//! `rust-version` fields.
//!
//! `rust-version` is the sharp one. It is the floor a crates.io consumer is promised, no gate
//! ever compiled it, and it sits on a published library. An unverified floor is not a lower
//! floor; it is a claim that can stop being true at any dependency bump with nothing going red.
//!
//! **`@stable` is not banned, it is accounted for.** The `install-channels.yml` steps simulate
//! a user, who has whatever stable is, and pinning them would test a channel nobody uses.
//! `cargo-semver-checks` needs rustc >= 1.93 to read rustdoc JSON and builds nothing that
//! ships. Each of those is a decision, and the rule here is that a decision is written where it
//! is made. A bare `@stable` with nothing above it is the accident.
//!
//! Every side is discovered. A roster in this file would stop covering the next workflow.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::common;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()))
}

fn table(rel_path: &str) -> toml::Table {
    read(&repo_root().join(rel_path))
        .parse()
        .unwrap_or_else(|e| panic!("{rel_path} is not TOML: {e}"))
}

/// The one pin: `[toolchain] channel` in the root `rust-toolchain.toml`.
fn pin() -> String {
    table("rust-toolchain.toml")["toolchain"]
        .get("channel")
        .and_then(toml::Value::as_str)
        .expect("rust-toolchain.toml has no `[toolchain] channel`")
        .to_string()
}

/// What the workflows' `sed -n 's/^channel = "\(.*\)"$/\1/p'` prints for a file's text.
///
/// The shell reader is a line match, not a TOML parser, so the file can say the same thing in
/// a form `sed` prints nothing for — `channel="1.88.0"`, a trailing comment — and every
/// workflow would then run `rustup toolchain install ""`. This is that reader, so the test
/// below can hold the file to the form it understands.
fn sed_channel(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("channel = \"")?.strip_suffix('"'))
        .map(str::to_string)
        .collect()
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

fn workflows() -> Vec<PathBuf> {
    files_named(".github", |p| p.extension().is_some_and(|e| e == "yml"))
}

fn rel(p: &std::path::Path) -> String {
    p.strip_prefix(repo_root())
        .unwrap_or(p)
        .display()
        .to_string()
}

/// The pin is a real toolchain version, not a channel.
///
/// `channel = "stable"` would satisfy every comparison below by making them all vacuously
/// equal, and would reintroduce exactly what this file exists to prevent.
#[test]
fn the_pin_is_an_exact_version() {
    let pin = pin();
    let parts: Vec<&str> = pin.split('.').collect();
    assert!(
        parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()),
        "rust-toolchain.toml pins `{pin}`, which is a channel rather than a version. Every \
         other assertion in this file would then compare two moving targets and pass."
    );
}

/// The workflows' `sed` reads the same channel TOML does, from exactly one line.
#[test]
fn the_workflows_reader_finds_the_pin() {
    assert_eq!(sed_channel("channel = \"1.88.0\"\n"), ["1.88.0"]);
    assert!(sed_channel("channel=\"1.88.0\"").is_empty());
    assert!(sed_channel("channel = \"1.88.0\" # pinned").is_empty());

    let text = read(&repo_root().join("rust-toolchain.toml"));
    let found = sed_channel(&text);
    assert_eq!(
        found,
        [pin()],
        "the workflows read the toolchain with `sed -n 's/^channel = \"\\(.*\\)\"$/\\1/p'`, \
         and against rust-toolchain.toml that prints {found:?}. Keep `channel = \"x.y.z\"` on \
         one line of its own, or every `rustup toolchain install` in .github/ gets the wrong \
         argument."
    );
}

/// The root `rust-toolchain.toml` is the only one.
///
/// rustup takes the nearest file walking up from where `cargo` runs, so a second one beneath
/// the root wins inside its directory. That was `yidam/tests/harness/rust-toolchain.toml`,
/// agreeing with `mise.toml` by a comment. Agreement by a comment is what this file replaces.
#[test]
fn the_root_rust_toolchain_file_is_the_only_one() {
    let root = repo_root().join("rust-toolchain.toml");
    assert!(
        root.is_file(),
        "no rust-toolchain.toml at the repository root, so a bare `cargo` builds with \
         rustup's default and mise has nothing to read"
    );
    let others: Vec<String> = files_named("", |p| {
        p.file_name()
            .is_some_and(|n| n == "rust-toolchain.toml" || n == "rust-toolchain")
    })
    .into_iter()
    .filter(|p| p.canonicalize().ok() != root.canonicalize().ok())
    .map(|p| rel(&p))
    .collect();
    assert!(
        others.is_empty(),
        "a second toolchain file wins over the root one inside its own directory, so which \
         compiler a contributor gets depends on where they stand: {others:?}"
    );
}

/// mise reads the file and does not restate it.
///
/// A `rust` entry in `[tools]` wins over the version file, so it would be a second statement
/// that silently decides. Without `rust` in `idiomatic_version_file_enable_tools`, mise ignores
/// the file — that setting is off by default — and `mise exec` falls through to rustup's default.
#[test]
fn mise_reads_the_pin_rather_than_stating_one() {
    let mise = table("mise.toml");
    let tools = mise
        .get("tools")
        .and_then(toml::Value::as_table)
        .expect("mise.toml has no [tools]");
    assert!(
        !tools.contains_key("rust"),
        "mise.toml declares `rust` in [tools]. That wins over rust-toolchain.toml and is a \
         second place the toolchain is stated; delete it."
    );
    let enabled = mise
        .get("settings")
        .and_then(|s| s.get("idiomatic_version_file_enable_tools"))
        .and_then(toml::Value::as_array)
        .expect(
            "mise.toml sets no `[settings] idiomatic_version_file_enable_tools`, so mise does \
             not read rust-toolchain.toml at all",
        );
    assert!(
        enabled.iter().any(|v| v.as_str() == Some("rust")),
        "`idiomatic_version_file_enable_tools` does not list `rust`, so mise ignores \
         rust-toolchain.toml: {enabled:?}"
    );
}

/// Every `rust-version` is the pinned toolchain.
///
/// A floor *below* the build pin is a coherent thing to want, and it is not what this
/// repository has: nothing ever compiled 1.85, so the promise on a published crate was
/// unverified. Raising it to the pin makes every existing gate the verification. Changing
/// that back is a decision — it needs an MSRV job to go with it, and this test is where it
/// will be argued. A floor *above* the pin is never coherent: no gate here could build it.
#[test]
fn every_msrv_is_the_pinned_toolchain() {
    let pin = pin();
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
            "{} declares MSRV {declared} and rust-toolchain.toml pins {pin}. Above the pin, \
             nothing here can build it; below, nothing builds with it, and a floor no gate \
             compiles is a promise to everyone installing from crates.io that can stop being \
             true without anything going red.",
            rel(f)
        );
    }
    assert!(
        found >= 2,
        "only {found} manifests declare rust-version; the two that must are yidam/cli and \
         the published SDK"
    );
}

/// No workflow states the version; each reads it.
///
/// Before #1632 eight workflow lines carried `1.88.0`, held to the pin by this file. Held
/// copies are still copies: every bump was nine edits. Reading the file makes the bump one.
#[test]
fn no_workflow_states_the_toolchain_version() {
    let pin = pin();
    let mut wrong = Vec::new();
    for f in workflows() {
        for (i, line) in read(&f).lines().enumerate() {
            if line.contains(&pin) {
                wrong.push(format!("  {}:{}: {}", rel(&f), i + 1, line.trim()));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "these name `{pin}` rather than reading rust-toolchain.toml. A comment counts: it is \
         the line that goes stale first.\n{}",
        wrong.join("\n")
    );
}

/// Every `rustup toolchain install` reads the version from `rust-toolchain.toml`.
///
/// Both release paths must have one. Without it a job builds what is published with
/// whichever toolchain the runner image happened to ship as default.
#[test]
fn every_workflow_toolchain_install_reads_the_pin() {
    let mut reading = BTreeSet::new();
    let mut wrong = Vec::new();
    for f in workflows() {
        for (i, line) in read(&f).lines().enumerate() {
            if !line.contains("rustup toolchain install") {
                continue;
            }
            if line.contains("rust-toolchain.toml") {
                reading.insert(rel(&f));
            } else {
                wrong.push(format!("  {}:{}: {}", rel(&f), i + 1, line.trim()));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "these install a toolchain without reading rust-toolchain.toml:\n{}",
        wrong.join("\n")
    );
    for must in [
        ".github/workflows/release.yml",
        ".github/workflows/publish-crates.yml",
    ] {
        assert!(
            reading.contains(must),
            "{must} installs no toolchain read from rust-toolchain.toml, so what it publishes \
             is built by whatever the runner's default is. Found in: {reading:?}"
        );
    }
}

/// Every `dtolnay/rust-toolchain` is `@stable` and says why.
///
/// A version ref would be a second statement of the pin, and dependabot reads it as a number
/// to raise — #629 arrived with three steps rewritten to `@1.120.0`, a Rust that does not
/// exist. `@stable` is sometimes exactly right, and the difference between that and an
/// accident is whether anybody wrote it down.
#[test]
fn every_workflow_stable_toolchain_carries_its_reason() {
    let mut explained = 0;
    let mut wrong = Vec::new();

    for f in workflows() {
        let text = read(&f);
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some(spec) = line.split("dtolnay/rust-toolchain@").nth(1) else {
                continue;
            };
            let spec = spec.trim();
            if spec != "stable" {
                wrong.push(format!(
                    "  {}:{}: `@{spec}`. Read the version from rust-toolchain.toml with \
                     `rustup toolchain install`, as the other pinned steps do.",
                    rel(&f),
                    i + 1
                ));
                continue;
            }
            // The contiguous comment block immediately above, if any.
            let reason = lines[..i]
                .iter()
                .rev()
                .take_while(|l| l.trim_start().starts_with('#'))
                .count();
            if reason == 0 {
                wrong.push(format!(
                    "  {}:{}: bare `@stable` with no comment. It may well be right — say so, \
                     the way the others do.",
                    rel(&f),
                    i + 1
                ));
            } else {
                explained += 1;
            }
        }
    }

    assert!(
        explained >= 3,
        "only {explained} `@stable` steps carry a reason; install-channels.yml has three that \
         must, because they simulate a user rather than build an artifact"
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The Rust version in a `FROM rust:<tag>` line: the tag up to its first `-`.
///
/// `rust:1.88.0-bookworm` gives `1.88.0` and `rust:1.88-bookworm` gives `1.88`, which is not
/// the pin, because that tag floats to whichever patch the registry holds today.
fn rust_base_version(line: &str) -> Option<String> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("FROM") {
        return None;
    }
    // `--platform=…` and other flags come before the image.
    let image = words.find(|w| !w.starts_with("--"))?;
    let tag = image
        .strip_prefix("rust:")
        .or_else(|| image.strip_prefix("docker.io/library/rust:"))?;
    Some(tag.split('-').next().unwrap_or(tag).to_string())
}

/// Every Dockerfile's Rust base is the pinned toolchain.
///
/// #1227. The cluster Dockerfile built from `rust:1.88-bookworm`, which agreed with the pin
/// by coincidence and floated to each new 1.88 patch. Nothing compared the two until the
/// image was published, and a published image built by a compiler no gate ran is the thing
/// `release.yml`'s pinned toolchain exists to prevent. A `FROM` cannot read a file, so this
/// is a copy, and this test is what holds it.
///
/// Every Dockerfile, discovered, for the reason at the top of this file.
#[test]
fn every_dockerfile_rust_base_is_the_pinned_toolchain() {
    // The parser first, so a reader that matches nothing cannot pass by reading nothing.
    assert_eq!(
        rust_base_version("FROM rust:1.88.0-bookworm AS build").as_deref(),
        Some("1.88.0")
    );
    assert_eq!(
        rust_base_version("FROM --platform=$BUILDPLATFORM rust:1.88-slim").as_deref(),
        Some("1.88")
    );
    assert_eq!(
        rust_base_version("from rust:1.90.0").as_deref(),
        Some("1.90.0")
    );
    assert_eq!(rust_base_version("FROM debian:bookworm-slim"), None);
    assert_eq!(rust_base_version("# FROM rust:1.0.0"), None);

    let pin = pin();
    let files = files_named("", |p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "Dockerfile" || n.ends_with(".Dockerfile"))
    });
    let mut found = 0;
    let mut wrong = Vec::new();
    for f in &files {
        for (i, line) in read(f).lines().enumerate() {
            let Some(version) = rust_base_version(line) else {
                continue;
            };
            found += 1;
            if version != pin {
                wrong.push(format!(
                    "  {}:{}: builds from rust `{version}`, and rust-toolchain.toml pins `{pin}`",
                    rel(f),
                    i + 1
                ));
            }
        }
    }
    assert!(
        found >= 1,
        "no Dockerfile builds from a `rust:` base. yidam/cluster/Dockerfile does, so this test \
         is reading the wrong tree or the parser stopped matching"
    );
    assert!(
        wrong.is_empty(),
        "these images build with a toolchain other than the one pin, and the cluster image is \
         published:\n{}",
        wrong.join("\n")
    );
}

/// The derived repository's source build compiles with the pinned toolchain.
///
/// `mise.yidam.toml` runs in a derived repository, and `rust-toolchain.toml` is in
/// `NOT_INHERITED`, so the task cannot read the file and keeps a copy. A copy that drifted
/// would build a derived repository's `.yidam/bin/yidam` with a compiler no gate here ran.
#[test]
fn the_derived_source_build_uses_the_pinned_toolchain() {
    let pin = pin();
    let tasks = table("mise.yidam.toml");
    let mut found = 0;
    let mut wrong = Vec::new();
    for (name, task) in &tasks {
        let Some(rust) = task.get("tools").and_then(|t| t.get("rust")) else {
            continue;
        };
        found += 1;
        let version = rust
            .as_str()
            .or_else(|| rust.get("version").and_then(toml::Value::as_str));
        if version != Some(pin.as_str()) {
            wrong.push(format!("  [{name}] tools.rust = {rust}"));
        }
    }
    assert!(
        found >= 1,
        "no task in mise.yidam.toml declares `tools.rust`. yidam-build-source does, so this \
         test is reading the wrong file or the task stopped provisioning its compiler"
    );
    assert!(
        wrong.is_empty(),
        "rust-toolchain.toml pins `{pin}`, and these derived-repository tasks build with \
         something else:\n{}",
        wrong.join("\n")
    );
}

/// Every cargo workspace commits its lockfile.
///
/// Two of three did not, hidden by a `Cargo.lock` entry in a global gitignore, and nobody
/// chose that. It cost `--locked` on the summary step #462 added, and it meant `cargo deny`
/// would have audited a dependency resolution that existed only on the runner.
#[test]
fn every_workspace_commits_its_lockfile() {
    let tracked: BTreeSet<String> =
        common::git::out(&repo_root(), &["ls-files", "--", "*Cargo.lock"])
            .lines()
            .map(|l| l.trim().to_string())
            .collect();

    // A workspace root is a Cargo.toml with a [workspace] table, or one cargo builds alone.
    let roots = ["yidam/cli", "yidam/sdks/rust", "yidam/tests/harness"];
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
