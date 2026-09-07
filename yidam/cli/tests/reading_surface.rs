//! The reading surface has to be named where an agent will meet it, and every query it names
//! has to run (#726).
//!
//! # What was measured
//!
//! Across ~170 corpus-building sessions in twelve derived repositories, 62,276 shell
//! invocations named `yidam`. `lint` ran 1,953 times and `graph-check` 1,289. `query`, `pack`,
//! `estimate` and `neighbors` ran **zero** times, and the corpus was navigated with 10,938
//! `grep`s. The cause was not preference: the retrieval surface appeared in the agent-facing
//! guidance of a derived repository zero times, and its only occurrences anywhere in the
//! vendored prelude were SDK parity *fixtures* — files no authoring agent reads.
//!
//! The gates got the opposite treatment and are at 100% adoption. The difference between the
//! two is documentation, and documentation is what rots silently — which is what this file is
//! for.
//!
//! # Why the assertion is about the *derived* contract
//!
//! `sadhana/root/AGENTS.md` and `sadhana/root/CLAUDE.md` are what a derived repository
//! actually gets; this repository's own `AGENTS.md` is read by people maintaining the
//! template. Checking the second would pass while every derived repository stayed silent,
//! which is precisely the state that was measured.
//!
//! # Narrow on purpose
//!
//! This checks that the commands are *named* and that the queries *run*. It cannot tell good
//! guidance from bad, and it cannot tell whether adoption moved — the issue is explicit that
//! only re-running the measurement answers that. What it can do is fail the day someone
//! deletes the section, and fail the day a documented query stops parsing.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

mod common;

use common::{repo_root, tracked_under};

/// The commands the measurement found at zero adoption, plus the one that says what is owed.
///
/// A hardcoded list, unlike [`commands_from_help`] below — and deliberately so. This is not
/// "every command exists in the docs", which `cli_reference.rs` already covers against a
/// discovered roster. It is the specific set #726 measured, and shrinking it is a decision
/// somebody has to make on purpose rather than a name quietly dropping out of a scan.
const RETRIEVAL: &[&str] = &["query", "pack", "estimate", "neighbors", "due"];

const GUIDELINE: &str = "yidam/prelude/guidelines/reading-the-corpus.md";

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// What a derived repository is handed: its root contract and the short version beside it.
fn derived_contract() -> String {
    format!(
        "{}\n{}",
        read("sadhana/root/AGENTS.md"),
        read("sadhana/root/CLAUDE.md")
    )
}

#[test]
fn the_derived_repo_contract_names_every_retrieval_command() {
    let text = derived_contract();
    let missing: Vec<&str> = RETRIEVAL
        .iter()
        .copied()
        .filter(|c| !text.contains(&format!("yidam {c}")))
        .collect();
    assert!(
        missing.is_empty(),
        "a derived repository's AGENTS.md and CLAUDE.md name none of: {missing:?}\n\
         That is the state #726 measured — 62,276 `yidam` invocations and zero runs of the \
         reading surface, because nothing an authoring agent reads had ever mentioned it."
    );
}

/// The guideline is only guidance if something points at it from where an agent starts.
#[test]
fn the_guideline_is_linked_from_both_contracts() {
    for (file, needle) in [
        (
            "sadhana/root/AGENTS.md",
            "prelude/guidelines/reading-the-corpus.md",
        ),
        (
            "sadhana/root/CLAUDE.md",
            "prelude/guidelines/reading-the-corpus.md",
        ),
        (
            "AGENTS.md",
            "yidam/prelude/guidelines/reading-the-corpus.md",
        ),
    ] {
        assert!(
            read(file).contains(needle),
            "{file} does not link {needle} — an unlinked guideline is the vendored-fixture \
             situation again, one directory over"
        );
    }
}

/// `grep` is what was used instead, so the guidance has to say so in as many words.
///
/// Weak as an assertion and worth having anyway: the finding was not "agents did not know
/// `query` existed" in the abstract, it was "agents reached for `grep`". Guidance that lists
/// commands without naming what it is displacing leaves the reader with two options and no
/// reason to switch.
#[test]
fn the_guidance_names_what_it_is_displacing() {
    for file in [
        "sadhana/root/AGENTS.md",
        "sadhana/root/CLAUDE.md",
        GUIDELINE,
    ] {
        assert!(
            read(file).contains("grep"),
            "{file} recommends the reading surface without naming `grep`, which is what the \
             measurement found in its place"
        );
    }
}

