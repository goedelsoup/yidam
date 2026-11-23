//! The declared-table checks (RFC-0059 §2.2): a `dataset` entry's `columns:` is a declaration,
//! and its table file is held to it.
//!
//! All three read the working tree, and ask git only whether the file is tracked: a table is
//! the bytes a calculator would read now, and an untracked one is a file no clone would have.

use std::collections::HashSet;
use std::path::Path;

use super::model::{Check, Severity, Violation};
use crate::corpus::{Node, Source};
use crate::table::{self, Declaration, Rows};

pub(crate) const DECLARATION_INVALID: &str = "table-declaration-invalid";
pub(crate) const SHAPE: &str = "table-shape";
pub(crate) const REF_UNRESOLVED: &str = "table-ref-unresolved";

/// Every tracked path under `root`, or `None` when git could not say.
pub(crate) fn tracked(root: &Path) -> Option<HashSet<String>> {
    let out = crate::git::Git::new(root)
        .args(["ls-files", "-z"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// One declared table, as far as it could be read.
pub(crate) enum Table {
    /// The declaration is malformed, or the file is missing, untracked or unreadable as its
    /// format. Each string is one reason, said about the entry.
    Invalid { entry_rel: String, why: Vec<String> },
    /// The file was read. `departures` is what [`table::shape`] found, uncapped.
    Read {
        decl: Declaration,
        rows: Rows,
        departures: Vec<table::Departure>,
    },
}

/// Every table the catalog declares, read and shaped.
///
/// The one place a declaration meets its file, shared by `lint`, `yidam run` and the `#row=`
/// citation so the three cannot disagree about what a table holds. `read` is how a file's bytes
/// are obtained — the overlay under `lint`, the disk under `run` — and `tracked` is git's
/// listing, which is consulted only when some entry declares a table.
pub(crate) fn declared(
    root: &Path,
    sources: &[Source],
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<Table> {
    let decls: Vec<(&Source, Result<Declaration, Vec<String>>)> = sources
        .iter()
        .filter(|s| s.malformed.is_none())
        .filter_map(|s| table::declaration(s).map(|d| (s, d)))
        .collect();
    if decls.is_empty() {
        return Vec::new();
    }
    let listed = tracked(root);
    decls
        .into_iter()
        .map(|(s, d)| match d {
            Err(why) => Table::Invalid {
                entry_rel: s.rel.clone(),
                why,
            },
            Ok(decl) => {
                let text = read(&root.join(&decl.path));
                let tracked = listed.as_ref().is_none_or(|l| l.contains(&decl.path));
                match text {
                    None => Table::Invalid {
                        entry_rel: s.rel.clone(),
                        why: vec![format!("names `{}`, which does not exist", decl.path)],
                    },
                    Some(_) if !tracked => Table::Invalid {
                        entry_rel: s.rel.clone(),
                        why: vec![format!(
                            "names `{}`, which git does not track — a table is read from the \
                             tree a clone has",
                            decl.path
                        )],
                    },
                    Some(text) => match table::read(&text, decl.format) {
                        Err(e) => Table::Read {
                            departures: vec![table::Departure {
                                line: 0,
                                column: None,
                                detail: format!("is not readable as {:?}: {e}", decl.format),
                            }],
                            decl,
                            rows: Rows::default(),
                        },
                        Ok(rows) => {
                            let departures = table::shape(&decl, &rows);
                            Table::Read {
                                decl,
                                rows,
                                departures,
                            }
                        }
                    },
                }
            }
        })
        .collect()
}

/// The three checks, in roster order: declaration, shape, refs.
pub(crate) fn checks(tables: &[Table], nodes: &[Node]) -> [Check; 3] {
    let mut invalid = Vec::new();
    let mut shape = Vec::new();
    let mut refs = Vec::new();
    let held: HashSet<&str> = nodes
        .iter()
        .filter_map(|n| crate::computed::node_id(&n.rel))
        .collect();
    for t in tables {
        match t {
            Table::Invalid { entry_rel, why } => {
                for w in why {
                    invalid.push(Violation::new(entry_rel.clone(), w.clone()));
                }
            }
            Table::Read {
                decl,
                rows,
                departures,
            } => {
                for d in departures.iter().take(table::SHAPE_CAP) {
                    shape.push(Violation::new(decl.path.clone(), d.to_string()));
                }
                if departures.len() > table::SHAPE_CAP {
                    shape.push(Violation::new(
                        decl.path.clone(),
                        format!(
                            "and {} more departures from `{}`'s declaration",
                            departures.len() - table::SHAPE_CAP,
                            decl.entry
                        ),
                    ));
                }
                for (line, column, cell) in table::refs(decl, rows) {
                    let resolved = crate::computed::node_reference(cell)
                        .is_some_and(|r| held.contains(r.as_str()));
                    if !resolved {
                        refs.push(Violation::new(
                            decl.path.clone(),
                            format!("line {line}, column `{column}`: `{cell}` names no node in this corpus"),
                        ));
                    }
                }
            }
        }
    }
    [
        Check::new(
            DECLARATION_INVALID,
            "A dataset entry's `columns:` declares a table it can be held to",
            Severity::Error,
            "An entry that carries `columns:` either declares a table or is reported. A \
             declaration with a refused column type, a duplicate column, a key naming no column, \
             or a file that is not one tracked `.tsv` or `.csv` cannot be checked against, so it \
             is reported here and its file is not read against it (RFC-0059 §2.2).",
            invalid,
        ),
        Check::new(
            SHAPE,
            "A declared table holds the rows its declaration describes",
            Severity::Error,
            "A table its declaration does not describe is read by a calculator as if it did. The \
             header, each row's width, each required cell, each cell's type, each closed value \
             and each key are held to the declaration, by the reader `yidam run` and the `#row=` \
             citation share. One file reports twenty findings, then a count (RFC-0059 §2.2).",
            shape,
        ),
        Check::new(
            REF_UNRESOLVED,
            "A table's `ref` cell names a node the corpus holds",
            Severity::Warn,
            "A `ref` cell names a node in RFC-0032's reference grammar. A warning and not an \
             error, because an extract may run ahead of the corpus and the cell is still a fact \
             about the source (RFC-0059 §2.2).",
            refs,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl_with_ref() -> Declaration {
        Declaration {
            entry: "r".into(),
            entry_rel: ".yidam/catalog/r.md".into(),
            path: "data/r.tsv".into(),
            format: table::Format::Tsv,
            columns: vec![
                table::Column {
                    name: "id".into(),
                    r#type: "string".into(),
                    required: true,
                    unit: None,
                    values: None,
                },
                table::Column {
                    name: "who".into(),
                    r#type: "ref".into(),
                    required: false,
                    unit: None,
                    values: None,
                },
            ],
            key: vec!["id".into()],
        }
    }

    #[test]
    fn twenty_five_bad_rows_report_twenty_and_a_count() {
        let decl = decl_with_ref();
        let mut text = "id\twho\n".to_string();
        for _ in 0..25 {
            text.push_str("\t\n");
        }
        let rows = table::read(&text, table::Format::Tsv).unwrap();
        let departures = table::shape(&decl, &rows);
        assert_eq!(departures.len(), 25);
        let [_, shape, _] = checks(
            &[Table::Read {
                decl,
                rows,
                departures,
            }],
            &[],
        );
        assert_eq!(shape.violations.len(), 21);
        assert!(shape.violations[20].detail.starts_with("and 5 more"));
    }

    #[test]
    fn an_unresolved_ref_is_a_warning_on_the_table_file() {
        let decl = decl_with_ref();
        let rows = table::read("id\twho\nA\tcommittee/nobody\nB\t\n", table::Format::Tsv).unwrap();
        let [_, _, refs] = checks(
            &[Table::Read {
                decl,
                rows,
                departures: Vec::new(),
            }],
            &[],
        );
        assert_eq!(refs.severity, Severity::Warn);
        assert_eq!(refs.violations.len(), 1, "the empty cell is no reference");
        assert_eq!(refs.violations[0].node, "data/r.tsv");
        assert!(refs.violations[0].detail.contains("line 2, column `who`"));
    }
}
