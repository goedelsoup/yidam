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
//!
//! ## And the status against the binary (#941)
//!
//! Two copies agreeing with each other is still two copies of a claim nothing outside the
//! documents can check. #941 found three RFCs reading `Draft` or `Accepted` while the commands
//! they specify were in `main.rs` and in a cut release — RFC-0026's `run` and `phase` shipped
//! under a release commit that names the phase surface in its own subject line, and the RFC
//! above it still said the design was under review.
//!
//! What made that undecidable is that an RFC's commands were only ever in its prose, and prose
//! names commands it does not specify: every RFC here mentions `lint` or `vault` somewhere, and
//! a scan over those would flag two thirds of the directory. So an RFC that specifies commands
//! now declares them in its header, as a `- **Commands:**` bullet, and the checks below
//! read that line against the built binary's own `--help-all`. The README's "The Commands line"
//! section is the rule those checks enforce.
//!
//! The gate is one-directional by construction: it fires on a status that lags behind a shipped
//! command, and it cannot fire on an RFC that specifies a command and declines to say so. That
//! hole is real and is not closable from here — which command an RFC *specifies*, as against
//! mentions, is the judgement the line exists to record. What holds it is review, plus the
//! population floor below and the title check, which takes the one case the documents make
//! decidable: an RFC advertising a command in its own H1 has to declare it.

mod common;

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

/// The commands an RFC's header declares it specifies, or `None` where it declares none.
///
/// The line reads `- **Commands:**` followed by top-level names as `--help-all` spells them,
/// comma-separated, backticks optional. Nothing else may be on it — a trailing clause would be
/// parsed as a command name, and `every_declared_command_is_spelled_like_a_command` rejects it
/// rather than ignoring it, because a name this file cannot recognise is a name it cannot check
/// the status against.
///
/// `None` and an empty list are different answers and both are kept: `None` is an RFC that
/// specifies no command, which is two thirds of the directory, and an empty list is a line
/// somebody left blank.
fn declared_commands(text: &str) -> Option<Vec<String>> {
    let value = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("- **Commands:**"))?;
    Some(
        value
            .split(',')
            .map(|c| c.trim().trim_matches('`').trim().to_string())
            .filter(|c| !c.is_empty())
            .collect(),
    )
}

/// Every RFC that declares commands, keyed by number.
fn rfcs_declaring_commands() -> BTreeMap<String, (PathBuf, Vec<String>)> {
    discover_rfc_files()
        .into_iter()
        .filter_map(|(number, path)| {
            let commands = declared_commands(&read(&path))?;
            Some((number, (path, commands)))
        })
        .collect()
}

/// The commands an RFC advertises in its own H1, as `` `yidam <name>` ``.
///
/// Five titles do — `yidam sync`, `yidam log --epistemic`, `yidam query`, `yidam propose`,
/// `yidam check-diff`. RFC-0016's bare `` `yidam` `` names no command and does not match, which
/// is the distinction the leading space carries: a backticked `yidam` with nothing after it is
/// the tool, not a subcommand of it.
fn commands_in_title(text: &str) -> Vec<String> {
    let title = text.lines().next().unwrap_or_default();
    let mut found = Vec::new();
    let mut rest = title;
    while let Some((_, after)) = rest.split_once("`yidam ") {
        let (inside, after) = after.split_once('`').unwrap_or((after, ""));
        if let Some(name) = inside.split_whitespace().next() {
            found.push(name.to_string());
        }
        rest = after;
    }
    found
}

/// Commands absent from a `--no-default-features` build, so absence proves nothing about them.
///
/// `tonpa` alone, for `cli_reference.rs`'s reason: it is the one command gated at the clap
/// level, and every other feature-gated path is always present and refuses at runtime.
fn feature_gated(name: &str) -> bool {
    name == "tonpa"
}

/// The scan sees a population, and every name in it is shaped like a command.
///
/// Both halves are the floor under the two checks below, which are assertions over a filtered
/// set and pass vacuously on an empty one. A parser that stopped matching the header line, or a
/// line that grew a trailing clause, would otherwise take the gate quietly out of service.
#[test]
fn every_declared_command_is_spelled_like_a_command() {
    let declaring = rfcs_declaring_commands();
    assert!(
        declaring.len() >= 10,
        "only {} RFC(s) declare a `- **Commands:**` line — thirteen did when #941 landed, and \
         this dropping is the parser breaking rather than the directory shrinking that far",
        declaring.len()
    );

    let built = common::commands_from_help();
    let mut malformed = Vec::new();
    for (number, (_, commands)) in &declaring {
        assert!(
            !commands.is_empty(),
            "RFC-{number}'s `- **Commands:**` line names nothing. Remove the line, or name the \
             commands it specifies."
        );
        for name in commands {
            let looks_like_one = !name.is_empty()
                && name.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                && !name.starts_with('-')
                && !name.ends_with('-');
            if !looks_like_one {
                malformed.push(format!("RFC-{number}: `{name}`"));
            }
        }
    }
    assert!(
        malformed.is_empty(),
        "these entries on a `- **Commands:**` line are not command names:\n  {}\nThe line takes \
         comma-separated top-level names as `yidam --help-all` spells them, and nothing else — a \
         clause explaining them belongs in the RFC's body.",
        malformed.join("\n  ")
    );

    assert!(
        declaring
            .values()
            .any(|(_, c)| c.iter().any(|n| built.contains(n))),
        "no RFC declares a command this binary has, so the check below cannot fire for any \
         reason. Either every declared name is misspelled or the roster came back wrong."
    );
}

