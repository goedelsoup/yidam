//! `yidam migrate used-by` — a catalog entry stops storing what cites it (RFC-0053 §2.2, #1498).
//!
//! An entry used to carry a hand-maintained `used-by:` list beside the citations, and
//! `catalog-reconcile` rewrote it whenever the two disagreed — one `reconcile:` commit per
//! entry, every time a node was added. The citations are the record. `yidam catalog-audit`
//! reports them as `cited_by`, and nothing but yidam read the stored copy.
//!
//! This deletes the `used-by:` block from every entry that declares one, `used-by: []`
//! included, by a line edit. It edits no other file.
//!
//! # What it keeps
//!
//! The deleted lines are the only thing lost, so the part of them that matters is kept: for
//! each entry, the names the list claimed that nothing cites. Either the list was stale, or the
//! file should cite the entry and does not, and this cannot tell which. They are reported, and
//! written into the migration record, so an author who skipped the dry run can still find them.
//!
//! The comparison is against everything [`draws_on`] finds for the entry — the instances and
//! the READMEs and class definitions — and not the instances alone, as the retired gate's was.
//! The gate asked whether the list matched `cited_by`. This asks whether a citation is missing,
//! and a README linking the entry is not missing one.
//!
//! A name the citations hold and the list omitted is not reported. The citation is the record.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::Result;

use super::catalog::audit::draws_on;
use super::catalog::record::used_by_lines;
use super::lint::checks::{basename, used_by_drift};
use super::migrate::{rel, MigrateReport};
use super::rename::{Edit, Unhandled};
use crate::corpus::normalize;
use crate::parse::parse_frontmatter;
use crate::paths::{yidam_catalog_dir, yidam_corpus_dir};
use crate::walk::walk_md_files;

/// One entry's list, as far as it named something that does not cite the entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Claimed {
    /// The entry, repository-relative.
    pub entry: String,
    /// What the list named, as written, that carries no citation to the entry.
    pub names: Vec<String>,
}

