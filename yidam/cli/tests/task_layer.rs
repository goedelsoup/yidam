//! The inherited task layer must be loadable by the repositories that inherit it.
//!
//! `mise.yidam.toml` is shipped verbatim into every derived repository and pulled in by
//! `[task_config] includes`. **Nothing in this repository loads it** — yidam's own tasks live
//! in `mise.toml` — so it is exactly the artifact that can ship broken and stay broken, and
//! it did: it failed to parse at all, and every derived repo taking the update lost all 35
//! inherited tasks including `graph-check`, `regen`, `yidam-build` and `yidam-vendor-update`.
//!
//! Two defects put it there and both are shapes rather than typos, which is what makes them
//! worth a test rather than a proofreading pass:
//!
//! 1. **A comment with task definitions spliced into it.** The header opened a parenthetical
//!    at ``(`[rename]`` and never closed it, so `[rename]`'s table header was swallowed by
//!    the comment and its `description`/`run` became bare top-level keys, `[graph]` and
//!    `[neighbors]` became real tables inside what reads as prose, and the parenthetical
//!    closed on a second `[graph-check]` that duplicated the real one 300 lines down. Three
//!    commits had anchored edits on the comment's literal `[graph-check]`.
//! 2. **`[env]` is not a legal table in a task file.** mise reads it as a task *named* `env`
//!    and rejects `_.path` as an unknown field. The block was there to put `.yidam/bin` first
//!    on `PATH` — the guarantee the conventions cite by name — so the one file that promised
//!    it was the one file that could not deliver it.
//!
//! Both are caught by one rule: **every top-level value is a table, and every table is a
//! task.** A spliced comment leaves bare keys; `[env]` and `[tools]` leave tables that are
//! not tasks. Neither needs the checker to know what went wrong.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse(rel: &str) -> toml::Table {
    let path = repo_root().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
    text.parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("{rel} does not parse as TOML, so mise loads none of it:\n{e}"))
}

/// A task file's tables are tasks. Anything else is a config section in the wrong file.
///
/// mise does not reject these loudly — `[tools]` and `[env]` become tasks named `tools` and
/// `env`, and the failure surfaces as an unknown-field error that orphans the whole file.
const NOT_TASKS: &[&str] = &[
    "env",
    "tools",
    "settings",
    "plugins",
    "task_config",
    "alias",
];

#[test]
fn the_inherited_task_file_parses() {
    let tasks = parse("mise.yidam.toml");
    assert!(
        tasks.len() > 30,
        "only {} tables — the file parsed but lost most of itself",
        tasks.len()
    );
}

/// Every top-level value is a table, and every table has something to run.
///
/// This is the rule that catches a comment with tasks spliced into it: the swallowed table
/// header leaves `description` and `run` as bare top-level strings.
#[test]
fn every_top_level_entry_is_a_runnable_task() {
    let tasks = parse("mise.yidam.toml");
    let mut bare = Vec::new();
    let mut inert = Vec::new();
    for (name, value) in &tasks {
        let Some(table) = value.as_table() else {
            bare.push(name.clone());
            continue;
        };
        if !table.contains_key("run") && !table.contains_key("depends") {
            inert.push(name.clone());
        }
    }
    assert!(
        bare.is_empty(),
        "mise.yidam.toml has top-level keys that are not tables: {bare:?}\n\
         A bare `description`/`run` at the top level means a `[task]` header was swallowed — \
         usually by an unclosed parenthetical in the header comment."
    );
    assert!(
        inert.is_empty(),
        "mise.yidam.toml tables with neither `run` nor `depends`: {inert:?}\n\
         In a task file every table is a task. A config section here (`[env]`, `[tools]`) is \
         read as a task by that name and its fields rejected, which orphans the whole file."
    );
}

#[test]
fn the_task_file_holds_no_config_sections() {
    let tasks = parse("mise.yidam.toml");
    let found: Vec<&str> = NOT_TASKS
        .iter()
        .copied()
        .filter(|k| tasks.contains_key(*k))
        .collect();
    assert!(
        found.is_empty(),
        "mise.yidam.toml declares {found:?}, which belong in a mise *config* file. \
         `[env] _.path` in particular belongs in the derived repo's own mise.toml — see \
         sadhana/root/mise.toml."
    );
}

