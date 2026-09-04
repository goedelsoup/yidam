//! `yidam sangha` — who the electors are, what they argued, and what was settled.
//!
//! Collective mode is the most distinctive thing a derived repository can do and the least
//! visible: the electors live in a markdown table, the arguments in one directory, the
//! settlements in another, and the branch namespace that ties them together
//! (`ma/<elector>`, `rigpa/<evolution>`) is only legible to `git for-each-ref`. Reading any
//! of it means opening four files and holding the naming convention in your head.
//!
//! This report holds the convention instead. It is deliberately a *report* rather than a
//! check: nothing here gates, and the lint checks that do gate on the sangha
//! (`resolution-annotation-*`) stay where they are. What this answers is "who is in this,
//! and what have they said" — which is navigation, and which no command answered.
//!
//! Read-only, and that is a constitutional limit rather than a scoping one. Article V
//! confines synthesis to resolution events, so a tool that *wrote* a position or drafted a
//! resolution would be performing one outside the protocol that routes them.

use anyhow::Result;
use std::path::Path;

use crate::paths::{repo_root, yidam_sangha_dir};

/// A recognized elector: someone maintaining a `ma/<name>` branch.
#[derive(Debug, serde::Serialize)]
pub struct Elector {
    pub name: String,
    pub branch: String,
    /// The `Role` cell, verbatim — markdown and all. It is prose written for a reader.
    pub role: String,
    /// Whether the branch exists, local or remote-tracking.
    ///
    /// A registered elector whose branch is gone is not an error here — it is the state
    /// the file and the refs are in, and reporting it is the point.
    pub branch_present: bool,
}

/// One elector's stated position on one question.
#[derive(Debug, serde::Serialize)]
pub struct Position {
    /// Repository-relative path.
    pub file: String,
    /// The registered elector whose name prefixes the filename, or empty when none does.
    pub elector: String,
    /// The rest of the stem — the `rigpa/<evolution>` a resolution would carry.
    pub question: String,
}

/// One settled resolution record.
#[derive(Debug, serde::Serialize)]
pub struct Resolution {
    pub file: String,
    /// The `evolution:` frontmatter field, falling back to the filename stem.
    pub evolution: String,
    /// The `date:` frontmatter field, or empty.
    pub date: String,
    /// The `ma/*` tips the resolution records having read, verbatim.
    pub tips: Vec<String>,
    /// The elector seat that executed the synthesis — `synthesized-by:`, or empty.
    ///
    /// One field, and a list where a synthesis was genuinely joint. It is the only thing in a
    /// resolution that names a *seat* rather than a branch that was read: in the repository
    /// that has run this protocol, all 126 commits across three elector branches carry the
    /// operator's git author, so nothing else in the record or in git tells the auditor's
    /// position from the owner's.
    pub synthesized_by: Vec<String>,
    /// Whether `rigpa/<evolution>` still exists.
    pub branch_present: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct SanghaReport {
    /// Whether this repository runs collective governance at all.
    ///
    /// Keyed on *registered electors*, not on the directory: the template ships
    /// `sangha/` with a placeholder table, so a directory test would report every derived
    /// repository as collective from its genesis commit.
    pub collective: bool,
    pub electors: Vec<Elector>,
    pub positions: Vec<Position>,
    pub resolutions: Vec<Resolution>,
}

/// One row of a markdown table, cells trimmed of whitespace and pipes.
fn table_cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !t.starts_with('|') {
        return None;
    }
    Some(
        t.trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect(),
    )
}

/// `` `ma/auditor` `` → `ma/auditor`
fn unquote(cell: &str) -> String {
    cell.trim().trim_matches('`').trim().to_string()
}