/// Every subcommand this binary offers, from its own `--help`.
///
/// Discovered rather than listed, for `cli_reference.rs`'s reason: a roster written here
/// stops covering a rename without ever going red.
fn commands_from_help() -> BTreeSet<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .arg("--help")
        .output()
        .expect("running `yidam --help`");
    assert!(out.status.success(), "`yidam --help` exited nonzero");
    let help = String::from_utf8(out.stdout).expect("--help is utf-8");
    let mut found = BTreeSet::new();
    for line in help.lines() {
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') || rest.starts_with('-') {
            continue;
        }
        let name = rest.split_whitespace().next().unwrap_or_default();
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            found.insert(name.to_string());
        }
    }
    found
}

#[test]
fn every_command_the_retrieval_set_names_is_one_this_binary_has() {
    let have = commands_from_help();
    let missing: Vec<&&str> = RETRIEVAL.iter().filter(|c| !have.contains(**c)).collect();
    assert!(
        missing.is_empty(),
        "the guidance recommends commands this binary does not have: {missing:?}"
    );
}

// ── the documented queries actually run ───────────────────────────────────────

/// `examples/streamflow` as a standalone repository — `query_goldens.rs`'s recipe.
///
/// From `git ls-files` rather than a directory walk, so the test measures the corpus and not
/// the maintainer's working directory.
fn stage() -> tempfile::TempDir {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    for tracked in tracked_under(&root, "examples/streamflow/") {
        let to = dir
            .path()
            .join(tracked.strip_prefix("examples/streamflow/").unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(&tracked), &to).unwrap();
    }
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "reading@yidam.test"],
        vec!["config", "user.name", "Reading"],
        vec!["add", "-A"],
        vec!["commit", "-qm", "genesis: streamflow"],
    ] {
        assert!(Command::new("git")
            .args(&args)
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success());
    }
    dir
}

/// Every `yidam …` line inside a fenced block in the guideline.
///
/// The guideline's examples are written against `examples/streamflow` on purpose: the prelude
/// cannot know a derived repository's class names, and an example nobody can run is how the
/// surface ended up documented only in parity fixtures. A placeholder shape (`<class>`) is
/// what the scaffolded `AGENTS.md` carries instead, under a `TEMPLATE` block telling bootstrap
/// to fill and run it — which is why that file is not scanned here.
fn documented_invocations() -> Vec<String> {
    let text = read(GUIDELINE);
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced && line.trim_start().starts_with("yidam ") {
            out.push(line.trim().to_string());
        }
    }
    assert!(
        out.len() >= 4,
        "parsed only {} invocation(s) out of {GUIDELINE} — its fenced blocks changed shape \
         and this test stopped checking anything",
        out.len()
    );
    out
}

/// **The assertion worth the most here.** A documented query that does not parse teaches an
/// agent the surface is broken, and it fails in exactly the way the surface's own diagnostics
/// exist to prevent — silently, as an empty result.
///
/// Exit 0 rather than a non-empty answer: `query` never gates, and one of these examples is
/// deliberately an *empty* result, because the absence diagnosis is the reason to prefer the
/// command at all. Exit 1 means the query was rejected, which is the failure being caught.
#[test]
fn every_query_the_guideline_documents_runs() {
    let repo = stage();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_yidam"));
    for invocation in documented_invocations() {
        let args = shell_words(&invocation);
        let out = Command::new(&bin)
            .args(&args[1..])
            .current_dir(repo.path())
            .output()
            .unwrap_or_else(|e| panic!("running `{invocation}`: {e}"));
        assert!(
            out.status.success(),
            "`{invocation}` is documented in {GUIDELINE} and exits {:?}:\n{}{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// Split a documented command line, honouring `'…'` and `"…"`.
///
/// A query is one argument containing spaces, and every example here quotes it — so a naive
/// whitespace split would hand the binary five arguments and this test would check that
/// `yidam query reach` fails, which it should.
fn shell_words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for ch in line.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, c @ ('\'' | '"')) => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() || started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() || started {
        out.push(cur);
    }
    out
}
