//! An RFC's status is written twice — once in its own header, once in the README's index
//! table row — and nothing before this file compared them.
//!
//! #483 found all 27 RFCs reading `Status: Draft`, including nine whose command already ships.
//! The fix is not a mass rewrite (a `Draft` that should read `Accepted` and one that should read
//! `Implemented` are different claims, and telling them apart is a per-RFC judgement, not a sed)
//! but a guard against the failure mode #483 named: "two copies of one fact with nothing holding
//! them together... it will drift the first time somebody updates one and not the other." A
//! reader who opens the RFC sees one status; a reader who only reads the index sees the other;
//! nothing before this file could tell them apart.
//!
//! Both sides are **discovered**, not enumerated: the RFC set comes from `docs/rfcs/*.md`
//! (README.md excluded by the filename pattern, not by name), and the legal status vocabulary
//! comes from the "Status legend" section of the README itself. A new RFC, a renamed one, or a
//! legend addition all take effect the moment the file changes — never a second list to remember
//! to update alongside them.

use std::collections::BTreeMap;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rfcs_dir() -> PathBuf {
    repo_root().join("docs/rfcs")
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()))
}

/// Every `NNNN-*.md` file under `docs/rfcs/`, keyed by its four-digit number.
///
/// The pattern is the same one the index table's own links use
/// (`[0001](0001-report-contract.md)`) — a real RFC file, not `README.md` or the template
/// fragment inside it, which lives in a fenced code block and carries no filename of its own.
fn discover_rfc_files() -> BTreeMap<String, PathBuf> {
    let mut found = BTreeMap::new();
    for entry in std::fs::read_dir(rfcs_dir())
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", rfcs_dir().display()))
    {
        let entry = entry.expect("directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_numbered_rfc = name.len() >= 5
            && name.as_bytes()[..4].iter().all(u8::is_ascii_digit)
            && name.as_bytes()[4] == b'-'
            && name.ends_with(".md");
        if is_numbered_rfc {
            found.insert(name[..4].to_string(), entry.path());
        }
    }
    assert!(
        found.len() >= 20,
        "found only {} files matching docs/rfcs/NNNN-*.md — the discovery pattern likely broke, \
         not the RFC directory shrinking that far",
        found.len()
    );
    found
}

/// The value of the `- **Status:** X` line in an RFC's own header.
///
/// Only the first match counts: RFC-0011 discusses the old universal `Status: Draft` in prose
/// (`` `Status: Draft` `` inside a sentence, no `- **` prefix), which this pattern does not match.
fn header_status(text: &str) -> String {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("- **Status:**"))
        .unwrap_or_else(|| panic!("no `- **Status:**` header line found"))
        .trim()
        .to_string()
}

/// The `Status` column of the README index table, one entry per RFC number.
///
/// Parsed rather than matched line-by-line against a known set of numbers: a row is any table
/// line whose first cell is a Markdown link `[NNNN](...)`, and the number and the last cell are
/// read out of it. A row for an RFC nobody wrote a file for, or a file with no row, both surface
/// as a mismatch between this map and `discover_rfc_files()` rather than silently passing.
fn index_table_statuses(readme: &str) -> BTreeMap<String, String> {
    let mut statuses = BTreeMap::new();
    for line in readme.lines() {
        let line = line.trim();
        if !line.starts_with("| [") {
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let Some(first) = cells.first() else { continue };
        // first cell looks like "[0001](0001-report-contract.md)"
        let Some(number) = first
            .strip_prefix('[')
            .and_then(|s| s.split(']').next())
            .filter(|n| n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()))
        else {
            continue;
        };
        let Some(status) = cells.last() else { continue };
        statuses.insert(number.to_string(), status.trim().to_string());
    }
    statuses
}

/// The status words the "Status legend" section defines, in the order it defines them.
///
/// Read out of the README rather than retyped here — the legend line is the one place this
/// vocabulary is declared, and a fifth status added there should not require a second edit to a
/// test file before it is legal to use.
fn legend_statuses(readme: &str) -> Vec<String> {
    let legend = readme
        .split_once("## Status legend")
        .expect("README.md has no `## Status legend` section")
        .1;
    // Stop at the next heading so the RFC template's own `Status: Draft` line below doesn't leak in.
    let legend = legend.split("\n## ").next().unwrap_or(legend);
    let mut names = Vec::new();
    let mut rest = legend;
    while let Some((_, after)) = rest.split_once('`') {
        let Some((name, after)) = after.split_once('`') else {
            break;
        };
        names.push(name.to_string());
        rest = after;
    }
    assert!(
        names.len() >= 5,
        "found only {} backtick-quoted names in the Status legend ({:?}); expected the five \
         states the legend prose lists",
        names.len(),
        names
    );
    names
}

/// The header status and the index-table status agree, for every RFC — discovered, not listed.
///
/// This is the guard #483 asked for first: cheap, and it removes the second-copy hazard
/// outright regardless of what is ever decided about the vocabulary itself. Mutate one file's
/// `Status:` line without touching the other and this test is what turns red.
#[test]
fn header_and_index_status_agree_for_every_rfc() {
    let files = discover_rfc_files();
    let readme = read(&repo_root().join("docs/rfcs/README.md"));
    let index = index_table_statuses(&readme);

    let file_numbers: Vec<&String> = files.keys().collect();
    let index_numbers: Vec<&String> = index.keys().collect();
    assert_eq!(
        file_numbers, index_numbers,
        "docs/rfcs/README.md's index table and the docs/rfcs/ directory name different RFC sets \
         — a file with no index row, or an index row with no file, either way one copy of the \
         set is stale"
    );

    let mut disagreements = Vec::new();
    for (number, path) in &files {
        let header = header_status(&read(path));
        let row = index
            .get(number)
            .unwrap_or_else(|| panic!("no index row for RFC-{number}"));
        if &header != row {
            disagreements.push(format!(
                "RFC-{number}: header says `{header}`, README index says `{row}`"
            ));
        }
    }

    assert!(
        disagreements.is_empty(),
        "header and index-table status disagree for {} RFC(s):\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}

/// Every header status is a word the legend actually defines.
///
/// Catches a typo (`Implemeted`) or a status invented ad hoc that the legend never explains —
/// the same silent-drift shape as the agreement check above, one layer earlier: an index and a
/// header can agree with each other and still both be a word nobody defined.
#[test]
fn every_header_status_is_a_legend_status() {
    let readme = read(&repo_root().join("docs/rfcs/README.md"));
    let legend = legend_statuses(&readme);

    for (number, path) in discover_rfc_files() {
        let header = header_status(&read(&path));
        assert!(
            legend.contains(&header),
            "RFC-{number}'s header status `{header}` is not one of the legend's {legend:?}"
        );
    }
}
