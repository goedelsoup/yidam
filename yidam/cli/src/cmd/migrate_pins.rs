//! `yidam migrate pins` — a read dated in prose becomes a pinned `commit:` (RFC-0051 §7, #1527).
//!
//! Before RFC-0051 a catalog entry dated its read of code in a sentence: "Read at commit
//! 4f5a52d", "Read again at c7decf31". A sentence cannot be checked, and a commit in a sibling
//! repository named in one reads as a dead reference. The pin can be checked. This writes the
//! pins the prose already states:
//!
//! ```yaml
//! location:                            location:
//!   - kind: file                         - kind: file
//!     value: crates/binding/src/lib.rs     value: crates/binding/src/lib.rs
//!                                   →      commit: 4f5a52d…(all 40 digits)
//! ```
//!
//! For each `kind: file` location with no `commit`, the commit is the **last** "read at",
//! "read at commit" or "read again at" in the entry's body, so an entry re-read since its first
//! read is pinned at the latest one. "Re-read at" is the same phrase, so it is read too. The
//! commit is kept only if it resolves in this repository, or else in exactly one `path`
//! dependency, which becomes `repo:`, and only if `value` exists at it. It is written expanded
//! to all 40 digits (Decision 4).
//!
//! # What it will not guess
//!
//! A body that names no commit, a commit that resolves nowhere or in two dependencies, and a
//! path that is not at the commit are each listed with the reason and left unpinned. **The
//! prose is not touched**: the sentence that dated the read stays as written, and node prose is
//! not read at all. `prose-commit-unpinned` lists the sentences that name a sibling's commit,
//! and rewriting one is an author's edit.
//!
//! Every pin is one or two inserted lines below the location's `value:`, at its column, and each
//! entry's insertions are checked before anything is written: the frontmatter is parsed with
//! them applied, and every location must read back as it was, with the pinned ones gaining
//! exactly the `commit` and `repo` they were meant to. An entry that does not is reported and
//! left as it was.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use super::lint::pins::{resolve, Lookup, Query};
use super::migrate::{file_eol, raw_scalar_on, rel, settle_ends, unquoted, MigrateReport};
use super::rename::{join_lines, split_lines, Edit, Unhandled};
use crate::parse::parse_frontmatter;
use crate::paths::yidam_catalog_dir;
use crate::walk::walk_md_files;

const COMMIT: &str = "commit";
const REPO: &str = "repo";

/// Plan the pins for every catalog entry.
pub(crate) fn plan(root: &Path, report: &mut MigrateReport) {
    let mut dated = Vec::new();
    for path in walk_md_files(&yidam_catalog_dir(root)) {
        // README.md is `catalog-audit`'s REGEN target, not a source.
        if path.file_name().is_some_and(|n| n == "README.md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file = rel(root, &path);
        match scan(&file, &text) {
            Scan::Nothing => {}
            Scan::Skipped(u) => report.unhandled.extend(u),
            Scan::Dated(entry, u) => {
                report.unhandled.extend(u);
                dated.push((entry, text));
            }
        }
    }
    let Some(pins) = locate(root, &dated, report) else {
        return;
    };
    for ((entry, text), pins) in dated.iter().zip(pins) {
        let edits: Vec<Edit> = pins
            .into_iter()
            .flat_map(|(line, commit, repo)| {
                let file = entry.file.clone();
                std::iter::once(Edit {
                    file: file.clone(),
                    line,
                    from: String::new(),
                    to: format!("{COMMIT}: {commit}"),
                })
                .chain(repo.map(|r| Edit {
                    file,
                    line,
                    from: String::new(),
                    to: format!("{REPO}: {r}"),
                }))
            })
            .collect();
        if edits.is_empty() {
            continue;
        }
        if !reads_back(text, &edits) {
            report.unhandled.push(Unhandled {
                file: entry.file.clone(),
                line: 1,
                text: format!(
                    "{} pin(s) whose insertion does not read back as written; left as it was",
                    pinned_in(&edits)
                ),
            });
            continue;
        }
        report.edits.extend(edits);
    }
}

/// How many locations a plan pins: one `commit:` line each.
pub(crate) fn pinned(report: &MigrateReport) -> usize {
    pinned_in(&report.edits)
}

fn pinned_in(edits: &[Edit]) -> usize {
    edits
        .iter()
        .filter(|e| e.to.starts_with("commit: "))
        .count()
}

/// Write every planned pin, and return the entries written.
///
/// Its own apply, as `routes` and `scaffold` have: a pin inserts lines, and the shared loop's
/// edit replaces a span inside one. Each insertion is re-located against the `value:` line it
/// was planned below, so an entry edited between the plan and the apply is not corrupted.
pub(crate) fn apply(root: &Path, report: &MigrateReport) -> Result<Vec<String>> {
    let mut by_file: BTreeMap<&str, Vec<&Edit>> = BTreeMap::new();
    for e in &report.edits {
        by_file.entry(&e.file).or_default().push(e);
    }
    let mut written = Vec::new();
    for (file, edits) in by_file {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path)?;
        let Some(out) = insert(&text, &edits) else {
            continue;
        };
        std::fs::write(&path, out)?;
        written.push(file.to_string());
    }
    Ok(written)
}

