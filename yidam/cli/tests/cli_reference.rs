//! `docs/cli-reference.md` documents the command surface. This checks it still is the surface.
//!
//! A reference page listing forty commands is a copy of something the binary already knows,
//! and a copy goes stale in the direction nobody looks: a command added today is documented
//! never, and the page keeps rendering, keeps building, and keeps being wrong. That failure is
//! silent by construction — no reader can tell a page that omits a command from one whose
//! subject has none.
//!
//! Both sides are **discovered**, neither is a list in this file. A hardcoded roster is the
//! same rot one level up: it would stop covering new commands without ever going red.
//!
//! Narrow on purpose. It checks that the set of commands matches. It does not read the prose,
//! and it cannot tell a correct description from a plausible wrong one — that is review's job.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = yidam/cli/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference() -> String {
    let p = repo_root().join("docs/cli-reference.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// Every subcommand this binary offers, read out of its own `--help`.
///
/// Asking the built binary rather than the source is deliberate: it is the same question a
/// reader asks, answered the same way, and it stays correct through a refactor of how the
/// clap enum is spelled.
///
/// A command line in that output is indented, starts with the name, and is followed by either
/// the `*` write-marker or two spaces before its description. The group headings are flush
/// left and the option block is filtered out by requiring a lowercase-and-hyphens name.
fn commands_from_help() -> BTreeSet<String> {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_yidam"))
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
            continue; // continuation, or an option like `-h, --help`
        }
        let name = rest.split_whitespace().next().unwrap_or_default();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            continue;
        }
        // `help` is clap's own, and is not part of the documented surface.
        if name == "help" {
            continue;
        }
        found.insert(name.to_string());
    }

    assert!(
        found.len() > 20,
        "parsed only {} command(s) from --help — the output shape changed and this test is \
         no longer reading it: {found:?}",
        found.len()
    );
    found
}

/// Every command name the reference page writes in a table's first cell.
///
/// The page's command tables lead each row with the command in backticks, sometimes carrying
/// an argument or the `*` write-marker. Subcommand tables (`migrate`'s, `tonpa`'s) lead with a
/// bare subcommand name and would be indistinguishable here, so those rows are excluded by
/// requiring the row to be in a table whose first column header is `Command`.
fn commands_from_reference() -> BTreeSet<String> {
    let text = reference();
    let mut found = BTreeSet::new();
    let mut in_command_table = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            in_command_table = false;
            continue;
        }
        let first_cell = trimmed
            .trim_start_matches('|')
            .split('|')
            .next()
            .unwrap_or_default()
            .trim();

        if first_cell == "Command" {
            in_command_table = true;
            continue;
        }
        if !in_command_table {
            continue;
        }
        // `| `neighbors <node>` | …` → `neighbors`
        let Some(inner) = first_cell.strip_prefix('`') else {
            continue;
        };
        let Some((code, _)) = inner.split_once('`') else {
            continue;
        };
        if let Some(name) = code.split_whitespace().next() {
            found.insert(name.to_string());
        }
    }

    assert!(
        found.len() > 20,
        "parsed only {} command(s) out of cli-reference.md — its tables changed shape and this \
         test is no longer reading them: {found:?}",
        found.len()
    );
    found
}

#[test]
fn every_command_the_binary_offers_is_in_the_reference() {
    let built = commands_from_help();
    let documented = commands_from_reference();

    let undocumented: Vec<_> = built.difference(&documented).collect();
    assert!(
        undocumented.is_empty(),
        "these commands exist and docs/cli-reference.md does not list them: {undocumented:?}\n\
         Add a row to the matching group's table."
    );
}

#[test]
fn every_command_the_reference_lists_still_exists() {
    let built = commands_from_help();
    let documented = commands_from_reference();

    // `tonpa` is behind a default feature, so a `--no-default-features` build legitimately
    // has no such command while the page still documents it — the page says so in that row.
    // Nothing else in the surface is gated at the clap level: `index-build` and the gated
    // export formats are always present and refuse at runtime, which is what lets this
    // direction be checked at all.
    let gated: BTreeSet<String> = ["tonpa"].iter().map(|s| s.to_string()).collect();

    let stale: Vec<_> = documented
        .difference(&built)
        .filter(|c| !gated.contains(*c))
        .collect();
    assert!(
        stale.is_empty(),
        "docs/cli-reference.md documents commands this binary does not have: {stale:?}\n\
         Remove the row, or check whether the command was renamed."
    );
}
