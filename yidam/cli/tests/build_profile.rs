//! What the test binaries link, and the local task that links it too.
//!
//! #923: `ci (cli · full features)` was 22m21s of a 23m31s main run, with every other job
//! under 6m. The cost is not compiling dependencies — `Swatinem/rust-cache` restores that
//! tree — it is codegen and link of the ~100 integration test binaries under `tests/`, done
//! twice by `ci-cli-full` and again by `coverage-full`. rust-cache prunes workspace artifacts
//! before saving, so those links happen on every run, and each binary statically absorbs the
//! DWARF of whatever it reaches: lance, datafusion, arrow, ort.
//!
//! [`profiles`] is the setting that removes that DWARF, and the argument for it is in
//! `yidam/cli/Cargo.toml` next to the table. What is gated here is the part a comment cannot
//! hold: that **every** workspace CI links test binaries from carries it, that nothing in the
//! workflows or the task layer quietly hands the debug info back, and that the local task the
//! setting exists to make usable runs the command CI runs rather than a drifting copy of it.
//!
//! The second is the real risk. A `RUSTFLAGS: -C debuginfo=2` or a `CARGO_PROFILE_DEV_DEBUG`
//! in a workflow `env:` wins over the manifest, costs nothing visible at review time, and
//! would put 1.3GB of DWARF back into every link with the table still sitting there saying
//! it was removed.

use std::collections::BTreeMap;
use std::path::PathBuf;

mod common;
use common::repo_root;

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

fn mise() -> toml::Table {
    read("mise.toml").parse().expect("mise.toml is not TOML")
}

