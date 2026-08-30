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

    /// Every command string a TOML value can hold: a bare string, or a list of them.
    fn commands(v: Option<&toml::Value>) -> Vec<String> {
        match v {
            Some(toml::Value::String(s)) => vec![s.clone()],
            Some(toml::Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => vec![],
        }
    }

    // Every place the derived config can name a task, discovered rather than listed.
    // `[tasks]` was the only one read here, and then `[hooks] postinstall` became a second —
    // a call site the walk did not cover, so a hook naming a task that does not exist would
    // fail at `mise install` in someone's fresh clone rather than here. That is the shape
    // this file already guards against one level up; it is worth not reintroducing it in the
    // guard itself.
    let mut call_sites: Vec<(String, Vec<String>)> = Vec::new();
    if let Some(tasks) = cfg.get("tasks").and_then(|t| t.as_table()) {
        for (name, task) in tasks {
            call_sites.push((format!("task `{name}`"), commands(task.get("run"))));
        }
    }
    let hooks = cfg
        .get("hooks")
        .and_then(|h| h.as_table())
        .unwrap_or_else(|| {
            panic!(
                "sadhana/root/mise.toml declares no [hooks]; `postinstall` is what makes \
                 `mise install` leave a fresh clone with its corpora rather than with an \
                 empty .yidam/tonpa/"
            )
        });
    for (name, value) in hooks {
        call_sites.push((format!("hook `{name}`"), commands(Some(value))));
    }

    let mut missing = Vec::new();
    for (site, lines) in &call_sites {
        for line in lines {
            let Some(rest) = line.trim().strip_prefix("mise run ") else {
                continue;
            };
            let called = rest.split_whitespace().next().unwrap_or_default();
            if !called.is_empty() && !inherited.contains(called) && !own.contains(called) {
                missing.push(format!(
                    "{site} runs `mise run {called}`, which is defined nowhere"
                ));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));

    // ...and the walk must have found the call sites it claims to read. A scan that reaches
    // neither table reports no missing tasks and passes.
    let checked: usize = call_sites.iter().map(|(_, l)| l.len()).sum();
    assert!(
        checked > 0 && call_sites.iter().any(|(s, _)| s.starts_with("hook")),
        "the walk read {checked} command(s) across {} call site(s) and no hook among them; \
         it is not reading what it claims to",
        call_sites.len()
    );
}

/// `mise install` must leave a fresh clone with its corpora, not just its toolchains.
///
/// Three separate things have to hold together, and each one is silent on its own.
///
/// The hook has to be in the **consumer's** `mise.toml`. mise reads `[hooks]` in a
/// task-config include as a task named `hooks` and fails the entire file to parse — the same
/// constraint as `[env] _.path` and `[tools]`, and the one that orphaned all 35 inherited
/// tasks once already. So a well-meaning move of this block into `mise.yidam.toml` does not
/// degrade, it takes everything with it; `the_task_file_holds_no_config_sections` above is
/// what catches that direction.
///
/// It has to run **after** the tools are installed, or a repository whose pin is released has
/// no `yidam` yet when the hook fires. Verified against mise 2026.7.0: postinstall runs after
/// installation and with the newly installed tools on PATH, on a first install and on a
/// repeat one alike.
///
/// And the task it names has to tolerate **no binary at all**. A pin with no release has
/// nothing on PATH until `yidam-build` compiles one, and failing `mise install` because the
/// step after it has not run yet would make provisioning depend on its own output.
#[test]
fn the_install_hook_fetches_corpora_and_survives_a_repo_with_no_binary() {
    let cfg = parse("sadhana/root/mise.toml");
    let hook = cfg["hooks"]["postinstall"]
        .as_str()
        .expect("[hooks] postinstall is a string");
    assert_eq!(
        hook, "mise run tonpa-install",
        "the postinstall hook must call the named task rather than inlining a command, so a \
         contributor has something to run by hand and one definition backs both"
    );

    let task = &parse("mise.yidam.toml")["tonpa-install"];
    let run = task["run"].as_str().expect("tonpa-install.run");
    assert!(
        run.contains("yidam tonpa install"),
        "tonpa-install does not run `yidam tonpa install`:\n{run}"
    );
    assert!(
        run.contains("command -v yidam") && run.contains("exit 0"),
        "tonpa-install must exit 0 when no `yidam` is on PATH. A repository whose pin carries \
         no release has no binary until `mise run yidam-build` compiles one, and the hook \
         runs during `mise install` — so without this, provisioning toolchains fails because \
         the step that comes after it has not happened yet:\n{run}"
    );
    // The absence of the binary is the ONLY thing swallowed. A dependency that cannot be
    // fetched must still return non-zero.
    //
    // What the task returns and what `mise install` returns are different questions, and the
    // issue this came from assumed they were the same. Measured against mise 2026.7.0: a
    // failing postinstall hook is logged `mise WARN Postinstall hook … failed` and
    // `mise install` exits 0 regardless. So this assertion is not protecting `mise install`'s
    // exit code — nothing can — it is protecting the only signal that survives, which is the
    // task's own error output.
    assert!(
        !run.contains("yidam tonpa install || true") && !run.contains("|| exit 0"),
        "tonpa-install swallows `tonpa install`'s exit code. A dependency that cannot be \
         fetched is exactly what this hook exists to surface:\n{run}"
    );
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
    // The compile lives in `yidam-build-source` since #408 — `yidam-build` itself downloads
    // and never invokes cargo. Read whichever of the two actually runs it, so this test
    // follows the code instead of naming a task that stopped compiling.
    let compiling: Vec<&str> = ["yidam-build", "yidam-build-source"]
        .iter()
        .filter_map(|t| inherited.get(*t))
        .filter_map(|t| t["run"].as_str())
        .filter(|run| run.contains("cargo install"))
        .collect();
    assert_eq!(
        compiling.len(),
        1,
        "exactly one inherited task must run `cargo install` — two would be two definitions \
         of how this repository's binary is built, and zero means the source path is gone"
    );
    // Commands only. The comment above the flag explains `--force` BY NAME — three sentences
    // of it — so `run.contains("--force")` was answered by the prose describing the fix and
    // stayed green with the flag deleted. Found by mutation, not by reading; it had been
    // true since the test was written.
    fn commands_only(run: &str) -> String {
        run.lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
    }

    let derived_run = commands_only(compiling[0]);
    assert!(
        derived_run.contains("--force"),
        "mise.yidam.toml's source build must pass --force. A comment naming the flag is not \
         the flag:\n{derived_run}"
    );

    // The same lesson, in this repository's own copy — the destination a port forgets.
    let own = parse("mise.toml");
    let own_run = commands_only(own["tasks"]["yidam-build"]["run"].as_str().unwrap());
    assert!(
        own_run.contains("--force"),
        "mise.toml's yidam-build must pass --force too:\n{own_run}"
    );
}

/// The pin must cross into the source build, and the source build must refuse without it.
///
/// Splitting the compile into its own task bought a toolchain nobody provisions on the
/// download path, and cost a boundary. `$origin` and `$commit` are read from `.yidam.toml`
/// by `yidam-build`; `yidam-build-source` runs in a different process and gets them through
/// the environment.
///
/// A source build that quietly defaulted to origin/HEAD would install a binary that is not
/// this repository's pin — silently, and looking exactly like success. That is the failure
/// `.yidam/bin` and the whole per-repository install exist to prevent, so the sub-task
/// refuses rather than guesses.
#[test]
fn the_pin_crosses_into_the_source_build_and_is_not_guessed() {
    let inherited = parse("mise.yidam.toml");
    let build = inherited["yidam-build"]["run"].as_str().unwrap();
    let source = inherited["yidam-build-source"]["run"].as_str().unwrap();

    assert!(
        build.contains("mise run yidam-build-source"),
        "yidam-build never hands off to yidam-build-source, so an untagged pin has no \
         source path at all:\n{build}"
    );
    for var in ["YIDAM_SOURCE_ORIGIN", "YIDAM_SOURCE_COMMIT"] {
        assert!(
            build.contains(var),
            "yidam-build invokes yidam-build-source without passing `{var}`; the sub-task \
             cannot read .yidam.toml's pin from another process"
        );
        assert!(
            source.contains(var),
            "yidam-build-source never reads `{var}`, so the pin yidam-build passes is \
             discarded"
        );
    }
    // Refuses rather than defaults. `${VAR:-}` and then a check — not `${VAR:-HEAD}`.
    assert!(
        source.contains("exit 1"),
        "yidam-build-source does not fail when the pin is absent. Defaulting would compile \
         whatever the origin's HEAD happens to be and install it as this repository's \
         pinned binary:\n{source}"
    );
    for wrong in ["YIDAM_SOURCE_COMMIT:-HEAD", "YIDAM_SOURCE_COMMIT:-main"] {
        assert!(
            !source.contains(wrong),
            "yidam-build-source defaults the commit to `{wrong}` — a source build that \
             falls back to a branch tip installs a binary that is not the pin"
        );
    }
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

/// The `[tools]` entry is derived from the pin, and touches nothing else in the file.
///
/// `.yidam.toml` pins a **commit**; a mise `[tools]` entry names a **version**. VERSIONING.md
/// argues against carrying both as "a third thing to get out of step", and it is right — so
/// the version is not stored anywhere. `yidam-vendor-update` derives it from the commit it
/// already writes, and rewrites one marked region of the consumer's `mise.toml`.
///
/// That file is the one thing this task must not replace wholesale: everything outside the
/// markers is domain-owned. And the region has to live *inside* the existing `[tools]` table
/// rather than appending a second one, because TOML rejects a duplicate table outright —
/// verified against mise 2026.7.0, which reports `TOML parse error … [tools]` and loads no
/// settings at all.
///
/// The awk is read out of `mise.yidam.toml` rather than copied here, for the reason the tag
/// resolver above gives: a copy would pass while the file rotted.
#[test]
fn the_tools_block_is_written_from_the_pin_and_only_between_its_markers() {
    let run = parse("mise.yidam.toml")["yidam-vendor-update"]["run"]
        .as_str()
        .expect("yidam-vendor-update.run")
        .to_string();

    let marker = "tools_block='";
    let start = run
        .find(marker)
        .expect("yidam-vendor-update defines tools_block")
        + marker.len();
    let program = &run[start..start + run[start..].find('\'').expect("unterminated awk")];
    assert!(
        !program.contains('\\'),
        "the tools_block awk contains a backslash. It is embedded in a TOML multi-line \
         string, extracted by this test, and run through sh — three escaping layers, and \
         every one of them a different set of rules. Keep it backslash-free:\n{program}"
    );

    let dir = tempfile::tempdir().unwrap();
    let prog = dir.path().join("tools.awk");
    std::fs::write(&prog, program).unwrap();

    let rewrite = |input: &str, body: &str| -> String {
        let src = dir.path().join("in.toml");
        std::fs::write(&src, input).unwrap();
        let out = std::process::Command::new("awk")
            .arg("-v")
            .arg(format!("body={body}"))
            .arg("-f")
            .arg(&prog)
            .arg(&src)
            .output()
            .expect("awk");
        assert!(
            out.status.success(),
            "awk failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    // A derived repository's mise.toml: a `[tools]` table that already has domain content,
    // and tasks after it that must survive untouched.
    const DERIVED: &str = "[task_config]\nincludes = [\"mise.yidam.toml\"]\n\n\
                           [tools]\nrust = { version = \"1.88.0\" }\n\n\
                           [tasks.build]\nrun = \"echo domain\"\n";
    let released =
        "\"github:goedelsoup/yidam\" = { version = \"0.6.0\", version_prefix = \"cli/v\" }";
    let unreleased = "# no cli/v* release at this pin";

    let first = rewrite(DERIVED, released);
    assert!(
        first.contains(released),
        "a pin with a release must produce a [tools] entry:\n{first}"
    );
    // `version_prefix` is asserted against the SHIPPED entry, not against the fixture body
    // above — that would be this test grading its own input. The entry is built by a printf
    // in the task, and it is the task that has to carry the option.
    assert!(
        run.contains(r#"version_prefix = "cli/v""#),
        "yidam-vendor-update writes a [tools] entry without `version_prefix = \"cli/v\"`. \
         Without it mise resolves this repository's four-layer release list repository-wide: \
         `@latest` picks up `editor/v*`, whose release ships only a .vsix, and a pinned bare \
         version 404s on releases/tags/goedelsoup/yidam@<v>. It does not degrade — it fails."
    );
    assert!(
        run.contains("github:goedelsoup/yidam"),
        "yidam-vendor-update names no backend for the [tools] entry"
    );
    // The tag lookup must filter by the CLI layer's prefix. Four layers tag onto one
    // history and they land on the same commit: `cli/v0.6.0`, `editor/v0.2.0` and `v0.1.1`
    // all point at c29ba9f. An unfiltered `git tag --points-at` returns all three, and the
    // one it happens to return first would decide which version this entry names.
    assert!(
        run.contains("--points-at") && run.contains("'cli/v*'"),
        "yidam-vendor-update resolves the pin's tag without filtering to `cli/v*`. Three \
         layers tag the same commit in this repository, so an unfiltered lookup names \
         whichever ref sorts first — and `editor/v*` is not a version the CLI backend can \
         resolve to an asset."
    );
    // The entry belongs to the table that is already there. A second `[tools]` is not a
    // cosmetic problem: TOML refuses the file.
    assert_eq!(
        first.matches("[tools]").count(),
        1,
        "the rewrite produced a second [tools] table; TOML rejects a duplicate table and \
         mise loads none of the file:\n{first}"
    );
    assert!(
        first.contains("rust = { version = \"1.88.0\" }") && first.contains("echo domain"),
        "the rewrite dropped domain-owned content outside the markers:\n{first}"
    );

    // Running it again with the same pin must change nothing at all. A task that rewrites a
    // file on every invocation puts a diff in front of someone who changed nothing.
    assert_eq!(
        rewrite(&first, released),
        first,
        "the rewrite is not idempotent"
    );

    // A bump replaces the entry rather than stacking a second one.
    let bumped = rewrite(
        &first,
        "\"github:goedelsoup/yidam\" = { version = \"0.7.0\", version_prefix = \"cli/v\" }",
    );
    assert_eq!(
        bumped.matches("github:goedelsoup/yidam").count(),
        1,
        "re-pinning left two entries; mise would read whichever TOML resolves last:\n{bumped}"
    );
    assert!(bumped.contains("0.7.0") && !bumped.contains("0.6.0"));

    // Most commits carry no release. The block is emptied and KEPT: it is the anchor the
    // next re-vendor edits, and its absence is indistinguishable from a repository that
    // predates the mechanism.
    let none = rewrite(&bumped, unreleased);
    assert!(
        !none.contains("github:goedelsoup/yidam"),
        "an untagged pin must not leave a version entry behind — it would name a release \
         that is not the pin:\n{none}"
    );
    assert!(
        none.contains("# <!-- YIDAM:TOOLS -->") && none.contains("# <!-- /YIDAM:TOOLS -->"),
        "the markers must survive an untagged pin, or the next re-vendor has nothing to \
         edit:\n{none}"
    );
    // ...and it fills again when a release does exist at the new pin.
    assert!(rewrite(&none, released).contains(released));
}

/// The scaffolded `mise.toml` must ship the markers, and must not ship a Rust toolchain.
///
/// Two halves of one change. The markers are the anchor `yidam-vendor-update` edits, so a
/// repository born without them gets the entry only once someone re-vendors — and the whole
/// point is that a fresh clone runs `mise install` and has its binary.
///
/// The toolchain is #396's actual complaint: `[tools]` carried Rust unconditionally so that
/// `yidam-build` had a chance of compiling the CLI, on every machine, for a binary that in
/// the common case was released as a tarball. `sadhana/crates/` ships only a README, so a
/// freshly bootstrapped repository has no `crates/Cargo.toml` and every domain task below
/// no-ops. `yidam-build` declares Rust as a per-task tool instead, which mise provisions
/// when that task runs rather than on `mise install` — verified: with rust declared only
/// there, `mise install` answers "all tools are installed" and provisions nothing.
#[test]
fn the_scaffolded_config_anchors_the_block_and_carries_no_compiler() {
    let text = std::fs::read_to_string(repo_root().join("sadhana/root/mise.toml"))
        .expect("sadhana/root/mise.toml");

    for m in ["# <!-- YIDAM:TOOLS -->", "# <!-- /YIDAM:TOOLS -->"] {
        assert!(
            text.contains(m),
            "sadhana/root/mise.toml does not ship `{m}`, so yidam-vendor-update has no \
             region to write the [tools] entry into and a fresh repository never gets one"
        );
    }
    // Inside `[tools]`, not beside it. The rewrite edits in place precisely because a second
    // `[tools]` table would make the file unparseable.
    let tools_at = text
        .find("\n[tools]")
        .expect("sadhana/root/mise.toml has [tools]");
    let marker_at = text.find("# <!-- YIDAM:TOOLS -->").unwrap();
    let next_table = text[tools_at + 1..]
        .find("\n[")
        .map(|i| tools_at + 1 + i)
        .unwrap_or(text.len());
    assert!(
        marker_at > tools_at && marker_at < next_table,
        "the YIDAM:TOOLS markers are not inside the [tools] table; the entry written there \
         would land in whatever table encloses them"
    );

    let scaffolded = parse("sadhana/root/mise.toml");
    let tools = scaffolded["tools"].as_table().expect("[tools] is a table");
    assert!(
        !tools.contains_key("rust"),
        "sadhana/root/mise.toml declares rust in [tools], so every derived repository \
         provisions a Rust toolchain on every machine at `mise install` — including the \
         ones with no crates/ workspace, to give yidam-build a chance of compiling a binary \
         its pin was released as. yidam-build declares its own."
    );

    // The compiler is declared on the task that compiles, and on no other. `yidam-build`
    // downloads a built binary; a `tools` declaration there provisions a Rust toolchain on
    // the path that never uses one, which is what #408 was.
    let inherited = parse("mise.yidam.toml");
    let source = &inherited["yidam-build-source"];
    let per_task = source
        .get("tools")
        .and_then(|t| t.as_table())
        .unwrap_or_else(|| {
            panic!(
                "yidam-build-source declares no `tools`, and the template no longer provides \
                 one — so an untagged pin has nothing to compile with"
            )
        });
    assert!(
        per_task.contains_key("rust"),
        "yidam-build-source's per-task tools do not include rust, which its `cargo install` \
         needs: {per_task:?}"
    );
    assert!(
        inherited["yidam-build"].get("tools").is_none(),
        "yidam-build declares `tools`. It downloads an already-compiled binary and hands the \
         compiling to yidam-build-source, so a declaration here provisions a toolchain on \
         the path that never invokes a compiler — for most repositories, every time."
    );
}