/// Parse the elector table out of `electors.md`.
///
/// **A row is an elector when its branch cell names a `ma/*` ref**, which is what
/// PROTOCOL.md defines an elector as — not when the row merely exists. That rule is what
/// skips the template's `*(no electors registered yet)*` placeholder without special-casing
/// the placeholder's wording, which a derived repository is free to rewrite.
pub(crate) fn parse_electors(text: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(cells) = table_cells(line) else {
            continue;
        };
        if cells.len() < 2 {
            continue;
        }
        let name = unquote(&cells[0]);
        let branch = unquote(&cells[1]);
        if !branch.starts_with("ma/") {
            continue;
        }
        let role = cells.get(2).cloned().unwrap_or_default();
        out.push((name, branch, role));
    }
    out
}

/// Split `advocate-budget-premise` given the registered elector names.
///
/// Longest name first, so an elector named `auditor` cannot claim a file belonging to one
/// named `auditor-general`. A file matching nobody keeps its whole stem as the question and
/// reports an empty elector — which is the honest answer for a position filed under a name
/// the table does not carry.
pub(crate) fn split_position(stem: &str, electors: &[String]) -> (String, String) {
    let mut names: Vec<&String> = electors.iter().collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    for name in names {
        if let Some(rest) = stem.strip_prefix(name.as_str()) {
            if let Some(question) = rest.strip_prefix('-') {
                return (name.clone(), question.to_string());
            }
        }
    }
    (String::new(), stem.to_string())
}

/// What a resolution record's frontmatter says about itself.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct RecordHead {
    pub evolution: String,
    pub date: String,
    pub tips: Vec<String>,
    pub synthesized_by: Vec<String>,
}

/// Read a resolution record's frontmatter.
///
/// Hand-rolled rather than `serde_yaml`, because a resolution record's frontmatter is
/// authored prose and a malformed one should cost the field it malformed, not the whole
/// report.
///
/// `tips:` and `synthesized-by:` are both list-or-scalar: `synthesized-by: ma/auditor` is the
/// common case and a list is what a jointly authored synthesis writes. Reading only one shape
/// would silently drop the other, and dropping it here reads downstream as *no seat executed
/// this* — which is the finding the field exists to make, so it must not be producible by a
/// parser.
fn parse_resolution(text: &str) -> RecordHead {
    let trimmed = text.trim_start();
    let Some(rest) = trimmed.strip_prefix("---\n") else {
        return RecordHead::default();
    };
    let Some(end) = rest.find("\n---") else {
        return RecordHead::default();
    };
    let mut head = RecordHead::default();
    // Which list a bare `- item` belongs to, if any.
    let mut list: Option<&'static str> = None;
    for line in rest[..end].lines() {
        if let Some(v) = line.strip_prefix("evolution:") {
            head.evolution = unquote(v);
            list = None;
        } else if let Some(v) = line.strip_prefix("date:") {
            head.date = unquote(v);
            list = None;
        } else if let Some(v) = line.strip_prefix("synthesized-by:") {
            list = Some("synthesized_by");
            let v = unquote(v);
            if !v.is_empty() {
                head.synthesized_by.push(v);
            }
        } else if let Some(v) = line.strip_prefix("tips:") {
            list = Some("tips");
            let v = unquote(v);
            if !v.is_empty() {
                head.tips.push(v);
            }
        } else if let Some(key) = list {
            match line.trim_start().strip_prefix("- ") {
                Some(item) => match key {
                    "tips" => head.tips.push(unquote(item)),
                    _ => head.synthesized_by.push(unquote(item)),
                },
                // Any other top-level key ends the list.
                None if !line.starts_with(' ') && !line.trim().is_empty() => list = None,
                None => {}
            }
        }
    }
    head
}