/// The `.yidam/bin` guarantee has to be declared somewhere a repository actually reads.
///
/// `directories.md` tells a reader that mise resolves the pinned binary first, and the CLI's
/// shadow warning is written on the assumption that mise-run commands are already correct.
/// Both were true of a declaration that no config file contained.
#[test]
fn the_derived_config_puts_the_pinned_binary_first() {
    let cfg = parse("sadhana/root/mise.toml");

    let includes = cfg
        .get("task_config")
        .and_then(|t| t.get("includes"))
        .and_then(|i| i.as_array())
        .expect("sadhana/root/mise.toml declares [task_config] includes");
    assert!(
        includes
            .iter()
            .any(|v| v.as_str() == Some("mise.yidam.toml")),
        "the derived config must include the inherited task layer: {includes:?}"
    );

    let path = cfg
        .get("env")
        .and_then(|e| e.get("_"))
        .and_then(|u| u.get("path"))
        .and_then(|p| p.as_array())
        .expect(
            "sadhana/root/mise.toml must declare [env] _.path — this is where the \
             `.yidam/bin` first guarantee lives, and it cannot live in the task file",
        );
    assert_eq!(
        path.first().and_then(|v| v.as_str()),
        Some(".yidam/bin"),
        "the pinned binary must come FIRST; anything ahead of it can answer instead"
    );
}