// ── reading an entry ────────────────────────────────────────────────────────────

/// A `kind: file` location with no `commit`, as the line scan found it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Unpinned {
    /// 1-based line of its `value:`, which the pin is inserted below.
    value_line: usize,
    /// 1-based line of its `kind:`, which a report names.
    kind_line: usize,
    /// The path, from the repository's top level.
    path: String,
}

/// An entry with unpinned `file` locations and the commit its body last dates a read at.
#[derive(Debug)]
struct Dated {
    file: String,
    commit: String,
    locations: Vec<Unpinned>,
}

#[derive(Debug)]
enum Scan {
    /// No unpinned `file` location: nothing to do and nothing to say.
    Nothing,
    /// Unpinned locations this cannot pin, each with the reason.
    Skipped(Vec<Unhandled>),
    /// Unpinned locations and a commit to look up, with any it reported along the way.
    Dated(Dated, Vec<Unhandled>),
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The column a `key:` on this line starts at, past any `- `.
fn key_column(line: &str) -> usize {
    indent(line)
        + if line.trim_start().starts_with("- ") {
            2
        } else {
            0
        }
}

/// Read one entry: its unpinned `file` locations, and the commit its body dates.
fn scan(file: &str, text: &str) -> Scan {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Scan::Nothing;
    }
    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
    else {
        return Scan::Nothing;
    };
    let skip = |line: usize, why: String| Unhandled {
        file: file.to_string(),
        line,
        text: why,
    };
    let mut skipped = Vec::new();
    let mut found = Vec::new();
    let mut seen = 0;
    for i in 1..close {
        let Some((_, _, kind_raw)) = raw_scalar_on(lines[i], "kind") else {
            continue;
        };
        if unquoted(&kind_raw).unwrap_or(&kind_raw) != "file" {
            continue;
        }
        // The list item this `kind:` belongs to, as `migrate locations` finds it.
        let col = key_column(lines[i]);
        let item_start = if lines[i].trim_start().starts_with("- ") {
            Some(i)
        } else {
            (1..i)
                .rev()
                .find(|&j| lines[j].trim_start().starts_with("- ") && key_column(lines[j]) == col)
        };
        let Some(item_start) = item_start else {
            continue;
        };
        let item_end = (item_start + 1..close)
            .find(|&j| !lines[j].trim().is_empty() && indent(lines[j]) < col)
            .unwrap_or(close);
        let at_col = |key: &str| {
            (item_start..item_end).find_map(|j| {
                (key_column(lines[j]) == col)
                    .then(|| raw_scalar_on(lines[j], key))
                    .flatten()
                    .map(|(_, _, v)| (j, v))
            })
        };
        if at_col(COMMIT).is_some() {
            continue;
        }
        seen += 1;
        let Some((j, value_raw)) = at_col("value") else {
            skipped.push(skip(
                i + 1,
                "a `file` location with no `value` to pin".into(),
            ));
            continue;
        };
        if matches!(value_raw.chars().next(), Some('|' | '>')) {
            skipped.push(skip(
                j + 1,
                "a `file` location with a block-scalar `value`".into(),
            ));
            continue;
        }
        let raw = unquoted(&value_raw).unwrap_or(&value_raw).trim();
        let path = raw.strip_prefix("./").unwrap_or(raw).trim_end_matches('/');
        if path.is_empty() || path.starts_with('/') || path.split('/').any(|s| s == "..") {
            skipped.push(skip(
                j + 1,
                format!("`value: {raw}` is not a path from the repository's top level"),
            ));
            continue;
        }
        found.push(Unpinned {
            value_line: j + 1,
            kind_line: i + 1,
            path: path.to_string(),
        });
    }