fn md_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = crate::walk::walk_md_files(dir)
        .into_iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some("README.md"))
        .collect();
    files.sort();
    files
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn sangha_data(root: &Path) -> SanghaReport {
    let dir = yidam_sangha_dir(root);

    let refs: std::collections::BTreeSet<String> = crate::git::phase_refs(root)
        .into_iter()
        .map(|p| p.name)
        .collect();

    let electors: Vec<Elector> =
        parse_electors(&std::fs::read_to_string(dir.join("electors.md")).unwrap_or_default())
            .into_iter()
            .map(|(name, branch, role)| Elector {
                branch_present: refs.contains(&branch),
                name,
                branch,
                role,
            })
            .collect();

    let names: Vec<String> = electors.iter().map(|e| e.name.clone()).collect();

    let positions = md_files(&dir.join("positions"))
        .iter()
        .map(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let (elector, question) = split_position(stem, &names);
            Position {
                file: rel(root, p),
                elector,
                question,
            }
        })
        .collect();

    let resolutions = md_files(&dir.join("resolutions"))
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let head = parse_resolution(&text);
            let evolution = if head.evolution.is_empty() {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string()
            } else {
                head.evolution
            };
            Resolution {
                branch_present: refs.contains(&format!("rigpa/{evolution}")),
                file: rel(root, p),
                evolution,
                date: head.date,
                tips: head.tips,
                synthesized_by: head.synthesized_by,
            }
        })
        .collect();

    SanghaReport {
        collective: !electors.is_empty(),
        electors,
        positions,
        resolutions,
    }
}

pub(crate) fn render_sangha(r: &SanghaReport) -> String {
    if !r.collective {
        return "No registered electors — this repository is not in collective mode.\n\
                Register one by opening a `ma/<name>` branch and adding it to \
                `.yidam/sangha/electors.md`."
            .to_string();
    }

    let mut out = String::new();
    out.push_str(&format!("Electors ({})\n", r.electors.len()));
    for e in &r.electors {
        let mark = if e.branch_present {
            ""
        } else {
            "  (no branch)"
        };
        let held = r.positions.iter().filter(|p| p.elector == e.name).count();
        out.push_str(&format!(
            "  {} — {}{}, {held} position(s)\n",
            e.name, e.branch, mark
        ));
    }

    let orphans = r.positions.iter().filter(|p| p.elector.is_empty()).count();
    out.push_str(&format!(
        "\nPositions ({}){}\n",
        r.positions.len(),
        if orphans > 0 {
            format!(" — {orphans} filed under no registered elector")
        } else {
            String::new()
        }
    ));

    out.push_str(&format!("\nResolutions ({})\n", r.resolutions.len()));
    for res in &r.resolutions {
        let date = if res.date.is_empty() {
            "—"
        } else {
            &res.date
        };
        // The seat is shown where the record names one, and its absence is shown too — a
        // resolution whose executor is unrecorded is the state this field exists to make
        // visible, and a blank would read as a rendering gap rather than a finding.
        let by = if res.synthesized_by.is_empty() {
            "no seat recorded".to_string()
        } else {
            format!("by {}", res.synthesized_by.join(", "))
        };
        out.push_str(&format!(
            "  {} — {date}, {} tip(s) read, {by}\n",
            res.evolution,
            res.tips.len()
        ));
    }
    out.trim_end().to_string()
}

