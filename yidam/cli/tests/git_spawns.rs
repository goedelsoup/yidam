//! One place that knows how to run git.
//!
//! #929: the spawn appeared 169 times across 73 files under `yidam/cli` — 50 of them
//! production across 26 files, the other 119 building the repositories the suite tests
//! against. Each site re-decided
//! `-C <root>` against `.current_dir`, whether to strip an inherited `GIT_DIR`, whether to
//! pin `core.quotepath`, where a user-supplied revision stops being a revision, and what a
//! non-zero exit means. That last one had four answers in production alone: `None`
//! (`cohort.rs`, `phases.rs`, `provenance.rs`), an `Err` carrying stderr
//! (`catalog/commit.rs`), `""` — indistinguishable from an empty answer — (`lint/scope.rs`,
//! `lint/lineage.rs`), and an `Option<Vec<String>>` (`git.rs::git_lines`). Three defences
//! were therefore present at one site and absent at the rest, and all three were live:
//!
//! - an exported `GIT_DIR` redirected every read, past both `.current_dir()` and `-C`
//! - `core.quotepath` (default on) escaped non-ASCII corpus paths out of `diff --name-status`
//! - `--end-of-options` closed `--at=--output=<path>` at `query/at.rs` and nowhere else
//!
//! They are closed in [`yidam::git::run`], which is now the only production code that spawns
//! git, and they stay closed only for as long as that stays true. Hence this file.
//!
//! ## What it checks, and what it cannot
//!
//! The literal `"git"` may appear in code in exactly three files, named below and each
//! re-checked. Forbidding the *literal* rather than the *spawn* is deliberate:
//! `Command::new(GIT)` behind a `const GIT: &str = "git";` is precisely how a
//! literal-argument scan is walked past, and resolving an arbitrary expression back to a
//! program name needs a compiler. Measured across every `.rs` under `src/` and `tests/`, the
//! stricter rule has **zero** false positives today — the word appears nowhere else in code —
//! so it costs nothing and closes the indirection.
//!
//! It cannot see a git invoked through a shell (`sh -c "git …"`), through `mise`, or from a
//! script this repository ships. Those are spawns of a shell, they are rare, and they are
//! outside what one Rust-side runner could own anyway.
//!
//! ## Why three files and not one
//!
//! - `src/git/run.rs` — the runner. Production.
//! - `src/git/fixture.rs` — `#[cfg(test)]` helpers that *build* repositories for the unit
//!   tests. They deliberately do not go through the runner: a fixture running through the
//!   thing under test could construct a repository that hides the bug.
//! - `tests/common/git.rs` — the same fixtures for the integration suite, which cannot reach
//!   a `#[cfg(test)]` module of the library across the crate boundary.
//!
//! The second and third are one helper split by a language rule, not a third opinion. All
//! three are asserted to still spawn, so an allowance whose file stopped needing it goes red
//! instead of quietly becoming a hole.
//!
//! ## Mutations it was checked against
//!
//! A file-scanning test that looks at nothing passes, so this one was broken on purpose
//! before being trusted. Each of these was applied by hand and the named test went red:
//!
//! | mutation | caught by |
//! |---|---|
//! | `Command::new("git")` added to a fourth file | both scan tests |
//! | the same, spelled `Command::new(GIT)` behind a `const GIT: &str = "git"` | `no_other_file_names_the_git_program` only — which is why it exists |
//! | an entry in `MAY_SPAWN_GIT` for a file that does not spawn | `the_gate_sees_what_it_is_looking_for` |
//! | `code_of` returning the empty string | `the_gate_sees_what_it_is_looking_for` |
//!
//! And one that must *not* fire: the literal written in a comment and nowhere else, which
//! this file's own prose does a dozen times.

use std::path::PathBuf;

mod common;

/// The files that may name the git program, and why each one is not the others.
const MAY_SPAWN_GIT: &[(&str, &str)] = &[
    (
        "src/git/run.rs",
        "the runner: owns cwd, env, config and exit policy",
    ),
    (
        "src/git/fixture.rs",
        "unit-test fixtures, which must not run through the code under test",
    ),
    (
        "tests/common/git.rs",
        "the same fixtures for `tests/`, which cannot reach a `#[cfg(test)]` module",
    ),
];