    // An unpinned `file` location the line scan did not see — a flow mapping, say — is
    // reported, not lost.
    let written = parse_frontmatter(text)
        .location
        .unwrap_or_default()
        .iter()
        .filter(|l| l.kind.as_deref() == Some("file") && l.commit.is_none())
        .count();
    if written > seen {
        skipped.push(skip(
            1,
            format!(
                "{} `file` location(s) written in a form this does not pin",
                written - seen
            ),
        ));
    }
    if found.is_empty() {
        return if skipped.is_empty() {
            Scan::Nothing
        } else {
            Scan::Skipped(skipped)
        };
    }
    let body = lines[close + 1..].join("\n");
    let Some(commit) = last_dated(&body) else {
        skipped.extend(found.iter().map(|u| {
            skip(
                u.kind_line,
                format!(
                    "`{}`: the body dates no read (\"read at <commit>\" or \"read again at \
                     <commit>\")",
                    u.path
                ),
            )
        }));
        return Scan::Skipped(skipped);
    };
    Scan::Dated(
        Dated {
            file: file.to_string(),
            commit,
            locations: found,
        },
        skipped,
    )
}

/// The commit of the last "read at", "read at commit" or "read again at" in `body`, lowercased:
/// 7 to 40 hex digits, at least one a digit so an English word is not read as one, optionally
/// in backticks.
fn last_dated(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut last = None;
    let mut from = 0;
    while let Some(k) = lower[from..].find("read") {
        let i = from + k;
        from = i + 4;
        if i > 0 && word(bytes[i - 1]) {
            continue;
        }
        let mut rest = &lower[i + 4..];
        let Some(r) = spaced(rest) else { continue };
        rest = r;
        if let Some(r) = rest.strip_prefix("again").and_then(spaced) {
            rest = r;
        }
        let Some(r) = rest.strip_prefix("at").and_then(spaced) else {
            continue;
        };
        rest = r;
        if let Some(r) = rest.strip_prefix("commit").and_then(spaced) {
            rest = r;
        }
        let rest = rest.strip_prefix('`').unwrap_or(rest);
        let len = rest.bytes().take_while(u8::is_ascii_hexdigit).count();
        let hex = &rest[..len];
        let bounded = rest.as_bytes().get(len).is_none_or(|b| !word(*b));
        if bounded && (7..=40).contains(&len) && hex.bytes().any(|b| b.is_ascii_digit()) {
            last = Some(hex.to_string());
        }
    }
    last
}

/// `s` past the whitespace that must start it, or `None` when none does.
fn spaced(s: &str) -> Option<&str> {
    let t = s.trim_start();
    (t.len() < s.len()).then_some(t)
}

// ── resolving ───────────────────────────────────────────────────────────────────