/// Report the sangha: electors, positions, resolutions.
pub fn sangha(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let data = sangha_data(&root);
    if format.is_json() {
        return crate::report::emit(&root, data);
    }
    println!("{}", render_sangha(&data));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The template's placeholder row is not an elector, and nothing about the placeholder's
    /// wording is what excludes it — the branch cell is.
    #[test]
    fn the_shipped_placeholder_registers_nobody() {
        let text = "| Name | Branch | Role |\n\
                    |------|--------|------|\n\
                    | *(no electors registered yet)* | | |\n";
        assert!(parse_electors(text).is_empty());
        // And the header/separator rows are not electors either.
        assert!(parse_electors("| Name | Branch |\n|---|---|\n").is_empty());
    }

    #[test]
    fn a_registered_elector_is_read_with_its_role() {
        let text = "| Name | Branch | Role |\n\
                    |------|--------|------|\n\
                    | `auditor` | `ma/auditor` | Holds a verification position. |\n\
                    | `advocate` | `ma/advocate` | Rapid response. |\n";
        let rows = parse_electors(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "auditor");
        assert_eq!(rows[0].1, "ma/auditor");
        assert_eq!(rows[0].2, "Holds a verification position.");
    }

    /// A row naming a branch outside `ma/*` is not an elector — that is the definition
    /// PROTOCOL.md gives, and the only rule this parser applies.
    #[test]
    fn a_row_whose_branch_is_not_a_ma_ref_is_not_an_elector() {
        let text = "| `someone` | `rigpa/thing` | Not an elector |\n";
        assert!(parse_electors(text).is_empty());
    }

    #[test]
    fn a_longer_elector_name_wins_the_prefix() {
        let electors = vec!["auditor".to_string(), "auditor-general".to_string()];
        assert_eq!(
            split_position("auditor-general-budget", &electors),
            ("auditor-general".to_string(), "budget".to_string())
        );
        assert_eq!(
            split_position("auditor-budget", &electors),
            ("auditor".to_string(), "budget".to_string())
        );
    }

    /// A position filed under a name the table does not carry keeps its whole stem and
    /// reports no elector. Silently attributing it to the nearest match would be worse.
    #[test]
    fn a_position_matching_nobody_is_unattributed() {
        let (elector, question) = split_position("stranger-thing", &["auditor".to_string()]);
        assert_eq!(elector, "");
        assert_eq!(question, "stranger-thing");
    }

    #[test]
    fn a_resolution_record_yields_its_evolution_date_and_tips() {
        let text = "---\n\
                    evolution: appointment-reach\n\
                    date: 2026-08-16\n\
                    synthesized-by: ma/auditor\n\
                    tips:\n  \
                      - ma/goedelsoup@0b12292\n  \
                      - ma/auditor@483228e\n\
                    ---\n\n## What was resolved\n";
        let head = parse_resolution(text);
        assert_eq!(head.evolution, "appointment-reach");
        assert_eq!(head.date, "2026-08-16");
        assert_eq!(head.tips, ["ma/goedelsoup@0b12292", "ma/auditor@483228e"]);
        assert_eq!(head.synthesized_by, ["ma/auditor"]);
    }

    /// A jointly authored synthesis names every seat, and the scalar spelling is the common
    /// case — reading only one shape would drop the other, and a dropped seat reads exactly
    /// like a record that never named one.
    #[test]
    fn synthesized_by_is_read_as_a_list_or_a_scalar() {
        let listed = parse_resolution(
            "---\nevolution: e\nsynthesized-by:\n  - ma/auditor\n  - ma/advocate\ntips:\n  - ma/x@1\n---\n",
        );
        assert_eq!(listed.synthesized_by, ["ma/auditor", "ma/advocate"]);
        assert_eq!(listed.tips, ["ma/x@1"], "the following list was absorbed");

        let scalar = parse_resolution("---\nevolution: e\nsynthesized-by: ma/auditor\n---\n");
        assert_eq!(scalar.synthesized_by, ["ma/auditor"]);
    }

    /// A record with no `synthesized-by` reports none rather than guessing one. Every record
    /// written before the field existed is this case, and inventing a seat for them would put
    /// a name on an act nobody attested.
    #[test]
    fn a_record_naming_no_seat_reports_none() {
        let head =
            parse_resolution("---\nevolution: e\ndate: 2026-01-01\ntips:\n  - ma/x@1\n---\n");
        assert!(head.synthesized_by.is_empty(), "{head:?}");
        assert_eq!(head.tips, ["ma/x@1"]);
    }

    /// A record with no frontmatter costs its fields and nothing else.
    #[test]
    fn a_record_without_frontmatter_still_reports() {
        let head = parse_resolution("# Just prose\n");
        assert_eq!(head, RecordHead::default());
    }

    #[test]
    fn a_key_after_tips_ends_the_list() {
        let text = "---\ntips:\n  - ma/a@1\ndate: 2026-01-01\n---\n";
        let head = parse_resolution(text);
        assert_eq!(head.tips, ["ma/a@1"]);
        assert_eq!(head.date, "2026-01-01");
    }

    #[test]
    fn a_repository_with_no_electors_says_so() {
        let r = SanghaReport {
            collective: false,
            electors: vec![],
            positions: vec![],
            resolutions: vec![],
        };
        assert!(render_sangha(&r).contains("not in collective mode"));
    }
}