/// Files excused for naming `"git"` in code without spawning it.
///
/// **Empty, and meant to stay that way.** An entry here is a promise that the literal is not
/// a program name — a config key, a match arm, a format argument. Each is re-checked by
/// [`every_exemption_is_still_describing_its_file`], because an exemption whose file no
/// longer contains what it excuses has stopped being an exemption and become a hole.
const NOT_A_SPAWN: &[(&str, &str)] = &[];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` under `src/` and `tests/`, as a path relative to `yidam/cli/`.
///
/// A text scan, so it covers the `--features index` code that PR CI never compiles —
/// `index_verify.rs` spawns a program too, and a check that only saw the default feature set
/// would report all clear about a file it had not read.
fn suite_sources() -> Vec<(String, String)> {
    let dir = crate_dir();
    let mut out: Vec<(String, String)> = ["src", "tests"]
        .iter()
        .flat_map(|sub| common::repo_walk(&dir.join(sub)))
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .map(|e| {
            let rel = e
                .path()
                .strip_prefix(&dir)
                .unwrap_or(e.path())
                .to_string_lossy()
                .replace('\\', "/");
            (
                rel,
                std::fs::read_to_string(e.path()).expect("source is readable"),
            )
        })
        .collect();
    out.sort();
    out
}

/// Prose removed, so the rule is read from code and not from the paragraphs describing it.
///
/// This file spells `Command::new("git")` a dozen times above in order to say what it
/// forbids; so does `git/run.rs`, and so does `git_hygiene.rs`. A scan that graded comments
/// would find the documentation of the rule and report it as a breach of the rule.
fn code_of(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The gate can see a spawn, and cannot see one written about.
///
/// Without this, a `code_of` that returned the empty string — or a walk that read no files —
/// would satisfy every assertion below in silence, and the gate would be green having looked
/// at nothing.
#[test]
fn the_gate_sees_what_it_is_looking_for() {
    let sources = suite_sources();
    assert!(
        sources.len() > 150,
        "only {} sources found across src/ and tests/; this crate has many more, so every \
         assertion below is scanning the wrong directory",
        sources.len()
    );

    // The known positives: each allowed file must still actually spawn git. An allowance for
    // a file that stopped spawning is one more place the rule appears to be enforced.
    for (file, why) in MAY_SPAWN_GIT {
        let (_, text) = sources
            .iter()
            .find(|(p, _)| p == file)
            .unwrap_or_else(|| panic!("{file} is allowed to spawn git ({why}) and is missing"));
        assert!(
            code_of(text).contains("Command::new(\"git\")"),
            "{file} is allowed to spawn git ({why}) and no longer does. Delete the entry: an \
             allowance nobody needs is a hole nobody is watching."
        );
    }

    // The detector on text whose answer is known, both ways round.
    assert!(
        code_of("let c = Command::new(\"git\");").contains("\"git\""),
        "the scan cannot see a spawn written in code"
    );
    assert!(
        !code_of("// Command::new(\"git\") is what this forbids").contains("\"git\""),
        "the scan grades comments, so prose describing the rule reads as a breach of it"
    );
}

/// `Command::new("git")` appears in three files, and that is the whole of it.
#[test]
fn only_the_runner_and_its_fixtures_spawn_git() {
    let allowed: Vec<&str> = MAY_SPAWN_GIT.iter().map(|(f, _)| *f).collect();
    let mut offenders = Vec::new();

    for (path, text) in suite_sources() {
        if path == "tests/git_spawns.rs" || allowed.contains(&path.as_str()) {
            continue;
        }
        if code_of(&text).contains("Command::new(\"git\")") {
            offenders.push(path);
        }
    }

    assert!(
        offenders.is_empty(),
        "these spawn git themselves:\n  {}\n\nWhich is #929: the mechanics get re-decided per \
         site, and a defence applied at one is absent at the rest. Production goes through \
         `crate::git::Git`, which pins the config, strips an inherited `GIT_DIR` and gives \
         revisions an `--end-of-options` slot; a fixture that builds a repository goes \
         through `crate::git::fixture` (or `common::git` under tests/).",
        offenders.join("\n  ")
    );
}

/// And nothing else names the program, so the spawn cannot be hidden behind a binding.
///
/// The separate assertion is the point. `only_the_runner_and_its_fixtures_spawn_git` reads a
/// literal argument, which a `const GIT: &str = "git";` two lines up defeats without the
/// scan noticing anything at all.
#[test]
fn no_other_file_names_the_git_program() {
    let allowed: Vec<&str> = MAY_SPAWN_GIT.iter().map(|(f, _)| *f).collect();
    let excused: Vec<&str> = NOT_A_SPAWN.iter().map(|(f, _)| *f).collect();
    let mut offenders = Vec::new();

    for (path, text) in suite_sources() {
        if path == "tests/git_spawns.rs"
            || allowed.contains(&path.as_str())
            || excused.contains(&path.as_str())
        {
            continue;
        }
        let code = code_of(&text);
        for (n, line) in code.lines().enumerate() {
            if line.contains("\"git\"") {
                offenders.push(format!("{path}:{}: {}", n + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these name the git program in code:\n  {}\n\nIf it is a spawn, route it through \
         `crate::git::Git` or the fixture helper. If the literal genuinely is not a program \
         name, this gate cannot tell the difference — add the file to NOT_A_SPAWN with the \
         reason, and know that it stops being watched for the real thing.",
        offenders.join("\n  ")
    );
}

/// An exemption that has stopped describing its file is a hole, not an exemption.
#[test]
fn every_exemption_is_still_describing_its_file() {
    for (file, why) in NOT_A_SPAWN {
        let path = crate_dir().join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{file} is exempted and unreadable: {e} — {why}"));
        assert!(
            code_of(&text).contains("\"git\""),
            "{file} is exempted for naming the git program ({why}) and no longer names it. \
             Delete the entry."
        );
    }
}