/// An RFC whose every declared command exists reads neither `Draft` nor `Accepted`.
///
/// The gate #941 asked for. `Draft` means *under review, not accepted* and `Accepted` means
/// *implementation may begin*; a command in the built binary is neither of those, and the three
/// RFCs that read one anyway had shipped commands under them for between two weeks and a month.
///
/// Every command and not any, deliberately. An RFC that specifies three commands and has built
/// one is mid-implementation, which is exactly what `Accepted` says — RFC-0004's `sync`,
/// `check-drift` and `upgrade` are the standing case, none of them built. The status has to move
/// on the day the last one lands, and this is what says so.
#[test]
fn an_rfc_whose_commands_all_ship_is_not_draft_or_accepted() {
    let built = common::commands_from_help();
    let mut lagging = Vec::new();

    for (number, (path, commands)) in rfcs_declaring_commands() {
        if !commands.iter().all(|c| built.contains(c)) {
            continue;
        }
        let status = header_status(&read(&path));
        if status == "Draft" || status == "Accepted" {
            lagging.push(format!(
                "RFC-{number}: `{status}`, and all of {commands:?} are in this binary"
            ));
        }
    }

    assert!(
        lagging.is_empty(),
        "{} RFC(s) read a status their own command surface has outrun:\n  {}\nThe status is the \
         stale copy — move it to `Implemented` in the header and in docs/rfcs/README.md's index \
         row, or correct the `- **Commands:**` line if it names a command the RFC does not \
         specify.",
        lagging.len(),
        lagging.join("\n  ")
    );
}

/// An `Implemented` RFC declares no command the binary does not have.
///
/// The other direction, and the cheaper mistake: a status moved ahead of the build, or a command
/// renamed out from under a document that claims to have landed it. `tonpa` is excluded because
/// a `--no-default-features` build legitimately does not have it.
#[test]
fn an_implemented_rfc_declares_no_command_the_binary_lacks() {
    let built = common::commands_from_help();
    let mut overclaimed = Vec::new();

    for (number, (path, commands)) in rfcs_declaring_commands() {
        if header_status(&read(&path)) != "Implemented" {
            continue;
        }
        let missing: Vec<&String> = commands
            .iter()
            .filter(|c| !built.contains(*c) && !feature_gated(c))
            .collect();
        if !missing.is_empty() {
            overclaimed.push(format!("RFC-{number}: {missing:?}"));
        }
    }

    assert!(
        overclaimed.is_empty(),
        "these RFCs read `Implemented` and declare commands this binary does not offer:\n  \
         {}\nEither the command was renamed and the header still names the old spelling, or the \
         status moved before the build did.",
        overclaimed.join("\n  ")
    );
}

/// A command an RFC advertises in its own title is a command its header declares.
///
/// The one case the documents themselves make decidable, and so the only guard against the
/// `- **Commands:**` line simply being left off. A title reading *"(`yidam query`)"* has already
/// said what this RFC's command surface is; the header saying it again in a form a test can read
/// costs one line, and is what puts five of the thirteen beyond a reviewer's memory.
#[test]
fn a_command_an_rfc_titles_itself_after_is_declared_in_its_header() {
    let mut undeclared = Vec::new();

    for (number, path) in discover_rfc_files() {
        let text = read(&path);
        let titled = commands_in_title(&text);
        if titled.is_empty() {
            continue;
        }
        let declared = declared_commands(&text).unwrap_or_default();
        for name in titled {
            if !declared.contains(&name) {
                undeclared.push(format!("RFC-{number}: `{name}`"));
            }
        }
    }

    assert!(
        undeclared.is_empty(),
        "these RFCs name a command in their H1 and do not declare it in a `- **Commands:**` \
         header line:\n  {}\nAn RFC that titles itself after a command specifies that command.",
        undeclared.join("\n  ")
    );
}