/// A task the derived CI invokes must exist in the layer that defines it.
///
/// `sadhana/root/mise.toml`'s `ci` task runs `mise run graph-check`, which comes from the
/// inherited file. When that file failed to parse, this was the command a derived repo's
/// gate reported as unknown.
#[test]
fn tasks_the_derived_config_invokes_are_defined() {
    let inherited: BTreeSet<String> = parse("mise.yidam.toml").keys().cloned().collect();
    let cfg = parse("sadhana/root/mise.toml");
    let own: BTreeSet<String> = cfg
        .get("tasks")
        .and_then(|t| t.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    let mut missing = Vec::new();
    for (name, task) in cfg.get("tasks").and_then(|t| t.as_table()).unwrap() {
        let run = task.get("run");
        let lines: Vec<String> = match run {
            Some(toml::Value::String(s)) => vec![s.clone()],
            Some(toml::Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => vec![],
        };
        for line in lines {
            let Some(rest) = line.trim().strip_prefix("mise run ") else {
                continue;
            };
            let called = rest.split_whitespace().next().unwrap_or_default();
            if !called.is_empty() && !inherited.contains(called) && !own.contains(called) {
                missing.push(format!(
                    "`{name}` runs `mise run {called}`, which is defined nowhere"
                ));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

/// The tasks a derived repository cannot function without.
///
/// Named rather than counted: a count drifts every time a subcommand is added, and the
/// failure this guards against was losing *all* of them at once.
#[test]
fn the_load_bearing_tasks_survive() {
    let tasks = parse("mise.yidam.toml");
    for required in [
        "yidam-build",
        "yidam-vendor-update",
        "yidam-vendor-status",
        "graph-check",
        "graph",
        "neighbors",
        "rename",
        "regen",
    ] {
        assert!(
            tasks.contains_key(required),
            "mise.yidam.toml no longer defines `{required}`"
        );
    }
}

/// Reinstalling must overwrite, not refuse.
///
/// `cargo install` exits 101 with "binary `yidam` already exists in destination" whenever the
/// install root holds a binary its own metadata does not account for. `.yidam/bin/` is
/// git-ignored and the metadata beside it is not, so they drift apart on a fresh clone. A
/// task whose whole purpose is to make the binary match the pin must not fail because a
/// binary is already there.
#[test]
fn both_build_tasks_force_the_install() {
    let inherited = parse("mise.yidam.toml");
    let derived_run = inherited["yidam-build"]["run"].as_str().unwrap();
    assert!(
        derived_run.contains("cargo install") && derived_run.contains("--force"),
        "mise.yidam.toml's yidam-build must pass --force:\n{derived_run}"
    );

    // The same lesson, in this repository's own copy — the destination a port forgets.
    let own = parse("mise.toml");
    let own_run = own["tasks"]["yidam-build"]["run"].as_str().unwrap();
    assert!(
        own_run.contains("--force"),
        "mise.toml's yidam-build must pass --force too:\n{own_run}"
    );
}

/// `yidam-build`'s tag resolver must match a pin against the *commit*, not the tag object.
///
/// The pin in `.yidam.toml` is a commit sha; a release is a tag. `yidam-build` reconciles
/// them with `git ls-remote --tags`, which prints two lines for an **annotated** tag:
///
/// ```text
/// <tag-object-sha>  refs/tags/cli/v0.2.0
/// <commit-sha>      refs/tags/cli/v0.2.0^{}
/// ```
///
/// `git tag -s` — the spelling VERSIONING.md's release process prescribes — makes annotated
/// tags. So a resolver that reads the bare line compares a commit against a tag object and
/// never matches, and the failure is invisible: it silently takes the source-build path,
/// forever, looking exactly like a pin with no release.
///
/// The awk program is read out of `mise.yidam.toml` rather than copied here, so the thing
/// under test is the thing that ships. A copy would pass while the file rotted.
#[test]
fn the_tag_resolver_matches_the_commit_and_not_the_tag_object() {
    let run = parse("mise.yidam.toml")["yidam-build"]["run"]
        .as_str()
        .expect("yidam-build.run")
        .to_string();

    let marker = "resolve_tag='";
    let start = run.find(marker).expect("yidam-build defines resolve_tag") + marker.len();
    let program = &run[start..start + run[start..].find('\'').expect("unterminated awk")];

    // Exactly what `git ls-remote --tags` prints: cli/v0.2.0 annotated (tag object `a…`,
    // commit `b…`), cli/v0.1.0 lightweight (`c…`).
    let a = "a".repeat(40);
    let b = "b".repeat(40);
    let c = "c".repeat(40);
    let d = "d".repeat(40);
    let listing = format!(
        "{a}\trefs/tags/cli/v0.2.0\n{b}\trefs/tags/cli/v0.2.0^{{}}\n{c}\trefs/tags/cli/v0.1.0\n"
    );

    let resolve = |commit: &str| -> String {
        let dir = tempfile::tempdir().unwrap();
        let listing_path = dir.path().join("ls");
        let prog_path = dir.path().join("prog.awk");
        std::fs::write(&listing_path, &listing).unwrap();
        std::fs::write(&prog_path, program).unwrap();
        let out = std::process::Command::new("awk")
            .arg("-v")
            .arg(format!("c={commit}"))
            .arg("-f")
            .arg(&prog_path)
            .arg(&listing_path)
            .output()
            .expect("awk");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };

    assert_eq!(
        resolve(&b),
        "cli/v0.2.0",
        "an annotated tag must resolve from its peeled commit sha"
    );
    assert_eq!(
        resolve(&a),
        "",
        "the tag OBJECT sha must not resolve — a pin is never a tag object, and matching \
         one would install a release for a commit that is not the pin"
    );
    assert_eq!(
        resolve(&c),
        "cli/v0.1.0",
        "a lightweight tag must still resolve from its only line"
    );
    assert_eq!(
        resolve(&d),
        "",
        "a commit with no release must resolve to nothing, so the build falls through to \
         the source path"
    );
}

/// The tag refspec must keep its trailing `*`, or the peeled refs never arrive.
///
/// [`the_tag_resolver_matches_the_commit_and_not_the_tag_object`] drives the awk over a
/// fixture listing, so it says nothing about how that listing is *obtained*. And the
/// obtaining is where the second half of the same trap lives: `git ls-remote` applies its
/// pattern before emitting peeled refs, so a refspec naming one tag exactly returns only
/// that tag's bare line — the tag object — and never the `^{}` commit the resolver matches.
///
/// Measured against the real origin while verifying the first release:
///
/// ```text
/// $ git ls-remote --tags <origin> 'cli/v0.2.0'
/// fe01247…  refs/tags/cli/v0.2.0
/// $ git ls-remote --tags <origin> 'cli/v*'
/// fe01247…  refs/tags/cli/v0.2.0
/// ee3a7f6…  refs/tags/cli/v0.2.0^{}
/// ```
///
/// Narrowing the refspec therefore looks like a tidy-up and silently retires the download
/// path. This is the assertion that makes it fail instead.
#[test]
fn the_tag_refspec_globs_so_peeled_refs_are_returned() {
    let run = parse("mise.yidam.toml")["yidam-build"]["run"]
        .as_str()
        .expect("yidam-build.run")
        .to_string();

    // Skip comment lines. The comment above the command quotes both spellings as worked
    // examples, so a naive scan finds the exact-tag one being warned about and reports the
    // documentation as the defect.
    let line = run
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .find(|l| l.contains("ls-remote --tags") && l.contains("cli/v"))
        .unwrap_or_else(|| panic!("yidam-build no longer lists cli/v tags"));

    let start = line.find("'").expect("refspec is quoted") + 1;
    let refspec = &line[start..start + line[start..].find('\'').expect("unterminated refspec")];

    assert!(
        refspec.ends_with('*'),
        "the tag refspec is `{refspec}`; it must end in `*` or `git ls-remote` returns no \
         `^{{}}` peeled refs, and the resolver — which matches on the peeled commit — can \
         never find a release again"
    );
}