/// One `(value line, full commit, repo)` per location to pin, per entry, in `dated`'s order.
///
/// One git process for this repository and one per `path` dependency that a commit missing
/// here is looked for in. `None`, with the reason in `blocked`, when git cannot read this
/// repository at all.
#[allow(clippy::type_complexity)]
fn locate(
    root: &Path,
    dated: &[(Dated, String)],
    report: &mut MigrateReport,
) -> Option<Vec<Vec<(usize, String, Option<String>)>>> {
    let queries = |entries: &[&Dated]| -> Vec<Query> {
        entries
            .iter()
            .flat_map(|d| {
                d.locations.iter().map(|u| Query {
                    commit: d.commit.clone(),
                    path: u.path.clone(),
                    contents: false,
                })
            })
            .collect()
    };
    let all: Vec<&Dated> = dated.iter().map(|(d, _)| d).collect();
    let Some(here) = resolve(root, &queries(&all)) else {
        report
            .blocked
            .push("git cannot read this repository, so no commit can be resolved".into());
        return None;
    };
    let mut out: Vec<Vec<(usize, String, Option<String>)>> = vec![Vec::new(); dated.len()];
    let say = |report: &mut MigrateReport, d: &Dated, u: &Unpinned, why: String| {
        report.unhandled.push(Unhandled {
            file: d.file.clone(),
            line: u.kind_line,
            text: format!("`{}`: {why}", u.path),
        });
    };

    // Here first: a commit this repository has is this repository's.
    let mut missing: Vec<usize> = Vec::new();
    let mut answers = here.into_iter();
    for (n, d) in all.iter().enumerate() {
        let mine: Vec<_> = answers.by_ref().take(d.locations.len()).collect();
        match mine.first().map(|a| &a.commit) {
            Some(Lookup::Found { oid, .. }) => {
                for (u, a) in d.locations.iter().zip(&mine) {
                    if matches!(a.object, Lookup::Found { .. }) {
                        out[n].push((u.value_line, oid.clone(), None));
                    } else {
                        say(
                            report,
                            d,
                            u,
                            format!("not in this repository at {}", d.commit),
                        );
                    }
                }
            }
            Some(Lookup::Ambiguous) => {
                for u in &d.locations {
                    say(
                        report,
                        d,
                        u,
                        format!("`{}` names more than one object here", d.commit),
                    );
                }
            }
            _ => missing.push(n),
        }
    }
    if missing.is_empty() {
        return Some(out);
    }

    // Then every `path` dependency, each asked only for the commits missing here.
    // A file that does not parse would ask no dependency, and every commit only a dependency
    // has would be reported as not in this repository — an edit planned on a misreading.
    let config = match crate::deps::load_config(&crate::paths::tonpa_config_path(root)) {
        Ok(config) => config,
        Err(e) => {
            report.blocked.push(format!("{e:#}"));
            return None;
        }
    };
    let asked: Vec<&Dated> = missing.iter().map(|&n| all[n]).collect();
    let qs = queries(&asked);
    // Per missing entry: the dependencies whose commit it is, with each location's answer.
    let mut owners: Vec<Vec<(String, String, Vec<bool>)>> = vec![Vec::new(); missing.len()];
    for (name, dep) in &config.dependencies {
        let Some(path) = &dep.path else { continue };
        let Some(there) = resolve(&root.join(path), &qs) else {
            continue;
        };
        let mut answers = there.into_iter();
        for (m, d) in asked.iter().enumerate() {
            let theirs: Vec<_> = answers.by_ref().take(d.locations.len()).collect();
            if let Some(Lookup::Found { oid, .. }) = theirs.first().map(|a| &a.commit) {
                let objects = theirs
                    .iter()
                    .map(|a| matches!(a.object, Lookup::Found { .. }))
                    .collect();
                owners[m].push((name.clone(), oid.clone(), objects));
            }
        }
    }
    let shallow = crate::git::is_shallow(root);
    for ((found, &n), d) in owners.into_iter().zip(&missing).zip(&asked) {
        match found.as_slice() {
            [(name, oid, objects)] => {
                for (u, here) in d.locations.iter().zip(objects) {
                    if *here {
                        out[n].push((u.value_line, oid.clone(), Some(name.clone())));
                    } else {
                        say(report, d, u, format!("not in `{name}` at {}", d.commit));
                    }
                }
            }
            [] => {
                let why = if shallow {
                    format!(
                        "{} is in no `path` dependency, and this clone is shallow, so it may \
                         be here and unseen",
                        d.commit
                    )
                } else {
                    format!(
                        "{} is in neither this repository nor any `path` dependency in \
                         `.yidam/tonpa.toml`",
                        d.commit
                    )
                };
                for u in &d.locations {
                    say(report, d, u, why.clone());
                }
            }
            several => {
                let names: Vec<String> = several.iter().map(|(n, _, _)| format!("`{n}`")).collect();
                for u in &d.locations {
                    say(
                        report,
                        d,
                        u,
                        format!(
                            "{} is in {}; which one was read is the author's to say",
                            d.commit,
                            names.join(" and ")
                        ),
                    );
                }
            }
        }
    }
    Some(out)
}

// ── writing ─────────────────────────────────────────────────────────────────────

/// `text` with `edits` inserted, each below its `value:` line at that key's column; `None` when
/// a line an edit names is no longer a `value:`.
fn insert(text: &str, edits: &[&Edit]) -> Option<String> {
    let mut lines = split_lines(text);
    let eol = file_eol(&lines);
    let mut below: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for e in edits {
        below.entry(e.line).or_default().push(&e.to);
    }
    // From the bottom up, so each line number still names the line it was planned against.
    for (line, adds) in below.into_iter().rev() {
        let (anchor, _) = lines.get(line.checked_sub(1)?)?;
        raw_scalar_on(anchor, "value")?;
        let pad = " ".repeat(key_column(anchor));
        for (k, add) in adds.into_iter().enumerate() {
            lines.insert(line + k, (format!("{pad}{add}"), eol));
        }
    }
    settle_ends(&mut lines, eol, text.ends_with('\n'));
    Some(join_lines(&lines))
}