/// The commands of a task, whether `run` is one string, a list, or absent — a task that only
/// declares `depends` runs nothing of its own and is not an error to ask about.
fn commands(task: &toml::Value) -> Vec<String> {
    match task.get("run") {
        Some(toml::Value::String(s)) => vec![s.clone()],
        Some(toml::Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// The commands of the named mise task, which must have some.
fn task_run(mise: &toml::Table, name: &str) -> Vec<String> {
    let task = mise
        .get("tasks")
        .and_then(|t| t.get(name))
        .unwrap_or_else(|| panic!("mise.toml has no task `{name}`"));
    let run = commands(task);
    assert!(!run.is_empty(), "task `{name}` has no `run`");
    run
}

/// The argument of `--manifest-path` in a command line, if it has one.
fn manifest_path(command: &str) -> Option<String> {
    let mut words = command.split_whitespace();
    while let Some(w) = words.next() {
        if w == "--manifest-path" {
            return words.next().map(str::to_string);
        }
        if let Some(rest) = w.strip_prefix("--manifest-path=") {
            return Some(rest.to_string());
        }
    }
    None
}

/// Every manifest some mise task builds test binaries from, and the tasks that do it.
///
/// Discovered rather than listed: a third workspace, or a second task pointed at an existing
/// one, arrives here without anyone remembering this file. `cargo test --doc` is not a link,
/// and `clippy` and `fmt` are not either — only `nextest run` and `cargo test` produce the
/// binaries this is about.
fn linking_manifests() -> BTreeMap<String, Vec<String>> {
    let mise = mise();
    let tasks = mise
        .get("tasks")
        .and_then(toml::Value::as_table)
        .expect("mise.toml declares no [tasks]");

    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, task) in tasks {
        for command in commands(task) {
            let links = (command.contains("cargo nextest run")
                || (command.contains("cargo test") && !command.contains("--doc")))
                && !command.contains("cargo llvm-cov");
            if !links {
                continue;
            }
            let Some(path) = manifest_path(&command) else {
                continue;
            };
            // Template content: `yidam/prelude/` is vendored into every derived repository,
            // and a profile written there would be a build decision imposed on someone
            // else's corpus rather than one about this repository's CI.
            if path.starts_with("yidam/prelude/") || !repo_root().join(&path).exists() {
                continue;
            }
            found.entry(path).or_default().push(name.clone());
        }
    }
    found
}

/// `debug` under `[profile.dev.package."*"]`, as the manifest at `rel` sets it.
fn dependency_debug(rel: &str) -> Option<toml::Value> {
    let manifest: toml::Table = read(rel).parse().unwrap_or_else(|e| panic!("{rel}: {e}"));
    manifest
        .get("profile")?
        .get("dev")?
        .get("package")?
        .get("*")?
        .get("debug")
        .cloned()
}

/// A `debug` value that keeps DWARF out of the link. `1` does not: it is full line tables
/// plus types, and on Linux it is most of the bytes.
fn is_off(debug: &toml::Value) -> bool {
    match debug {
        toml::Value::Integer(0) => true,
        toml::Value::Boolean(false) => true,
        toml::Value::String(s) => s == "none" || s == "line-tables-only",
        _ => false,
    }
}

/// Every workspace whose test binaries CI links builds its dependencies without debug info.
///
/// The set comes from `mise.toml`, not from a list here, because the defect this prevents is
/// a *new* link site — a third workspace, or a task pointed at an existing manifest — quietly
/// costing what #923 measured while this file stays green about two manifests it names.
#[test]
fn every_workspace_that_links_tests_drops_dependency_debug_info() {
    let manifests = linking_manifests();

    // An empty map and a clean repository read the same, and the discovery is three brittle
    // string matches deep. `yidam/cli` is the workspace this crate's own tests live in, so
    // if it is missing the scan is broken, not the repository.
    assert!(
        manifests.contains_key("yidam/cli/Cargo.toml"),
        "the scan found {} linking manifests and `yidam/cli/Cargo.toml` is not among them, \
         which is this crate's own — the discovery is reading the wrong thing: {:?}",
        manifests.len(),
        manifests.keys().collect::<Vec<_>>()
    );

    let mut bare = Vec::new();
    for (path, tasks) in &manifests {
        match dependency_debug(path) {
            Some(d) if is_off(&d) => {}
            found => bare.push(format!(
                "{path} (linked by {}) sets {}",
                tasks.join(", "),
                match found {
                    Some(d) => format!("debug = {d}"),
                    None => "no `debug` under [profile.dev.package.\"*\"]".to_string(),
                }
            )),
        }
    }

    assert!(
        bare.is_empty(),
        "these workspaces link test binaries with dependency debug info on:\n  {}\n\nEach \
         binary then carries the DWARF of every dependency it reaches. Measured on \
         yidam/cli under --all-features: 2,708MB of rlibs against 1,402MB without it, \
         copied once per binary, ~100 binaries per feature set. Add to the workspace \
         root manifest:\n\n    [profile.dev.package.\"*\"]\n    debug = 0\n\n`\"*\"` \
         matches dependencies only, so the crate being tested keeps its own line numbers.",
        bare.join("\n  ")
    );
}

/// A line that would give dependency debug info back over the manifest's head.
fn re_enables_debug(line: &str) -> bool {
    let line = line.trim();
    if line.starts_with('#') {
        return false;
    }
    let profile_env = line.contains("CARGO_PROFILE_")
        && (line.contains("_DEBUG") || line.contains("_PACKAGE_"))
        && !line.contains("_DEBUG_ASSERTIONS");
    let rustflags = (line.contains("RUSTFLAGS") || line.contains("rustflags"))
        && (line.contains("debuginfo") || line.contains("-Cdebuginfo"));
    profile_env || rustflags
}

/// Nothing in the workflows or the task layer overrides the manifest.
///
/// A `CARGO_PROFILE_DEV_PACKAGE_*_DEBUG=2`, or a `-C debuginfo=2` in `RUSTFLAGS`, beats
/// `Cargo.toml` and leaves the table in place looking like it still decides. This is the one
/// way the setting can be lost without anyone editing the manifest, so it is the one worth
/// a scan rather than a comment.
#[test]
fn nothing_in_ci_hands_dependency_debug_info_back() {
    // The rule before it is pointed at anything: two substrings that match nothing read
    // exactly like a clean repository.
    assert!(re_enables_debug("  CARGO_PROFILE_DEV_DEBUG: '2'"));
    assert!(re_enables_debug(
        "  CARGO_PROFILE_DEV_PACKAGE_lance_DEBUG: '2'"
    ));
    assert!(re_enables_debug("RUSTFLAGS = \"-C debuginfo=2\""));
    assert!(!re_enables_debug("  CARGO_BUILD_JOBS: '2'"));
    assert!(!re_enables_debug(
        "  CARGO_PROFILE_DEV_DEBUG_ASSERTIONS: 'true'"
    ));
    assert!(!re_enables_debug("#   CARGO_PROFILE_DEV_DEBUG: '2'"));

    let mut sources: Vec<(String, String)> = vec![("mise.toml".to_string(), read("mise.toml"))];
    let workflows = repo_root().join(".github/workflows");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&workflows)
        .expect(".github/workflows is unreadable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml" || e == "yaml"))
        .collect();
    entries.sort();
    for path in entries {
        let rel = format!(".github/workflows/{}", path.file_name().unwrap().display());
        sources.push((rel, std::fs::read_to_string(&path).expect("workflow")));
    }

    assert!(
        sources.len() > 3,
        "only {} files reached the scan; this repository has more workflows than that, so \
         an empty finding list says nothing",
        sources.len()
    );

    let offenders: Vec<String> = sources
        .iter()
        .flat_map(|(rel, text)| {
            text.lines()
                .enumerate()
                .filter(|(_, l)| re_enables_debug(l))
                .map(move |(i, l)| format!("{rel}:{}: {}", i + 1, l.trim()))
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "these override the dependency debug setting the CLI manifest makes:\n  {}\n\nAn \
         environment override wins over `[profile.dev.package.\"*\"]` and leaves the table \
         in the manifest still claiming to decide. If the debug info is wanted, delete the \
         table and its argument; do not shadow it.",
        offenders.join("\n  ")
    );
}

/// The local CLI test task runs the command CI runs.
///
/// #923: *"Locally there is no fast subset either — `[tasks.test]` runs the domain crates, so
/// a contributor's only CLI option is the full `ci-cli`."* `test-cli` is that subset, and the
/// way it goes wrong is not that it breaks but that it drifts: `ci-cli` gains a flag, and the
/// task everyone actually runs between two edits is now testing something else. Pinning it to
/// `ci-cli`'s own line makes drift a failure rather than a surprise in CI.
#[test]
fn the_local_cli_test_task_runs_the_command_ci_runs() {
    let mise = mise();

    let ci: Vec<String> = task_run(&mise, "ci-cli")
        .into_iter()
        .filter(|c| c.contains("cargo nextest run"))
        .collect();
    assert_eq!(
        ci.len(),
        1,
        "`ci-cli` runs {} nextest commands; `test-cli` mirrors one line and cannot mirror \
         several: {ci:?}",
        ci.len()
    );

    let local = task_run(&mise, "test-cli");
    assert_eq!(
        local.len(),
        1,
        "`test-cli` is one command a contributor can append a filter to; it runs {local:?}"
    );

    assert_eq!(
        local[0], ci[0],
        "`test-cli` and `ci-cli` no longer run the same test command. A contributor's \
         iteration loop is then not the gate, and the difference is found after the push."
    );
}