pub(crate) fn plan(root: &Path, report: &mut MigrateReport) {
    let draws = if yidam_corpus_dir(root).exists() {
        draws_on(&crate::corpus::Corpus::open(root))
    } else {
        HashMap::new()
    };
    for path in walk_md_files(&yidam_catalog_dir(root)) {
        // README.md is `catalog-audit`'s REGEN target, not a source.
        if path.file_name().is_some_and(|n| n == "README.md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file = rel(root, &path);
        let declared = parse_frontmatter(&text).used_by;
        let range = match used_by_lines(&text) {
            Ok(Some(range)) => range,
            Ok(None) => continue,
            // The parser tolerates leading blank lines and the editor cannot. An entry the
            // parser reads a list from is one the gate will keep reporting, so say why it was
            // left rather than skipping it in silence.
            Err(e) if declared.is_some() => {
                report.unhandled.push(Unhandled {
                    file,
                    line: 1,
                    text: format!("declares `used-by:` and was left as written: {e}"),
                });
                continue;
            }
            Err(_) => continue,
        };
        let lines: Vec<&str> = text.split('\n').collect();
        for line in range.clone() {
            report.edits.push(Edit {
                file: file.clone(),
                line,
                from: lines.get(line - 1).copied().unwrap_or("").to_string(),
                to: String::new(),
            });
        }

        let cited = draws.get(&normalize(&path)).cloned().unwrap_or_default();
        let citing: Vec<String> = cited.nodes.into_iter().chain(cited.elsewhere).collect();
        let Some(drift) = used_by_drift(declared.as_deref(), &citing) else {
            continue;
        };
        if drift.claimed_not_citing.is_empty() {
            continue;
        }
        // `used_by_drift` names basenames; the record names what the list said, so an author
        // reading it can find the line in the entry's history.
        let missing: HashSet<&str> = drift
            .claimed_not_citing
            .iter()
            .map(String::as_str)
            .collect();
        let mut seen = BTreeSet::new();
        let names: Vec<String> = declared
            .unwrap_or_default()
            .into_iter()
            .filter(|n| missing.contains(basename(n)) && seen.insert(n.clone()))
            .collect();
        report.claimed.push(Claimed { entry: file, names });
    }
}

/// How many entries the migration deletes a list from.
pub(crate) fn deleted(report: &MigrateReport) -> usize {
    report
        .edits
        .iter()
        .map(|e| e.file.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

/// Delete every planned block, and return the entries written.
///
/// Its own apply, as `pins` has: a deletion removes lines, and the shared loop's edit replaces
/// a span inside one. Each entry's block is located again and must be the lines the plan
/// read, so an entry edited between the plan and the apply is left as it is.
pub(crate) fn apply(root: &Path, report: &MigrateReport) -> Result<Vec<String>> {
    let mut by_file: BTreeMap<&str, Vec<&Edit>> = BTreeMap::new();
    for e in &report.edits {
        by_file.entry(&e.file).or_default().push(e);
    }
    let mut written = Vec::new();
    for (file, edits) in by_file {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path)?;
        let Some(out) = delete(&text, &edits) else {
            continue;
        };
        std::fs::write(&path, out)?;
        written.push(file.to_string());
    }
    Ok(written)
}

/// `text` without its `used-by` block, or `None` if the block is not the one `edits` planned.
fn delete(text: &str, edits: &[&Edit]) -> Option<String> {
    let range = used_by_lines(text).ok()??;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let planned: Vec<usize> = edits.iter().map(|e| e.line).collect();
    if planned != range.clone().collect::<Vec<_>>() {
        return None;
    }
    for e in edits {
        if lines.get(e.line - 1)?.trim_end_matches('\n') != e.from {
            return None;
        }
    }
    let out: String = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !range.contains(&(i + 1)))
        .map(|(_, l)| *l)
        .collect();
    // The deletion must leave an entry that declares nothing and still parses as it did.
    let before = parse_frontmatter(text);
    let after = parse_frontmatter(&out);
    (after.used_by.is_none() && after.name == before.name).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edits_for(file: &str, text: &str) -> Vec<Edit> {
        let lines: Vec<&str> = text.split('\n').collect();
        used_by_lines(text)
            .unwrap()
            .unwrap()
            .map(|line| Edit {
                file: file.into(),
                line,
                from: lines[line - 1].to_string(),
                to: String::new(),
            })
            .collect()
    }

    #[test]
    fn a_list_is_deleted_and_nothing_else_moves() {
        let entry = "---\nname: s\nused-by:\n  - ../corpus/gage/a.yml\n  - b.yml\n\n\
                     type: dataset\n---\n\n# S\n";
        let edits = edits_for("e.md", entry);
        assert_eq!(edits.iter().map(|e| e.line).collect::<Vec<_>>(), [3, 4, 5]);
        let refs: Vec<&Edit> = edits.iter().collect();
        assert_eq!(
            delete(entry, &refs).unwrap(),
            "---\nname: s\n\ntype: dataset\n---\n\n# S\n"
        );
    }

    #[test]
    fn an_empty_list_is_deleted_too() {
        let entry = "---\nname: s\nused-by: []\n---\n\n# S\n";
        let edits = edits_for("e.md", entry);
        let refs: Vec<&Edit> = edits.iter().collect();
        assert_eq!(delete(entry, &refs).unwrap(), "---\nname: s\n---\n\n# S\n");
    }

    #[test]
    fn an_entry_edited_since_the_plan_is_left_alone() {
        let entry = "---\nname: s\nused-by:\n  - a.yml\n---\n";
        let edits = edits_for("e.md", entry);
        let refs: Vec<&Edit> = edits.iter().collect();
        let edited = "---\nname: s\nused-by:\n  - c.yml\n---\n";
        assert_eq!(delete(edited, &refs), None);
    }

    #[test]
    fn an_entry_declaring_no_list_has_nothing_to_delete() {
        assert_eq!(used_by_lines("---\nname: s\n---\n").unwrap(), None);
    }
}