/// Whether `text` with `edits` inserted parses to what they meant: every location as it was,
/// the pinned ones gaining exactly the `commit` and `repo` planned for them.
fn reads_back(text: &str, edits: &[Edit]) -> bool {
    let Some(after) = insert(text, &edits.iter().collect::<Vec<_>>()) else {
        return false;
    };
    let before = parse_frontmatter(text).location.unwrap_or_default();
    let after = parse_frontmatter(&after).location.unwrap_or_default();
    if before.len() != after.len() {
        return false;
    }
    let meant: Vec<(String, Option<String>)> = {
        let mut by_line: BTreeMap<usize, (String, Option<String>)> = BTreeMap::new();
        for e in edits {
            let slot = by_line.entry(e.line).or_default();
            if let Some(c) = e.to.strip_prefix("commit: ") {
                slot.0 = c.to_string();
            } else if let Some(r) = e.to.strip_prefix("repo: ") {
                slot.1 = Some(r.to_string());
            }
        }
        by_line.into_values().collect()
    };
    let mut pinned = Vec::new();
    for (b, a) in before.iter().zip(&after) {
        if b.kind != a.kind
            || b.value != a.value
            || b.description != a.description
            || b.symbol != a.symbol
            || b.members != a.members
        {
            return false;
        }
        if b.commit.is_some() || a.commit.is_none() {
            if b.commit != a.commit || b.repo != a.repo {
                return false;
            }
            continue;
        }
        if b.repo.is_some() && b.repo != a.repo {
            return false;
        }
        pinned.push((a.commit.clone().unwrap_or_default(), a.repo.clone()));
    }
    pinned == meant
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{commit, git_out, init, write};
    use std::path::PathBuf;

    #[test]
    fn the_last_dated_read_is_the_one_kept() {
        let body = "Read at commit 4f5a52d, where it moved.\nAt 6fabd52 it adds x.\n\
                    Re-read at e6ba169b. Read again at `C7DECF31`, the re-pin.";
        assert_eq!(last_dated(body).as_deref(), Some("c7decf31"));
        assert_eq!(
            last_dated("read at 4f5a52d and again at 9ab98070").as_deref(),
            Some("4f5a52d")
        );
    }

    #[test]
    fn a_word_or_a_bare_at_is_not_a_dated_read() {
        // "At <hex>" dates a change, not this entry's read; "unread at" is not "read at";
        // `defaced` is hex but no commit; nine digits run into a word.
        for body in [
            "At 6fabd52 it adds x.",
            "unread at 4f5a52d",
            "read at defaced",
            "read at 4f5a52dz",
            "read at 4f5a5",
            "readat 4f5a52d",
        ] {
            assert_eq!(last_dated(body), None, "{body}");
        }
    }

    fn entry(location: &str, body: &str) -> String {
        format!("---\nname: x\ntype: document\nlocation:\n{location}---\n\n{body}\n")
    }

    fn dated(text: &str) -> Dated {
        match scan("e.md", text) {
            Scan::Dated(d, skipped) => {
                assert!(skipped.is_empty(), "{skipped:?}");
                d
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_unpinned_file_location_is_read_and_a_pinned_one_is_not() {
        let text = entry(
            "  - kind: file\n    value: ./src/a.rs\n  - value: src/b.rs\n    kind: file\n  \
             - kind: file\n    value: src/c.rs\n    commit: 1111111\n  - kind: url\n    \
             value: https://x.org\n",
            "Read at 4f5a52d.",
        );
        let d = dated(&text);
        assert_eq!(d.commit, "4f5a52d");
        let paths: Vec<&str> = d.locations.iter().map(|u| u.path.as_str()).collect();
        assert_eq!(paths, ["src/a.rs", "src/b.rs"]);
        assert_eq!(d.locations[1].value_line, 7);
    }

    #[test]
    fn a_body_that_dates_nothing_is_listed() {
        let text = entry("  - kind: file\n    value: src/a.rs\n", "It reads well.");
        let Scan::Skipped(s) = scan("e.md", &text) else {
            panic!()
        };
        assert_eq!(s.len(), 1);
        assert!(s[0].text.contains("dates no read"), "{}", s[0].text);
        // An entry with no unpinned `file` location has nothing to say.
        let text = entry(
            "  - kind: url\n    value: https://x.org\n",
            "It reads well.",
        );
        assert!(matches!(scan("e.md", &text), Scan::Nothing));
    }

    #[test]
    fn a_flow_mapping_is_reported_and_not_lost() {
        let text = entry("  - {kind: file, value: src/a.rs}\n", "Read at 4f5a52d.");
        let Scan::Skipped(s) = scan("e.md", &text) else {
            panic!()
        };
        assert!(s[0].text.contains("does not pin"), "{}", s[0].text);
    }

    fn pin(line: usize, commit: &str, repo: Option<&str>) -> Vec<Edit> {
        let mut v = vec![Edit {
            file: "e.md".into(),
            line,
            from: String::new(),
            to: format!("commit: {commit}"),
        }];
        if let Some(r) = repo {
            v.push(Edit {
                file: "e.md".into(),
                line,
                from: String::new(),
                to: format!("repo: {r}"),
            });
        }
        v
    }

    #[test]
    fn a_pin_is_inserted_below_its_value_and_reads_back() {
        let full = "4f5a52d".to_string() + &"0".repeat(33);
        let text = entry(
            "  - kind: file\n    value: src/a.rs\n    description: a\n  - value: src/b.rs\n    \
             kind: file\n",
            "Read at 4f5a52d.",
        );
        let mut edits = pin(6, &full, None);
        edits.extend(pin(8, &full, Some("sib")));
        assert!(reads_back(&text, &edits));
        let out = insert(&text, &edits.iter().collect::<Vec<_>>()).unwrap();
        assert!(
            out.contains(&format!(
                "  - kind: file\n    value: src/a.rs\n    commit: {full}\n    description: a\n"
            )),
            "{out}"
        );
        assert!(
            out.contains(&format!(
                "  - value: src/b.rs\n    commit: {full}\n    repo: sib\n    kind: file\n"
            )),
            "{out}"
        );
        assert!(
            out.ends_with("---\n\nRead at 4f5a52d.\n"),
            "the prose moved"
        );
        // A line that is no longer a `value:` refuses rather than inserting somewhere else.
        assert!(insert(&text, &pin(5, &full, None).iter().collect::<Vec<_>>()).is_none());
        assert!(!reads_back(&text, &pin(5, &full, None)));
    }

    // ── against repositories ──

    fn repo(dir: &Path, files: &[(&str, &str)]) -> String {
        std::fs::create_dir_all(dir).unwrap();
        init(dir);
        for (p, t) in files {
            write(dir, p, t);
        }
        commit(dir, "seed");
        git_out(dir, &["rev-parse", "HEAD"]).trim().to_string()
    }

    fn planned(here: &Path) -> MigrateReport {
        let mut r = MigrateReport::new(here, here, &super::super::migrate::Operation::Pins);
        plan(here, &mut r);
        r
    }

    /// `here` and two `path` dependencies, each with one commit, and each one's id.
    fn three(tmp: &Path) -> (PathBuf, String, String, String) {
        let here = tmp.join("here");
        let ours = repo(&here, &[("src/a.rs", "a"), (".yidam/tonpa.toml",
            "[dependencies.sib]\npath = \"../sib\"\n\n[dependencies.twin]\npath = \"../twin\"\n")]);
        let sib = repo(&tmp.join("sib"), &[("lib/b.rs", "b")]);
        let twin = repo(&tmp.join("twin"), &[("lib/c.rs", "c")]);
        (here, ours, sib, twin)
    }

    fn catalog(here: &Path, name: &str, location: &str, body: &str) {
        write(
            here,
            &format!(".yidam/catalog/{name}.md"),
            &entry(location, body),
        );
    }

    #[test]
    fn a_commit_is_kept_here_or_in_the_one_dependency_that_has_it() {
        let tmp = tempfile::tempdir().unwrap();
        let (here, ours, sib, _) = three(tmp.path());
        let file = |v: &str| format!("  - kind: file\n    value: {v}\n");
        catalog(
            &here,
            "mine",
            &file("src/a.rs"),
            &format!("Read at commit {}.", &ours[..8]),
        );
        catalog(
            &here,
            "theirs",
            &file("lib/b.rs"),
            &format!("Read again at {}.", &sib[..9]),
        );
        catalog(&here, "gone", &file("src/a.rs"), "Read at 1234567.");
        catalog(
            &here,
            "moved",
            &file("src/z.rs"),
            &format!("read at {}", &ours[..7]),
        );
        catalog(&here, "undated", &file("src/a.rs"), "Read last spring.");

        let r = planned(&here);
        assert!(r.blocked.is_empty(), "{:?}", r.blocked);
        let to: Vec<(&str, &str)> = r
            .edits
            .iter()
            .map(|e| (e.file.as_str(), e.to.as_str()))
            .collect();
        assert_eq!(
            to,
            [
                (".yidam/catalog/mine.md", format!("commit: {ours}").as_str()),
                (
                    ".yidam/catalog/theirs.md",
                    format!("commit: {sib}").as_str()
                ),
                (".yidam/catalog/theirs.md", "repo: sib"),
            ]
        );
        let why: BTreeMap<&str, &str> = r
            .unhandled
            .iter()
            .map(|u| (u.file.as_str(), u.text.as_str()))
            .collect();
        assert!(
            why[".yidam/catalog/gone.md"].contains("in neither"),
            "{why:?}"
        );
        assert!(
            why[".yidam/catalog/moved.md"].contains("not in this repository"),
            "{why:?}"
        );
        assert!(
            why[".yidam/catalog/undated.md"].contains("dates no read"),
            "{why:?}"
        );
    }

    #[test]
    fn a_commit_two_dependencies_share_is_not_guessed() {
        let tmp = tempfile::tempdir().unwrap();
        let (here, _, sib, _) = three(tmp.path());
        // `twin` fetches `sib`'s history, so the commit is in both.
        git_out(&tmp.path().join("twin"), &["fetch", "-q", "../sib", "HEAD"]);
        catalog(
            &here,
            "both",
            "  - kind: file\n    value: lib/b.rs\n",
            &format!("Read at {sib}."),
        );
        let r = planned(&here);
        assert!(r.edits.is_empty(), "{:?}", r.edits);
        assert!(
            r.unhandled[0].text.contains("`sib` and `twin`"),
            "{}",
            r.unhandled[0].text
        );
    }

    /// A CRLF entry is written back CRLF, the inserted line with it (#1693). The insert used
    /// to split with `lines()` and join with `"\n"`, so one pin rewrote every line of it.
    #[test]
    fn a_pin_keeps_a_crlf_entrys_line_endings() {
        let full = "4f5a52d".to_string() + &"0".repeat(33);
        let text = entry(
            "  - kind: file\n    value: src/a.rs\n    description: a\n",
            "Read at 4f5a52d.",
        )
        .replace('\n', "\r\n");
        let out = insert(&text, &pin(6, &full, None).iter().collect::<Vec<_>>()).unwrap();
        let expect = text.replace(
            "    value: src/a.rs\r\n",
            &format!("    value: src/a.rs\r\n    commit: {full}\r\n"),
        );
        assert_eq!(out, expect);
        assert_eq!(out.matches('\n').count(), out.matches("\r\n").count());
    }

    /// A file with both endings keeps each line's own, and the inserted line takes the first
    /// line's, as `rename`'s `moved-from:` line does (#1693). An LF file is byte for byte as
    /// before, with or without a last newline.
    #[test]
    fn a_pin_in_a_mixed_or_unterminated_file_keeps_each_lines_ending() {
        let full = "4f5a52d".to_string() + &"0".repeat(33);
        let edits = pin(2, &full, None);
        let edits: Vec<&Edit> = edits.iter().collect();
        let mixed = "---\r\n  - value: a\nkind: file\r\n";
        assert_eq!(
            insert(mixed, &edits).unwrap(),
            format!("---\r\n  - value: a\n    commit: {full}\r\nkind: file\r\n")
        );
        let bare = "---\n  - value: a";
        assert_eq!(
            insert(bare, &edits).unwrap(),
            format!("---\n  - value: a\n    commit: {full}")
        );
    }

    /// Line 0 is no line, so it refuses rather than computing `0 - 1` (#1693).
    #[test]
    fn a_pin_at_line_zero_refuses() {
        let full = "4f5a52d".to_string() + &"0".repeat(33);
        let text = "---\n  - value: a\n";
        assert!(insert(text, &pin(0, &full, None).iter().collect::<Vec<_>>()).is_none());
    }
}
