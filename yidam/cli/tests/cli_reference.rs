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

mod common;

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

/// Every command name the reference page writes in a table's first cell.
///
/// The page's command tables lead each row with the command in backticks, sometimes carrying
/// an argument or the `*` write-marker. Subcommand tables (`migrate`'s, `tonpa`'s) lead with a
/// bare subcommand name and would be indistinguishable here, so those rows are excluded by
/// requiring the row to be in a table whose first column header is `Command`.
fn commands_from_reference() -> BTreeSet<String> {
    writers_from_reference().into_keys().collect()
}

/// Every command the page names, and whether any of its rows carries the `*` marker.
///
/// *Any* row, because a parent command gets a row per subcommand: `vault push` writes and
/// `vault status` does not, and `--help-all` has one entry for `vault` carrying the marker. So the
/// page marks a command as writing when one of the things it can do writes, which is what the
/// single entry in `--help-all` means too.
fn writers_from_reference() -> std::collections::BTreeMap<String, bool> {
    let text = reference();
    let mut found = std::collections::BTreeMap::new();
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
        // The marker sits outside the backticks, in the same cell: `` `vault push` * ``.
        let writes = first_cell
            .split('`')
            .next_back()
            .unwrap_or_default()
            .trim()
            .starts_with('*');
        if let Some(name) = code.split_whitespace().next() {
            // `|=`: one writing subcommand makes the parent a writer.
            *found.entry(name.to_string()).or_insert(false) |= writes;
        }
    }

    assert!(
        found.len() > 20,
        "parsed only {} command(s) out of cli-reference.md — its tables changed shape and this \
         test is no longer reading them: {found:?}",
        found.len()
    );
    assert!(
        found.values().filter(|w| **w).count() > 10,
        "no row parsed as carrying the `*` marker, so this is not reading the cell it thinks \
         it is: {found:?}"
    );
    found
}

/// A command that writes says so on the page, and one that does not says nothing.
///
/// The third place #831's defect turned up. `yidam vault-status` writes a tracked file with a
/// name one character from a read-only command; `decisions-log` writes one and was documented
/// *"Read-only."*. Each surface that could have said so had its own copy of the answer — the
/// command's `about`, the `*` in `--help-all`, this page's marker — and nothing held them to each
/// other, so all three could disagree and two of them did.
///
/// Both sides are rendered output, not source: `--help-all` as a reader sees it, the page as a
/// reader reads it.
#[test]
fn the_page_marks_the_writers_the_binary_marks() {
    let binary = common::writers_from_help();
    let page = writers_from_reference();

    let mut wrong: Vec<String> = Vec::new();
    for (name, &writes) in &binary {
        // A command the page omits is the other tests' finding, not this one's.
        let Some(&documented) = page.get(name) else {
            continue;
        };
        if documented != writes {
            wrong.push(format!(
                "{name}: --help says {}, the page says {}",
                if writes { "writes" } else { "reads" },
                if documented { "writes" } else { "reads" }
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "docs/cli-reference.md disagrees with `yidam --help-all` about which commands write:\n  \
         {}\nOne of the two is wrong about a command that modifies the repository.",
        wrong.join("\n  ")
    );
}

#[test]
fn every_command_the_binary_offers_is_in_the_reference() {
    let built = common::commands_from_help();
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
    let built = common::commands_from_help();
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
