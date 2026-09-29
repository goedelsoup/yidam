//! `yidam migrate` — changing an ontology without breaking every corpus that adopted it.
//!
//! A typed ontology is only as good as its ability to change, and until the class contract
//! gated, changing one was merely tedious: hand-edit the class, grep for instances, hope.
//! Now it is a build break. Add a property and every instance trips `missing-property`;
//! retype one and they trip `property-type`; re-target an edge and they trip
//! `edge-target-class`. A corpus cannot adopt a *corrected* class definition without
//! failing its own gate in the interval — which is a strong incentive to leave the
//! definition wrong.
//!
//! # The three operations
//!
//! | Operation | What it touches |
//! |---|---|
//! | class rename | the class file, its directory, every instance's `class:`, every edge `target:` at both ends, and every link that resolved into the directory |
//! | property rename | the declaration, and the key on every instance carrying it |
//! | property retype | the declaration, plus every instance value the new type only rewrites the *quoting* of — and it **refuses** a value the new type cannot admit at all |
//! | value rename | one item of the declaration's `values:` list, and the value on every instance holding it |
//! | edge re-target | the declaration at both ends, plus a report of the instances now in violation |
//!
//! # What it refuses to guess
//!
//! A retype is the operation with a wrong answer available. `type: string` → `type: date`
//! over a value reading `last spring` has no mechanical conversion, and inventing one — or
//! writing the field back as a string and calling it migrated — would put the corpus in a
//! state its own gate rejects while reporting success. So a retype that cannot be performed
//! is `blocked`, listing the instances and their values, and **nothing is written**.
//!
//! The check that decides is [`crate::cmd::lint::checks::property_type_violation`] — the
//! same predicate `property-type` gates on, not a second reading of it. A migration that
//! disagreed with the gate about what a valid value is would be a migration into a failing
//! build.
//!
//! A retype *into* `string` on a property that declares `values:` (RFC-0044) is held to that
//! set by the same rule: the set was carried and ignored on the type the property had, it
//! binds on `string`, and an instance outside it is reported and blocks. Widening the set or
//! fixing the value is a judgment about what the instance meant, and both are the author's.
//!
//! **Requoting is not guessing**, and it was refused along with the guesses until #1044. Two
//! of `property-type`'s messages name their own repair: a `number` holding `"24"` is told to
//! *unquote it*, and a `string` holding a bare `24` — or a `date` holding a bare `1985` — is
//! told to *quote it*. Those are the same bytes written the other way, and a retype that
//! refused them was asking an author to carry out an instruction character for character
//! before it would run. So it performs them, on the instances and nowhere else, and verifies
//! each one by parsing what it would write and putting that back through the gate's
//! predicate. `about 24` and `~24` still refuse: there is no number in them to unquote, and
//! the tilde was carrying a meaning the number cannot.
//!
//! # Line edits, not a YAML round-trip
//!
//! Every rewrite here is a line edit, as `rename`'s is. Parsing and re-emitting the
//! document would reformat every file it touched: comments dropped, block scalars
//! reflowed, key order normalized. The diff a reviewer reads is the whole value of an
//! epistemic commit, and a migration that produced a thousand-line diff for a two-field
//! change would not be read at all.

use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::paths::{repo_root, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_ont_files};

use super::rename::{target_on, Edit, Unhandled};
use crate::corpus::resolve_target;

/// Which migration to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// `<old>.ont.yml` and `corpus/<old>/` become `<new>`.
    ClassRename { old: String, new: String },
    /// A declared property changes name on the class and on every instance.
    PropertyRename {
        class: String,
        old: String,
        new: String,
    },
    /// A declared property changes type. Refuses when an instance would not satisfy it.
    PropertyRetype {
        class: String,
        property: String,
        new_type: String,
    },
    /// One value of a declared `values:` set (RFC-0044) changes name on the class and on every
    /// instance holding it.
    ///
    /// `property-rename`'s shape one level down: the set is closed, so a value cannot be
    /// renamed on the instances without the declaration, or on the declaration without every
    /// instance tripping `property-type` in the interval.
    ValueRename {
        class: String,
        property: String,
        from: String,
        to: String,
    },
    /// A declared relationship points at a different class.
    EdgeRetarget {
        class: String,
        relationship: String,
        new_target: String,
    },
    /// Every reference written inside an evidence tag becomes a `references:` entry.
    ///
    /// The one operation here that migrates *data* rather than the ontology over it, and the
    /// reason it belongs anyway: it is the same event shape — a mechanical rewrite across
    /// hundreds of files under one commit subject, with a record of what it refused. See
    /// [`crate::cmd::migrate_references`].
    References,
    /// Every paragraph an earlier `propose` spliced into prose becomes a `yidam:` record.
    ///
    /// The one-time lift #712 left behind. Like [`Self::References`] it migrates *data* rather
    /// than the ontology over it, and for the same reason it belongs here: a mechanical rewrite
    /// across every node under one commit subject, with a record of what it refused. See
    /// [`crate::cmd::migrate_findings`].
    Findings,
    /// `AGENTS.md`'s whole-file reading list becomes the `yidam routes` block (#1135).
    ///
    /// Migrates neither the ontology nor the data but the scaffold, and belongs here for the
    /// same reason the two lifts do: a one-time mechanical rewrite with a record of what it
    /// refused. See [`crate::cmd::migrate_routes`].
    Routes,
    /// `ci.yml` and `CLAUDE.md` get the regions `yidam-vendor-update` rewrites (#1054).
    ///
    /// Migrates the scaffold, as [`Self::Routes`] does, and for the same reason. See
    /// [`crate::cmd::migrate_scaffold`].
    Scaffold,
}

impl Operation {
    /// The kind, as the migration record and the report name it.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ClassRename { .. } => "class-rename",
            Self::PropertyRename { .. } => "property-rename",
            Self::PropertyRetype { .. } => "property-retype",
            Self::ValueRename { .. } => "value-rename",
            Self::EdgeRetarget { .. } => "edge-retarget",
            Self::References => "references",
            Self::Findings => "findings",
            Self::Routes => "routes",
            Self::Scaffold => "scaffold",
        }
    }

    /// One line, for the commit subject and the record's summary.
    pub fn summary(&self) -> String {
        match self {
            Self::ClassRename { old, new } => format!("class `{old}` → `{new}`"),
            Self::PropertyRename { class, old, new } => {
                format!("`{class}.{old}` → `{class}.{new}`")
            }
            Self::PropertyRetype {
                class,
                property,
                new_type,
            } => format!("`{class}.{property}` is now `{new_type}`"),
            Self::ValueRename {
                class,
                property,
                from,
                to,
            } => format!("`{class}.{property}`: `{from}` → `{to}`"),
            Self::EdgeRetarget {
                class,
                relationship,
                new_target,
            } => format!("`{class}` — `{relationship}` now targets `{new_target}`"),
            // Replaced once planned: what this migration *is* is how much of the corpus it
            // reaches, which is not known until the corpus has been read. This stable form is
            // what `record_path` slugs, so it must not begin with the operation's own name —
            // `references-references-out-of-evidence-tags.yml` is a filename a corpus keeps.
            Self::References => "evidence tags into the reference field".to_string(),
            // Replaced once planned, for the reason above, and a stable form that does not
            // begin with the operation's own name for the same one.
            Self::Findings => "legacy propose paragraphs into records".to_string(),
            // Replaced once planned, and for the same reasons as the two above.
            Self::Routes => "AGENTS.md reading list into a generated block".to_string(),
            // A stable form that does not begin with the operation's own name, as above.
            Self::Scaffold => "ci.yml and CLAUDE.md regions a re-vendor updates".to_string(),
        }
    }
}

/// An instance the migration leaves in a state the gate will reject.
///
/// Distinct from `blocked`, which stops the migration. This is work the migration *creates*
/// and cannot do: re-targeting an edge is a decision about the ontology, and which instances
/// should now point elsewhere is a decision about the corpus. Reporting them is the whole
/// difference between "migrated" and "migrated and quietly broke forty nodes".
#[derive(Debug, serde::Serialize)]
pub struct Violation {
    pub node: String,
    pub detail: String,
}

#[derive(Debug, serde::Serialize)]
pub struct MigrateReport {
    pub corpus_dir: String,
    /// `class-rename`, `property-rename`, `property-retype`, `value-rename`, `edge-retarget`.
    pub operation: &'static str,
    pub summary: String,
    /// Whether anything was written. False for `--dry-run`, and false whenever `blocked` is
    /// non-empty.
    pub applied: bool,
    /// File moves. One per instance for a class rename; empty otherwise.
    pub moves: Vec<Edit>,
    pub edits: Vec<Edit>,
    /// Instances this migration leaves in violation. Reported, never silently fixed.
    pub violations: Vec<Violation>,
    /// Markdown references to a renamed path. Reported, never rewritten.
    pub unhandled: Vec<Unhandled>,
    /// References lifted out of evidence-tag details. Empty for every ontology operation.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifted: Vec<super::migrate_references::Lift>,
    /// Evidence-tag details naming nothing addressable.
    ///
    /// **A count and not a list.** 614 details across the measured corpora are prose — "owner
    /// attestation, no document in hand" — and printing them as work to do would tell an author
    /// to fix 614 details that are correct. What a detail *should* have resolved to is
    /// `reference-not-in-the-grammar`'s question, not this one.
    #[serde(skip_serializing_if = "is_zero")]
    pub prose_details: usize,
    /// Findings lifted out of legacy `propose` paragraphs. Empty for every other operation.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<super::migrate_findings::Lifted>,
    /// Paragraphs an author has reworded past recognition.
    ///
    /// A count and not a list, for the reason [`Self::prose_details`] gives: those sentences are
    /// somebody's prose now, and listing them would read as work to do.
    #[serde(skip_serializing_if = "is_zero")]
    pub prose_findings: usize,
    /// The reading list `migrate routes` replaces, and the block it writes. `None` for every
    /// other operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routes: Option<super::migrate_routes::Routes>,
    /// The two files `migrate scaffold` marks, and what goes inside each region. `None` for
    /// every other operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaffold: Option<super::migrate_scaffold::Scaffold>,
    /// Why this cannot proceed. Non-empty means nothing was touched.
    pub blocked: Vec<String>,
    /// Where the migration record was written, once applied.
    pub record: String,
    /// A commit subject in the closed vocabulary.
    ///
    /// `migrate`, which GRAPH.md defines as "Data or schema moved" — the same verb `rename`
    /// reaches for, and for the same reason: inventing one costs twice, because
    /// `lint --commits` reports it and `classify_commit` files it as Epistemic.
    pub commit_subject: String,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

impl MigrateReport {
    fn new(root: &Path, corpus: &Path, op: &Operation) -> Self {
        Self {
            corpus_dir: slash(corpus.strip_prefix(root).unwrap_or(corpus)),
            operation: op.kind(),
            summary: op.summary(),
            applied: false,
            moves: vec![],
            edits: vec![],
            violations: vec![],
            unhandled: vec![],
            lifted: vec![],
            prose_details: 0,
            findings: vec![],
            prose_findings: 0,
            routes: None,
            scaffold: None,
            blocked: vec![],
            record: String::new(),
            commit_subject: String::new(),
        }
    }
}

/// The class file for `name`, whether or not it exists.
fn ont_path(corpus: &Path, name: &str) -> PathBuf {
    corpus.join(format!("{name}.ont.yml"))
}

/// The value of a `key: value` line at any indent **as written**, quotes and all, with its
/// byte range.
///
/// Deliberately not a YAML parse: this is used to rewrite one scalar in place, leaving
/// every other byte of the file — comments, block scalars, key order — exactly as written.
///
/// [`scalar_on`] is this same reading with the quotes stripped off, which is what every
/// rename here wants and the exact opposite of what a requote wants: the value *inside* the
/// quotes of `"24"` is `24` already, so rewriting that span would write the same bytes back.
fn raw_scalar_on(line: &str, key: &str) -> Option<(usize, usize, String)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let body = trimmed
        .strip_prefix("- ")
        .map(|r| (2, r))
        .unwrap_or((0, trimmed));
    let rest = body.1.strip_prefix(key)?.strip_prefix(':')?;
    let lead = rest.len() - rest.trim_start().len();
    let mut value = rest.trim_start();
    if let Some(hash) = value.find(" #") {
        value = &value[..hash];
    }
    let value = value.trim_end();
    if value.is_empty() {
        return None;
    }
    let start = indent + body.0 + key.len() + 1 + lead;
    Some((start, start + value.len(), value.to_string()))
}

/// The value of a `key: value` line, inside its quotes where it has them.
fn scalar_on(line: &str, key: &str) -> Option<(usize, usize, String)> {
    let (start, end, value) = raw_scalar_on(line, key)?;
    match unquoted(&value) {
        Some(inner) => Some((start + 1, end - 1, inner.to_string())),
        None => Some((start, end, value)),
    }
}

/// The inside of a quoted scalar, or `None` when it carries no quotes.
///
/// The length test is not redundant: a lone `"` both starts and ends with one.
fn unquoted(value: &str) -> Option<&str> {
    let quote = value.chars().next()?;
    if !matches!(quote, '"' | '\'') || value.len() < 2 || !value.ends_with(quote) {
        return None;
    }
    Some(&value[1..value.len() - 1])
}

/// A mapping key line — `  <name>:` — with the key's byte range.
///
/// Used for the instance side of a property rename, where the property *is* the key.
fn mapping_key_on(line: &str, key: &str) -> Option<(usize, usize)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    // Only a nested key: a top-level `class:` is not a property of the property bag.
    if indent == 0 {
        return None;
    }
    let rest = trimmed.strip_prefix(key)?;
    if !rest.starts_with(':') {
        return None;
    }
    Some((indent, indent + key.len()))
}

/// An item of a flow-form `values: [a, b, c]` line reading `value`, with its byte range.
///
/// The item is matched as written or inside its quotes, and the range is the item as written,
/// quotes included — a value rename rewrites the whole token, as a requote does. Not a YAML
/// parse, for the reason [`raw_scalar_on`] is not: every other byte of the line stays.
fn values_item_on(line: &str, value: &str) -> Option<(usize, usize)> {
    let (start, _, list) = raw_scalar_on(line, "values")?;
    let inner = list.strip_prefix('[')?.strip_suffix(']')?;
    let mut quote: Option<char> = None;
    let mut item_start = 0;
    for (i, c) in inner
        .char_indices()
        .chain(std::iter::once((inner.len(), ',')))
    {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if matches!(c, '"' | '\'') => quote = Some(c),
            None if c == ',' => {
                let raw = &inner[item_start..i];
                let lead = raw.len() - raw.trim_start().len();
                let item = raw.trim();
                if !item.is_empty() && (item == value || unquoted(item) == Some(value)) {
                    let s = start + 1 + item_start + lead;
                    return Some((s, s + item.len()));
                }
                item_start = i + 1;
            }
            None => {}
        }
    }
    None
}

/// A block-form list item — `  - <value>` — reading `value`, with the item's byte range.
///
/// The block spelling of [`values_item_on`]. A trailing ` #` comment is left where it is.
fn list_item_on(line: &str, value: &str) -> Option<(usize, usize)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let rest = trimmed.strip_prefix("- ")?;
    let lead = rest.len() - rest.trim_start().len();
    let mut item = rest.trim_start();
    if let Some(hash) = item.find(" #") {
        item = &item[..hash];
    }
    let item = item.trim_end();
    if item.is_empty() || (item != value && unquoted(item) != Some(value)) {
        return None;
    }
    let s = indent + 2 + lead;
    Some((s, s + item.len()))
}

/// `value` spelled so that YAML reads it back as exactly that string.
///
/// Bare where bare reads back as itself — the candidate is parsed rather than reasoned about,
/// which is what keeps `true`, `24` and `null` from silently leaving the set they were renamed
/// into — and double-quoted otherwise. The flow-list separators are quoted whether or not the
/// parser would mind them in a block, because the declaration may be written either way.
fn yaml_token(value: &str) -> String {
    let plain = !value.is_empty()
        && value.trim() == value
        && !value.contains(['"', '\'', ',', '[', ']', '{', '}', '#', '\n'])
        && matches!(
            serde_yaml::from_str::<serde_yaml::Value>(value),
            Ok(serde_yaml::Value::String(s)) if s == value
        );
    if plain {
        value.to_string()
    } else if !value.contains('"') {
        format!("\"{value}\"")
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

/// `to`, written the way `written` was: quoted with the same quote where it had one, bare
/// where a bare spelling reads back as itself, and otherwise quoted.
fn written_like(written: &str, to: &str) -> String {
    match written.chars().next() {
        Some(q @ ('"' | '\'')) if unquoted(written).is_some() && !to.contains(q) => {
            format!("{q}{to}{q}")
        }
        _ => yaml_token(to),
    }
}

fn push_edit(edits: &mut Vec<Edit>, file: &str, line: usize, from: &str, to: &str) {
    edits.push(Edit {
        file: file.to_string(),
        line,
        from: from.to_string(),
        to: to.to_string(),
    });
}

fn rel(root: &Path, path: &Path) -> String {
    slash(path.strip_prefix(root).unwrap_or(path))
}

// ── planning ──────────────────────────────────────────────────────────────────

pub(crate) fn plan(root: &Path, corpus: &Path, op: &Operation) -> MigrateReport {
    let mut report = MigrateReport::new(root, corpus, op);
    match op {
        Operation::ClassRename { old, new } => {
            plan_class_rename(root, corpus, old, new, &mut report)
        }
        Operation::PropertyRename { class, old, new } => {
            plan_property_rename(root, corpus, class, old, new, &mut report)
        }
        Operation::PropertyRetype {
            class,
            property,
            new_type,
        } => plan_property_retype(root, corpus, class, property, new_type, &mut report),
        Operation::ValueRename {
            class,
            property,
            from,
            to,
        } => plan_value_rename(root, corpus, class, property, from, to, &mut report),
        Operation::EdgeRetarget {
            class,
            relationship,
            new_target,
        } => plan_edge_retarget(root, corpus, class, relationship, new_target, &mut report),
        Operation::References => {
            super::migrate_references::plan(root, corpus, &mut report);
            report.summary = super::migrate_references::summary(&report);
        }
        Operation::Findings => {
            super::migrate_findings::plan(root, corpus, &mut report);
            report.summary = super::migrate_findings::summary(&report);
        }
        Operation::Routes => {
            super::migrate_routes::plan(root, &mut report);
            report.summary = super::migrate_routes::summary(&report);
        }
        Operation::Scaffold => {
            super::migrate_scaffold::plan(root, &mut report);
            report.summary = super::migrate_scaffold::summary(&report);
        }
    }
    if report.blocked.is_empty() {
        // A reference lift's unit of work is a reference and not an edit: only the tags it
        // collapses are edits, so counting those would report a third of what it did.
        let (count, unit, files) = if matches!(op, Operation::References) {
            (
                report.lifted.len(),
                "reference",
                report
                    .lifted
                    .iter()
                    .map(|l| l.node.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
            )
        } else if matches!(op, Operation::Findings) {
            // A findings lift makes no edits at all: its unit is a paragraph replaced by a
            // record, and counting edits would report that it did nothing.
            (
                report.findings.len(),
                "finding",
                report
                    .findings
                    .iter()
                    .map(|l| l.node.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
            )
        } else if let Some(routes) = &report.routes {
            // A routes migration makes no edits either: its unit is a bullet of the list the
            // block replaces, in the one file it writes.
            (
                routes.replaced.len(),
                "bullet",
                usize::from(!routes.already),
            )
        } else if let Some(scaffold) = &report.scaffold {
            // Nor a scaffold migration: its unit is a file given a region.
            let n = scaffold.files.iter().filter(|f| f.to_write()).count();
            (n, "file", n)
        } else {
            (
                report.edits.len(),
                "edit",
                report
                    .edits
                    .iter()
                    .map(|e| e.file.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
            )
        };
        report.commit_subject = format!(
            "migrate: {} ({count} {unit}(s) across {files} file(s))",
            op.summary()
        );
    }
    report
}

/// A class must exist to be migrated, and the name it is going to must be free.
fn check_class(corpus: &Path, class: &str, report: &mut MigrateReport) -> bool {
    if !ont_path(corpus, class).is_file() {
        report
            .blocked
            .push(format!("class `{class}` has no {class}.ont.yml"));
        return false;
    }
    true
}

fn plan_class_rename(root: &Path, corpus: &Path, old: &str, new: &str, report: &mut MigrateReport) {
    if !check_class(corpus, old, report) {
        return;
    }
    if old == new {
        report.blocked.push("the two names are the same".into());
    }
    if ont_path(corpus, new).exists() || corpus.join(new).exists() {
        report.blocked.push(format!(
            "`{new}` already exists — renaming onto it would lose it"
        ));
    }
    if new.is_empty() || new.contains('/') {
        report.blocked.push(format!("`{new}` is not a class name"));
    }
    if !report.blocked.is_empty() {
        return;
    }

    // The class file itself: its `class:` field, and the file's own name.
    let ont = ont_path(corpus, old);
    let ont_rel = rel(root, &ont);
    let text = std::fs::read_to_string(&ont).unwrap_or_default();
    for (i, line) in text.lines().enumerate() {
        if let Some((_, _, value)) = scalar_on(line, "class") {
            if value == old {
                push_edit(&mut report.edits, &ont_rel, i + 1, old, new);
            }
        }
    }
    // Moves carry REPOSITORY-relative paths, both sides. They used to be corpus-relative
    // for instances and repo-relative for the class file, and `apply` re-appended `.yml` to
    // a name that already had one — which produced `canyon-outlet.yml.yml` and six dangling
    // edges. One convention, applied by one `root.join`.
    report.moves.push(Edit {
        file: ont_rel,
        line: 0,
        from: format!("{}/{old}.ont.yml", report.corpus_dir),
        to: format!("{}/{new}.ont.yml", report.corpus_dir),
    });

    // Every OTHER class that names this one as an edge target. An edge is declared from
    // both ends, so a class rename that fixed only its own side would leave the mirror
    // pointing at a class that no longer exists — and `edge-target-class` would report the
    // instances rather than the declaration, which is the finding a reader cannot act on.
    for path in walk_ont_files(corpus) {
        if path == ont {
            continue;
        }
        let file = rel(root, &path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if let Some((_, _, value)) = scalar_on(line, "target") {
                if value == old {
                    push_edit(&mut report.edits, &file, i + 1, old, new);
                }
            }
        }
    }

    // Every instance: its own `class:`, and its file's new home.
    let old_dir = corpus.join(old);
    for path in walk_corpus_instances(corpus) {
        let id = slash(path.strip_prefix(corpus).unwrap_or(&path));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let file = rel(root, &path);
        let moving = path.starts_with(&old_dir);

        if moving {
            for (i, line) in text.lines().enumerate() {
                if let Some((_, _, value)) = scalar_on(line, "class") {
                    if value == old {
                        push_edit(&mut report.edits, &file, i + 1, old, new);
                    }
                }
            }
            let name = id.split('/').next_back().unwrap_or_default();
            report.moves.push(Edit {
                file: file.clone(),
                line: 0,
                from: format!("{}/{id}", report.corpus_dir),
                to: format!("{}/{new}/{name}", report.corpus_dir),
            });
        }

        // Links, from both sides. A link written by a moving instance keeps its
        // destination and gains a new origin; a link written at a moving instance keeps its
        // origin and gains a new destination. Both are `../<class>/<file>`, so both change
        // exactly when one end is inside the renamed directory.
        for (i, line) in text.lines().enumerate() {
            let Some((_, _, value)) = target_on(line) else {
                continue;
            };
            let resolved = slash(&resolve_target(Path::new(&id), &value));
            // What moves is the *class directory*, and every instance sits at exactly
            // `<class>/<file>` — so a rename preserves every instance's depth. A link
            // therefore changes when its TARGET moved, and never merely because its owner
            // did. Rewriting on the owner rebuilt links that had not changed, and rebuilt
            // them wrongly: `../../catalog/x.md` leaves the corpus, `normalize` swallows
            // the escape, and the round trip came back one level short.
            //
            // The class file is a target too. `instance-of` points at `../<class>.ont.yml`
            // from every instance in the class, and that file is being renamed with the
            // directory — missing it left every instance pointing at nothing.
            let moved = |p: &str| -> Option<String> {
                if let Some(rest) = p.strip_prefix(&format!("{old}/")) {
                    return Some(format!("{new}/{rest}"));
                }
                if p == format!("{old}.ont.yml") {
                    return Some(format!("{new}.ont.yml"));
                }
                None
            };
            let Some(target) = moved(&resolved) else {
                continue;
            };
            let owner = moved(&id).unwrap_or_else(|| id.clone());
            let rewritten = super::rename::relative_target(&owner, &target);
            if rewritten == value {
                continue;
            }
            push_edit(&mut report.edits, &file, i + 1, &value, &rewritten);
        }
    }

    report.unhandled = prose_mentions(root, &format!("{}/{old}/", report.corpus_dir));
}

fn plan_property_rename(
    root: &Path,
    corpus: &Path,
    class: &str,
    old: &str,
    new: &str,
    report: &mut MigrateReport,
) {
    if !check_class(corpus, class, report) {
        return;
    }
    if old == new {
        report.blocked.push("the two names are the same".into());
    }
    let declared = declared_properties(corpus, class);
    if !declared.iter().any(|(n, _)| n == old) {
        report
            .blocked
            .push(format!("`{class}` declares no property `{old}`"));
    }
    if declared.iter().any(|(n, _)| n == new) {
        report.blocked.push(format!(
            "`{class}` already declares `{new}` — renaming onto it would merge two properties"
        ));
    }
    if !report.blocked.is_empty() {
        return;
    }

    // The declaration.
    let ont = ont_path(corpus, class);
    let ont_rel = rel(root, &ont);
    let text = std::fs::read_to_string(&ont).unwrap_or_default();
    for (i, line) in text.lines().enumerate() {
        if let Some((_, _, value)) = scalar_on(line, "name") {
            if value == old {
                push_edit(&mut report.edits, &ont_rel, i + 1, old, new);
            }
        }
    }

    // Every instance carrying it. The property is a mapping *key* on the instance and a
    // `name:` value on the class — the same rename, two different shapes.
    for path in instances_of(corpus, class) {
        let file = rel(root, &path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if mapping_key_on(line, old).is_some() {
                push_edit(&mut report.edits, &file, i + 1, old, new);
            }
        }
    }
}

fn plan_value_rename(
    root: &Path,
    corpus: &Path,
    class: &str,
    property: &str,
    from: &str,
    to: &str,
    report: &mut MigrateReport,
) {
    if !check_class(corpus, class, report) {
        return;
    }
    if from == to {
        report.blocked.push("the two values are the same".into());
    }
    if to.is_empty() {
        report.blocked.push("the new value is empty".into());
    }
    if !declared_properties(corpus, class)
        .iter()
        .any(|(n, _)| n == property)
    {
        report
            .blocked
            .push(format!("`{class}` declares no property `{property}`"));
        return;
    }
    // The set as the gate reads it, so a value the declaration spells in a form this cannot
    // locate is refused below rather than half-renamed.
    let set = declared_values(corpus, class, property);
    if set.is_empty() {
        report.blocked.push(format!(
            "`{class}.{property}` declares no `values:` — there is no set to rename within"
        ));
    } else {
        if !set.iter().any(|v| v == from) {
            report.blocked.push(format!(
                "`{class}.{property}` declares no value `{from}` — its set is [{}]",
                set.join(", ")
            ));
        }
        if set.iter().any(|v| v == to) {
            report.blocked.push(format!(
                "`{class}.{property}` already declares `{to}` — renaming onto it would merge two values"
            ));
        }
    }
    if !report.blocked.is_empty() {
        return;
    }

    // The declaration: the one item of the property's `values:` list, in whichever of the
    // two list spellings the class wrote it. Scoped to the property by its `name:` line, as
    // the file is a list of property mappings and two of them may declare the same value.
    let ont = ont_path(corpus, class);
    let ont_rel = rel(root, &ont);
    let text = std::fs::read_to_string(&ont).unwrap_or_default();
    let mut current: Option<String> = None;
    let mut block: Option<usize> = None;
    let mut found = false;
    for (i, line) in text.lines().enumerate() {
        if let Some((_, _, name)) = scalar_on(line, "name") {
            current = Some(name);
            block = None;
            continue;
        }
        if current.as_deref() != Some(property) || line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if let Some(open) = block {
            if indent > open {
                if let Some((s, e)) = list_item_on(line, from) {
                    let written = &line[s..e];
                    push_edit(
                        &mut report.edits,
                        &ont_rel,
                        i + 1,
                        written,
                        &written_like(written, to),
                    );
                    found = true;
                }
                continue;
            }
            block = None;
        }
        if let Some((s, e)) = values_item_on(line, from) {
            let written = &line[s..e];
            push_edit(
                &mut report.edits,
                &ont_rel,
                i + 1,
                written,
                &written_like(written, to),
            );
            found = true;
        } else if line
            .trim_start()
            .strip_prefix("values:")
            .is_some_and(|rest| rest.trim().is_empty() || rest.trim_start().starts_with('#'))
        {
            block = Some(indent);
        }
    }
    if !found {
        // The parser saw the value and the line scan did not: a flow list broken across
        // lines, or a spelling this does not read. Refusing keeps the two halves together.
        report.blocked.push(format!(
            "`{class}.{property}` declares `{from}` in a form this cannot rewrite in place — \
             edit the `values:` list by hand"
        ));
        return;
    }

    // Every instance holding it, as written or inside its quotes. An instance holding some
    // other value is outside this rename: in the set, it stays; outside it, `property-type`
    // already reports it and a rename is not the repair.
    for path in instances_of(corpus, class) {
        let file = rel(root, &path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let Some((line, written)) = property_value_line(&text, property) else {
            continue;
        };
        if written == from || unquoted(&written) == Some(from) {
            push_edit(
                &mut report.edits,
                &file,
                line,
                &written,
                &written_like(&written, to),
            );
        }
    }
}

/// The line an instance writes a property's value on, and that value exactly as written.
///
/// Scoped to the top-level `properties:` block. A property's name can also be a key under
/// `links:`, or inside another property's own mapping, and a value edit that landed on one of
/// those would rewrite something the retype never named.
fn property_value_line(text: &str, property: &str) -> Option<(usize, String)> {
    let mut in_properties = false;
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            in_properties = line.trim_end() == "properties:";
            continue;
        }
        if !in_properties {
            continue;
        }
        if let Some((_, _, written)) = raw_scalar_on(line, property) {
            return Some((i + 1, written));
        }
    }
    None
}

/// What a retype can do with one instance value.
enum Requote {
    /// The bytes to write in place of the ones there now.
    Write(String),
    /// The other spelling was tried and is no better, and this says what it would have been
    /// and what is wrong with it.
    ///
    /// Worth a sentence of its own because `property-type`'s cannot serve here: its `number`
    /// arm reads *unquote it* whether or not unquoting helps, so a refusal that printed only
    /// the gate's finding would tell an author to do the thing this just declined to do.
    Refused(String),
    /// Nothing to try: no quotes to drop, and nothing a quote would change.
    Nothing,
}

/// Write one value the other way, or say why that cannot be done.
///
/// The two conversions `property-type` already names in its own findings: *unquote it* for a
/// `number` holding `"24"`, and *quote it* for a type that wants text holding a bare `24`. A
/// scalar has exactly one other spelling, so there is nothing here to choose between — which
/// is what separates this from the guesses the module note refuses.
///
/// The candidate is **parsed and re-checked** rather than reasoned about, and two things are
/// asked of it:
///
/// 1. The new type admits it, by [`super::lint::checks::property_type_violation`] — the
///    predicate that gates, for the reason the refusal in [`plan_property_retype`] gives.
///    Whether dropping a pair of quotes produces a number is a question about YAML and not
///    about quotes: serde_yaml reads `00060` and `1__0` as text, so unquoting either would
///    leave `property-type` reporting the instance it reported before.
/// 2. It still says what the corpus wrote. `"0x1A"` unquoted is the number **26** — serde_yaml
///    reads bases the gate's own parser does not — and a retype that turned a code into 26
///    while reporting success would be the invention this module exists to refuse.
fn requote(new_type: &str, value: &serde_yaml::Value, written: &str) -> Requote {
    use serde_yaml::Value;
    // The line has to be the one this value was read from. `written` comes from a text scan
    // and `value` from a parse of the whole document, and a key that is not the property's
    // own — or a block scalar, whose value is not on this line at all — would offer bytes
    // that say something else.
    if serde_yaml::from_str::<Value>(written).ok().as_ref() != Some(value) {
        return Requote::Nothing;
    }
    // Which way to write it is the *value's* shape and not the new type's: text in quotes has
    // quotes to lose, and a number or a boolean has bytes to quote. A bare string has neither,
    // which is why `cubic feet per second` retyped to `date` is refused rather than quoted —
    // it is already text, and the quotes were never what was wrong with it.
    let (candidate, verb) = match (value, unquoted(written)) {
        (Value::String(_), Some(bare)) => (bare.to_string(), "unquoting"),
        (Value::Number(_) | Value::Bool(_), None) => (format!("\"{written}\""), "quoting"),
        _ => return Requote::Nothing,
    };
    let Ok(parsed) = serde_yaml::from_str::<Value>(&candidate) else {
        return Requote::Nothing;
    };
    if super::lint::checks::property_type_violation(new_type, &parsed).is_some() {
        return Requote::Refused(format!(
            "{verb} it gives `{candidate}`, which `{new_type}` does not accept either"
        ));
    }
    let reads_back = match (&parsed, &value) {
        // Unquoted: the same quantity the text spelled, read by the parser the gate and
        // `query`'s ordering share rather than by a third one.
        (Value::Number(n), Value::String(text)) => {
            let read = super::lint::checks::numeric_value(&n.to_string());
            read.is_some() && read == super::lint::checks::numeric_value(text)
        }
        // Quoted: the bare bytes, now text — byte for byte, so `0x1A` becomes `"0x1A"` and
        // never `"26"`. What the corpus wrote is what it meant.
        (Value::String(s), _) => *s == written,
        _ => false,
    };
    if !reads_back {
        let read = serde_yaml::to_string(&parsed)
            .unwrap_or_default()
            .trim()
            .to_string();
        return Requote::Refused(format!(
            "{verb} it gives `{candidate}`, which YAML reads as `{read}` rather than as what \
             is written"
        ));
    }
    Requote::Write(candidate)
}

fn plan_property_retype(
    root: &Path,
    corpus: &Path,
    class: &str,
    property: &str,
    new_type: &str,
    report: &mut MigrateReport,
) {
    if !check_class(corpus, class, report) {
        return;
    }
    let declared = declared_properties(corpus, class);
    let Some((_, current)) = declared.iter().find(|(n, _)| n == property) else {
        report
            .blocked
            .push(format!("`{class}` declares no property `{property}`"));
        return;
    };
    if current == new_type {
        report
            .blocked
            .push(format!("`{class}.{property}` is already `{new_type}`"));
        return;
    }

    // The refusal, and the one conversion that is not a guess. Every instance's value is
    // tested against the new type by the predicate `property-type` gates on — so a migration
    // that succeeds leaves a corpus its own gate accepts, and one that would not is not
    // performed at all. A value the new type rejects only for how it is *written* is requoted
    // instead; see [`requote`].
    //
    // Held back rather than pushed as they are found: an instance further down may have no
    // conversion, and a retype that rewrote half a class before refusing would leave the
    // corpus in a state neither type describes.
    let mut values: Vec<(String, usize, String, String)> = Vec::new();
    // A `string` may declare a closed set (RFC-0044), and a retype *into* `string` is the
    // moment the set starts to bind — on the type the property had, it was carried and
    // ignored. Consulted for the reason the type predicate is: a migration that left an
    // instance outside the set would be a migration into a failing build. Read once, here,
    // and empty on every other type.
    let set = match new_type {
        "string" => declared_values(corpus, class, property),
        _ => vec![],
    };
    for path in instances_of(corpus, class) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let inst = crate::parse::parse_instance(&text);
        let Some(value) = inst
            .properties
            .as_ref()
            .and_then(|m| m.get(serde_yaml::Value::String(property.to_string())))
        else {
            continue;
        };
        // What the new type will see: the value as it stands, or as the requote writes it.
        // `None` is a value that was refused above, which needs no second finding.
        let admitted = match super::lint::checks::property_type_violation(new_type, value) {
            None => Some(value.clone()),
            Some(why) => {
                let located = property_value_line(&text, property);
                let outcome = located.as_ref().map_or(Requote::Nothing, |(_, written)| {
                    requote(new_type, value, written)
                });
                match (outcome, located) {
                    (Requote::Write(to), Some((line, from))) => {
                        let parsed = serde_yaml::from_str(&to).ok();
                        values.push((rel(root, &path), line, from, to));
                        parsed
                    }
                    // The gate's finding, then what this tried. Both, because the finding is
                    // what a reader will search for and the clause is the part that says what
                    // to do next.
                    (outcome, _) => {
                        let clause = match outcome {
                            Requote::Refused(why) => format!(": {why}"),
                            _ => String::new(),
                        };
                        report.blocked.push(format!(
                            "{}: `{property}` {why} — no mechanical conversion to \
                             `{new_type}`{clause}",
                            rel(root, &path)
                        ));
                        None
                    }
                }
            }
        };
        if let Some(why) = admitted
            .as_ref()
            .and_then(|v| super::lint::checks::declared_value_violation(&set, v))
        {
            // Not a conversion this can make: `resigned to enter Congress` against
            // `resigned` is a judgment about what the instance meant, which is the guess the
            // module note refuses. The two repairs are both the author's.
            report.blocked.push(format!(
                "{}: `{property}` {why} — widen `values:` on the declaration, or fix the value",
                rel(root, &path)
            ));
        }
    }
    if !report.blocked.is_empty() {
        report.blocked.push(
            "nothing was written. Fix these values first, then retype — a migration that \
             left the corpus failing its own gate would be a migration into a broken build."
                .into(),
        );
        return;
    }

    let ont = ont_path(corpus, class);
    let ont_rel = rel(root, &ont);
    let text = std::fs::read_to_string(&ont).unwrap_or_default();
    // The `type:` belonging to this property: the first one after its `- name:` line.
    let mut in_property = false;
    for (i, line) in text.lines().enumerate() {
        if let Some((_, _, value)) = scalar_on(line, "name") {
            in_property = value == property;
        }
        if in_property {
            if let Some((_, _, value)) = scalar_on(line, "type") {
                push_edit(&mut report.edits, &ont_rel, i + 1, &value, new_type);
                break;
            }
        }
    }
    if report.edits.is_empty() {
        report.blocked.push(format!(
            "`{class}.{property}` declares no `type:` to change — add one by hand"
        ));
        return;
    }
    // After the declaration, never before it: the test above is asking whether the class file
    // was rewritten, and an instance's edit would have answered it for something else.
    for (file, line, from, to) in values {
        push_edit(&mut report.edits, &file, line, &from, &to);
    }
}

fn plan_edge_retarget(
    root: &Path,
    corpus: &Path,
    class: &str,
    relationship: &str,
    new_target: &str,
    report: &mut MigrateReport,
) {
    if !check_class(corpus, class, report) {
        return;
    }
    if !ont_path(corpus, new_target).is_file() {
        report.blocked.push(format!(
            "`{new_target}` has no {new_target}.ont.yml — re-targeting at a class that does \
             not exist would make every edge a violation"
        ));
    }
    let Some(old_target) = declared_edge_target(corpus, class, relationship) else {
        if report.blocked.is_empty() {
            report.blocked.push(format!(
                "`{class}` declares no relationship `{relationship}`"
            ));
        }
        return;
    };
    if old_target == new_target {
        report.blocked.push(format!(
            "`{class}` — `{relationship}` already targets `{new_target}`"
        ));
    }
    if !report.blocked.is_empty() {
        return;
    }

    // The declaration on this class, and the mirror on the class at the other end. An edge
    // is documented from both ends; rewriting one leaves the ontology contradicting itself.
    for (path, want) in [
        (ont_path(corpus, class), old_target.as_str()),
        (ont_path(corpus, &old_target), class),
    ] {
        if !path.is_file() {
            continue;
        }
        let file = rel(root, &path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut in_edge = false;
        for (i, line) in text.lines().enumerate() {
            if let Some((_, _, value)) = scalar_on(line, "relationship") {
                in_edge = value == relationship;
            }
            if !in_edge {
                continue;
            }
            if let Some((_, _, value)) = scalar_on(line, "target") {
                if value == want {
                    push_edit(&mut report.edits, &file, i + 1, &value, new_target);
                }
                in_edge = false;
            }
        }
    }

    // What this migration creates and cannot do. Which instances should now point
    // elsewhere is a decision about the corpus, not about the ontology.
    for path in instances_of(corpus, class) {
        let id = slash(path.strip_prefix(corpus).unwrap_or(&path));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let inst = crate::parse::parse_instance(&text);
        for link in inst.links.as_deref().unwrap_or(&[]) {
            if link.relationship.as_deref() != Some(relationship) {
                continue;
            }
            let Some(target) = link.target.as_deref() else {
                continue;
            };
            let resolved = slash(&resolve_target(Path::new(&id), target));
            let lands_in = resolved.split('/').next().unwrap_or_default();
            if lands_in == new_target {
                continue;
            }
            report.violations.push(Violation {
                node: rel(root, &path),
                detail: format!(
                    "`{relationship}` still lands on `{target}`, a {lands_in} — re-point it \
                     at a {new_target} or drop the edge"
                ),
            });
        }
    }
}

/// `(name, type)` for each property the class declares.
fn declared_properties(corpus: &Path, class: &str) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(ont_path(corpus, class)).unwrap_or_default();
    let parsed = yidam_core::ontology::parse_class(class, &text);
    parsed
        .properties
        .into_iter()
        .map(|p| (p.name, p.property_type))
        .collect()
}

/// The closed set a property declares (RFC-0044), or empty.
fn declared_values(corpus: &Path, class: &str, property: &str) -> Vec<String> {
    let text = std::fs::read_to_string(ont_path(corpus, class)).unwrap_or_default();
    yidam_core::ontology::parse_class(class, &text)
        .properties
        .into_iter()
        .find(|p| p.name == property)
        .map(|p| p.values)
        .unwrap_or_default()
}

/// The class a relationship declares as its other end.
fn declared_edge_target(corpus: &Path, class: &str, relationship: &str) -> Option<String> {
    let text = std::fs::read_to_string(ont_path(corpus, class)).unwrap_or_default();
    yidam_core::ontology::parse_class(class, &text)
        .edges
        .into_iter()
        .find(|e| e.relationship == relationship)
        .map(|e| e.target)
}

fn instances_of(corpus: &Path, class: &str) -> Vec<PathBuf> {
    let dir = corpus.join(class);
    walk_corpus_instances(corpus)
        .into_iter()
        .filter(|p| p.starts_with(&dir))
        .collect()
}

/// Markdown mentioning a path this migration moves. Reported, never rewritten.
///
/// The same over-inclusive text scan `rename` performs, for the same reason: the reader is
/// being asked to check rather than to act, and a line naming the old path in an argument
/// is worth seeing even when it is not a link. `.yidam/.vendor/` is excluded — a finding
/// there is one nobody can act on.
fn prose_mentions(root: &Path, needle: &str) -> Vec<Unhandled> {
    let vendor = root.join(".yidam").join(".vendor");
    let mut out = Vec::new();
    for path in crate::walk::walk_linkable_files(&root.join(".yidam")) {
        if path.starts_with(&vendor) || path.extension().is_some_and(|x| x == "yml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if !line.contains(needle) {
                continue;
            }
            out.push(Unhandled {
                file: rel(root, &path),
                line: i + 1,
                text: line.trim().to_string(),
            });
        }
    }
    out
}

// ── applying ──────────────────────────────────────────────────────────────────

/// One record of one migration, written into `.yidam/migrations/`.
///
/// # Why not `.yidam/decisions/`
///
/// A decision record is *argued*: `context`, `decision`, `rationale`, written by somebody
/// who weighed alternatives. A migration record is *mechanical* — which operation ran, over
/// which files, rewriting what. Filing them together would make `decisions-log` a list of
/// two different kinds of thing, and would cost the property that makes a decision record
/// worth reading: that a person wrote it.
///
/// The decision *behind* a migration still belongs in `.yidam/decisions/`, and this record
/// can cite it. Two nodes and an edge is the graph working; one record doing both jobs is
/// the graph being avoided.
///
/// It also lets this shape be closed and typed, which a decision record deliberately is
/// not — a corpus is free to carry its own fields there and this is generated output.
#[derive(Debug, serde::Serialize)]
pub struct MigrationRecord {
    /// `class-rename`, `property-rename`, `property-retype`, `value-rename`, `edge-retarget`.
    pub operation: &'static str,
    pub summary: String,
    /// Files rewritten, repository-relative.
    pub files: Vec<String>,
    pub edits: usize,
    pub moves: usize,
    /// Instances this migration left in violation, if any. Recorded because the next reader
    /// of this file is the person who has to deal with them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<String>,
}

const RECORD_HEADER: &str = "\
# One ontology migration, as performed.
#
# GENERATED by `yidam migrate`. This is the mechanical half of the event: what was
# rewritten, where. The ARGUMENT for it — why the class was wrong — belongs in
# .yidam/decisions/, and this record is meant to be read beside one.
#
# Kept because a migration is otherwise invisible in the log: a wave of mechanical edits
# across forty files, under one commit subject, with nothing saying which operation
# produced them or what it refused to touch.
";

fn record_path(root: &Path, op: &Operation, slug_hint: &str) -> PathBuf {
    root.join(".yidam")
        .join("migrations")
        .join(format!("{}-{}.yml", op.kind(), slug_hint))
}

/// A filename-safe form of the thing being migrated.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            'a'..='z' | '0'..='9' => out.push(c),
            'A'..='Z' => out.extend(c.to_lowercase()),
            _ if !out.ends_with('-') && !out.is_empty() => out.push('-'),
            _ => {}
        }
    }
    out.trim_matches('-').to_string()
}

/// Apply the plan. Content edits land before any move, so nothing observes a half-state.
///
/// The same ordering `rename` uses and for the same reason: an edit is located by
/// `(file, line)`, and moving a file first would invalidate every path recorded against it.
fn apply(root: &Path, corpus: &Path, op: &Operation, report: &mut MigrateReport) -> Result<()> {
    // A reference lift rewrites byte spans inside prose and appends a block, neither of which
    // the `(file, line, from, to)` model below can express. Its own apply re-derives the plan
    // for the same reason this one re-locates every edit.
    if matches!(op, Operation::References) {
        // Both, and not `lifted` alone: a node that already declares a reference its tag still
        // spells out has a tag to collapse and nothing to write, and guarding on the write alone
        // would silently decline to do the half that remains.
        if report.lifted.is_empty() && report.edits.is_empty() {
            return Ok(());
        }
        super::migrate_references::apply(root, corpus, report)?;
        if !report.blocked.is_empty() {
            return Ok(());
        }
        // Return rather than fall through: the loop below re-reads and re-writes every file it
        // has an edit for, and re-writing a file this has already rewritten would join its lines
        // a second time over content the plan no longer describes.
        let files: BTreeSet<String> = report
            .lifted
            .iter()
            .map(|l| l.node.clone())
            .chain(report.edits.iter().map(|e| e.file.clone()))
            .collect();
        return write_record(root, op, report, files.into_iter().collect());
    }
    // A findings lift removes a paragraph from a block scalar *and* writes a record elsewhere in
    // the document. That is a whole-file rewrite rather than a smaller edit, which is why it did
    // not fit in #712 — and the same escape the reference lift takes above.
    if matches!(op, Operation::Findings) {
        if report.findings.is_empty() {
            return Ok(());
        }
        super::migrate_findings::apply(root, corpus, report)?;
        if !report.blocked.is_empty() {
            return Ok(());
        }
        let files: BTreeSet<String> = report.findings.iter().map(|l| l.node.clone()).collect();
        return write_record(root, op, report, files.into_iter().collect());
    }
    // A routes migration replaces a run of lines with a block of a different length, which a
    // one-for-one line edit cannot express. The same escape as the two lifts above.
    if matches!(op, Operation::Routes) {
        let Some(routes) = report.routes.clone().filter(|r| !r.already) else {
            return Ok(());
        };
        super::migrate_routes::apply(root, report)?;
        return write_record(root, op, report, vec![routes.file]);
    }
    // A scaffold migration wraps a span of lines in markers, and may move one: the same escape.
    if matches!(op, Operation::Scaffold) {
        if report.scaffold.as_ref().is_none_or(|s| s.nothing_to_do()) {
            return Ok(());
        }
        let written = super::migrate_scaffold::apply(root, report)?;
        return write_record(root, op, report, written);
    }
    let mut by_file: BTreeMap<&str, Vec<&Edit>> = Default::default();
    for e in &report.edits {
        by_file.entry(&e.file).or_default().push(e);
    }
    for (file, edits) in &by_file {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path)?;
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        for e in edits {
            let Some(line) = lines.get_mut(e.line - 1) else {
                continue;
            };
            // Re-locate rather than trusting a recorded span: the plan and the apply are two
            // reads of the same file, and rewriting a range that has moved would corrupt it.
            let span = ["target", "class", "name", "type", "relationship"]
                .iter()
                .find_map(|key| scalar_on(line, key).filter(|(_, _, v)| *v == e.from))
                .map(|(s, t, _)| (s, t))
                .or_else(|| {
                    // A retype's instance edits change the *quoting* of a value, on a key the
                    // class names rather than one of the five above: `length_km: "24"` becomes
                    // `length_km: 24`. The span is the value as written, quotes included —
                    // `scalar_on` reports the value inside them, and rewriting that range puts
                    // the same bytes back.
                    let Operation::PropertyRetype { property, .. } = op else {
                        return None;
                    };
                    raw_scalar_on(line, property)
                        .filter(|(_, _, v)| *v == e.from)
                        .map(|(s, t, _)| (s, t))
                })
                .or_else(|| {
                    // A value rename's edits are all whole tokens: the instance value as
                    // written, or one item of the declaration's list in either spelling.
                    let Operation::ValueRename { property, .. } = op else {
                        return None;
                    };
                    raw_scalar_on(line, property)
                        .filter(|(_, _, v)| *v == e.from)
                        .map(|(s, t, _)| (s, t))
                        .or_else(|| values_item_on(line, &e.from))
                        .or_else(|| list_item_on(line, &e.from))
                })
                .or_else(|| mapping_key_on(line, &e.from));
            let Some((start, end)) = span else { continue };
            line.replace_range(start..end, &e.to);
        }
        let mut out = lines.join("\n");
        if text.ends_with('\n') {
            out.push('\n');
        }
        std::fs::write(&path, out)?;
    }

    for m in &report.moves {
        let (from, to) = (root.join(&m.from), root.join(&m.to));
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if !super::rename::git_mv(root, &from, &to) {
            // Not a repository, or the file is untracked. Moving it is still the right
            // outcome — `git mv` is for history, not for correctness.
            std::fs::rename(&from, &to)?;
        }
    }

    // The now-empty class directory. Left behind, `walk_ont_files` ignores it and
    // `graph-check` never sees it — but a reader opening the corpus does, and an empty
    // directory named after a class that no longer exists reads as a class with no
    // instances rather than as debris.
    if let Operation::ClassRename { old, .. } = op {
        let dir = corpus.join(old);
        if dir.is_dir()
            && std::fs::read_dir(&dir)
                .map(|d| d.count() == 0)
                .unwrap_or(false)
        {
            let _ = std::fs::remove_dir(&dir);
        }
    }

    write_record(
        root,
        op,
        report,
        by_file.keys().map(|f| f.to_string()).collect(),
    )
}

/// The record, and the `applied` flag that says it was written.
fn write_record(
    root: &Path,
    op: &Operation,
    report: &mut MigrateReport,
    files: Vec<String>,
) -> Result<()> {
    let record = MigrationRecord {
        operation: op.kind(),
        summary: report.summary.clone(),
        files,
        edits: report.edits.len(),
        moves: report.moves.len(),
        violations: report
            .violations
            .iter()
            .map(|v| format!("{}: {}", v.node, v.detail))
            .collect(),
    };
    // `op.summary()` and not the report's: they are the same string for every ontology
    // operation, and a reference lift recomputes its summary from what it found — which would
    // put the corpus's finding counts in the record's filename and change it on every run.
    let path = record_path(root, op, &slug(&op.summary()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_yaml::to_string(&record)?;
    std::fs::write(&path, format!("{RECORD_HEADER}{body}"))?;
    report.record = rel(root, &path);
    report.applied = true;
    Ok(())
}

pub(crate) fn render_migrate(r: &MigrateReport) -> String {
    // Before the generic refusal, which cannot print the block a refusal hands over to paste.
    if matches!(r.operation, "routes") {
        return render_routes(r);
    }
    if matches!(r.operation, "scaffold") {
        return render_scaffold(r);
    }
    if !r.blocked.is_empty() {
        let mut out = format!("Cannot migrate — {}:\n", r.summary);
        for b in &r.blocked {
            let _ = writeln!(out, "  {b}");
        }
        return out.trim_end().to_string();
    }
    if matches!(r.operation, "references") {
        return render_references(r);
    }
    if matches!(r.operation, "findings") {
        return render_findings(r);
    }
    let mut out = format!(
        "{} {}\n{} edit(s) across {} file(s)\n",
        if r.applied {
            "Migrated"
        } else {
            "Would migrate"
        },
        r.summary,
        r.edits.len(),
        r.edits
            .iter()
            .map(|e| e.file.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    );
    for e in &r.edits {
        let _ = writeln!(out, "  {}:{}  {} → {}", e.file, e.line, e.from, e.to);
    }
    for m in &r.moves {
        let _ = writeln!(out, "  move  {} → {}", m.from, m.to);
    }
    if !r.violations.is_empty() {
        let _ = write!(
            out,
            "\n{} instance(s) now in violation — this migration cannot decide these:\n",
            r.violations.len()
        );
        for v in &r.violations {
            let _ = writeln!(out, "  {}: {}", v.node, v.detail);
        }
    }
    if !r.unhandled.is_empty() {
        let _ = write!(
            out,
            "\n{} prose reference(s) NOT rewritten — check these by hand:\n",
            r.unhandled.len()
        );
        for u in &r.unhandled {
            let _ = writeln!(out, "  {}:{}  {}", u.file, u.line, u.text);
        }
    }
    if !r.record.is_empty() {
        let _ = write!(out, "\nrecord: {}", r.record);
    }
    let _ = write!(out, "\ncommit: {}", r.commit_subject);
    out.trim_end().to_string()
}

/// A reference lift, by node rather than by edit.
///
/// One line per node and not one per reference: the largest measured corpus lifts 695 of them,
/// and a reader deciding whether to apply the migration is asking which nodes change and to
/// what — not to scroll a list as long as the corpus.
fn render_references(r: &MigrateReport) -> String {
    if r.lifted.is_empty() && r.edits.is_empty() {
        // Not "would migrate 0": this ran, and saying so is the difference between a corpus
        // with nothing to lift and a command that was never applied.
        return format!(
            "Nothing to lift — no evidence-tag detail names anything addressable that is not \
             already written down.\n{} detail(s) read as prose.",
            r.prose_details
        );
    }
    let mut by_node: BTreeMap<&str, (Vec<&str>, usize)> = Default::default();
    for l in &r.lifted {
        let entry = by_node.entry(&l.node).or_default();
        entry.0.push(&l.reference);
        if l.detail_cleared {
            entry.1 += 1;
        }
    }
    let mut out = format!(
        "{} {}\n{} reference(s) across {} node(s); {} tag(s) collapse to a bare standing\n",
        if r.applied {
            "Migrated"
        } else {
            "Would migrate"
        },
        r.summary,
        r.lifted.len(),
        by_node.len(),
        r.edits.len(),
    );
    for (node, (refs, collapses)) in &by_node {
        let _ = writeln!(
            out,
            "  {node}  {}{}",
            refs.join(", "),
            if *collapses > 0 {
                format!("  ({collapses} tag(s) collapsed)")
            } else {
                String::new()
            }
        );
    }
    if r.prose_details > 0 {
        let _ = write!(
            out,
            "\n{} detail(s) name nothing addressable and are left alone. \
             `yidam lint --explain reference-not-in-the-grammar` is where a detail that should \
             have resolved is reported.\n",
            r.prose_details
        );
    }
    if !r.blocked.is_empty() {
        let _ = write!(out, "\nNOT written:\n");
        for b in &r.blocked {
            let _ = writeln!(out, "  {b}");
        }
    }
    if !r.record.is_empty() {
        let _ = write!(out, "\nrecord: {}", r.record);
    }
    let _ = write!(out, "\ncommit: {}", r.commit_subject);
    out.trim_end().to_string()
}

/// A findings lift, by node rather than by finding.
///
/// One line per node, as [`render_references`] does and for the same reason: a reader deciding
/// whether to apply this is asking which nodes change and what question moves on each.
fn render_findings(r: &MigrateReport) -> String {
    if r.findings.is_empty() {
        // Not "would migrate 0": this ran, and the corpus holding no legacy paragraph is the
        // answer rather than the absence of one.
        let mut out =
            "Nothing to lift — no node carries a paragraph an earlier `propose` wrote.".to_string();
        if r.prose_findings > 0 {
            let _ = write!(
                out,
                "\n{} paragraph(s) reworded past recognition, left as prose.",
                r.prose_findings
            );
        }
        return out;
    }
    let mut by_node: BTreeMap<&str, Vec<&str>> = Default::default();
    for l in &r.findings {
        by_node.entry(&l.node).or_default().push(&l.check);
    }
    let mut out = format!(
        "{} {}\n{} finding(s) across {} node(s)\n",
        if r.applied {
            "Migrated"
        } else {
            "Would migrate"
        },
        r.summary,
        r.findings.len(),
        by_node.len(),
    );
    for (node, checks) in &by_node {
        let _ = writeln!(out, "  {node}  {}", checks.join(", "));
    }
    if r.prose_findings > 0 {
        let _ = write!(
            out,
            "\n{} paragraph(s) reworded past recognition and left as prose. Those sentences are \
             the author's now, and this migration will not guess where its own words ended.\n",
            r.prose_findings
        );
    }
    if !r.record.is_empty() {
        let _ = write!(out, "\nrecord: {}", r.record);
    }
    let _ = write!(out, "\ncommit: {}", r.commit_subject);
    out.trim_end().to_string()
}

/// A routes migration: the list it replaces, and the block — written, or to paste.
fn render_routes(r: &MigrateReport) -> String {
    let Some(routes) = &r.routes else {
        // Refused before there was a block to print: no `AGENTS.md`, or no routes vendored.
        let mut out = format!("Cannot migrate — {}:\n", r.summary);
        for b in &r.blocked {
            let _ = writeln!(out, "  {b}");
        }
        return out.trim_end().to_string();
    };
    if routes.already {
        // Not "would migrate 0 bullets": the answer is that the generator already reaches it.
        return format!(
            "Nothing to migrate — {} already carries the `yidam routes` block, and \
             `yidam regen` keeps it current.",
            routes.file
        );
    }
    if !r.blocked.is_empty() {
        let mut out = format!("Cannot migrate — {}:\n", r.summary);
        for b in &r.blocked {
            let _ = writeln!(out, "  {b}");
        }
        let _ = write!(
            out,
            "\n{} is left as it was. Paste this where its reading list belongs, then run \
             `yidam regen`:\n\n{}",
            routes.file, routes.block
        );
        return out;
    }
    let mut out = format!(
        "{} {}\n  {}:{}-{}  replaced by the block\n",
        if r.applied {
            "Migrated"
        } else {
            "Would migrate"
        },
        r.summary,
        routes.file,
        routes.line,
        routes.line + routes.lines - 1,
    );
    for b in &routes.replaced {
        let _ = writeln!(out, "    {b}");
    }
    if !r.record.is_empty() {
        let _ = write!(out, "\nrecord: {}", r.record);
    }
    let _ = write!(out, "\ncommit: {}", r.commit_subject);
    out.trim_end().to_string()
}

/// A scaffold migration: each file, and what its region takes in.
fn render_scaffold(r: &MigrateReport) -> String {
    let mut out = String::new();
    if !r.blocked.is_empty() {
        let _ = writeln!(out, "Cannot migrate — {}:", r.summary);
        for b in &r.blocked {
            let _ = writeln!(out, "  {b}");
        }
        let _ = write!(out, "\nNothing was written.");
        return out;
    }
    let Some(scaffold) = &r.scaffold else {
        return out;
    };
    if scaffold.nothing_to_do() {
        let _ = writeln!(out, "Nothing to migrate:");
    } else {
        let verb = if r.applied {
            "Migrated"
        } else {
            "Would migrate"
        };
        let _ = writeln!(out, "{verb} {}:", r.summary);
    }
    for f in &scaffold.files {
        if f.absent {
            let _ = writeln!(out, "  {}  absent, so there is nothing to mark", f.file);
        } else if f.already {
            let _ = writeln!(out, "  {}  already carries its region", f.file);
        } else if f.owned {
            let _ = writeln!(
                out,
                "  {}  carries none of the template's sections, so none of it is the re-vendor's",
                f.file
            );
        } else if f.wrapped.is_empty() {
            let _ = writeln!(
                out,
                "  {}  an empty region, which the next re-vendor fills with the corpus gate",
                f.file
            );
        } else {
            let _ = writeln!(
                out,
                "  {}  the region takes in: {}",
                f.file,
                f.wrapped.join(", ")
            );
        }
    }
    if scaffold.nothing_to_do() {
        return out.trim_end().to_string();
    }
    let _ = write!(
        out,
        "\nEverything inside a region is replaced by the scaffold's at the next \
         `mise run yidam-vendor-update`, so a step added inside one of those jobs or sections \
         shows up in that diff as removed. Move it to a job or section of its own, outside.\n"
    );
    if !r.record.is_empty() {
        let _ = write!(out, "\nrecord: {}", r.record);
    }
    let _ = write!(out, "\ncommit: {}", r.commit_subject);
    out.trim_end().to_string()
}

/// Perform one ontology migration.
pub fn migrate(op: Operation, dry_run: bool, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    crate::paths::require_yidam_repo(&root)?;
    let corpus = yidam_corpus_dir(&root);
    let mut report = plan(&root, &corpus, &op);

    if report.blocked.is_empty() && !dry_run {
        apply(&root, &corpus, &op, &mut report)?;
    }

    crate::report::finish(&root, format, &report, |r| {
        println!("{}", render_migrate(r))
    })?;
    if !report.blocked.is_empty() {
        anyhow::bail!("migrate: blocked");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(text: &str) -> serde_yaml::Value {
        serde_yaml::from_str(text).unwrap()
    }

    /// The conversion, or the clause explaining why there is none. `Nothing` reads as an empty
    /// clause so one assertion covers all three outcomes.
    fn tried(new_type: &str, written: &str) -> Result<String, String> {
        match requote(new_type, &yaml(written), written) {
            Requote::Write(to) => Ok(to),
            Requote::Refused(why) => Err(why),
            Requote::Nothing => Err(String::new()),
        }
    }

    /// The split that made a requote expressible. A rename wants the value inside the quotes
    /// and a requote wants the quotes themselves, and one reader with the stripping pulled out
    /// is how the two cannot come to disagree about where a value starts.
    #[test]
    fn the_two_readings_of_one_line_differ_only_in_the_quotes() {
        let line = "  length_km: \"24\"";
        let (start, end, raw) = raw_scalar_on(line, "length_km").unwrap();
        assert_eq!(raw, "\"24\"");
        assert_eq!(&line[start..end], "\"24\"");
        let (start, end, inner) = scalar_on(line, "length_km").unwrap();
        assert_eq!(inner, "24");
        assert_eq!(&line[start..end], "24");
    }

    /// The two spellings of a `values:` list read the same item to the same token: as written,
    /// quotes included, so a rename replaces the whole of it and never half a quoted value.
    #[test]
    fn a_values_item_is_located_in_either_list_spelling() {
        let flow = "    values: [extant, \"in operation\", ruin]  # closed";
        let (s, e) = values_item_on(flow, "in operation").unwrap();
        assert_eq!(&flow[s..e], "\"in operation\"");
        let (s, e) = values_item_on(flow, "ruin").unwrap();
        assert_eq!(&flow[s..e], "ruin");
        assert_eq!(values_item_on(flow, "extan"), None);
        assert_eq!(values_item_on(flow, "operation"), None);
        // A comma inside quotes does not split the item.
        let quoted = "    values: [\"a, b\", c]";
        let (s, e) = values_item_on(quoted, "a, b").unwrap();
        assert_eq!(&quoted[s..e], "\"a, b\"");

        let block = "      - 'in operation'  # the quoted one";
        let (s, e) = list_item_on(block, "in operation").unwrap();
        assert_eq!(&block[s..e], "'in operation'");
        assert_eq!(list_item_on("      - extant", "extant"), Some((8, 14)));
        assert_eq!(list_item_on("  - name: extant", "extant"), None);
    }

    /// The new spelling follows the old one, and a bare token is bare only where YAML reads it
    /// back as the same string: `true` renamed in bare would leave the set, not join it.
    #[test]
    fn a_renamed_value_keeps_its_quoting_and_stays_a_string() {
        assert_eq!(written_like("extant", "standing"), "standing");
        assert_eq!(
            written_like("\"in operation\"", "operating"),
            "\"operating\""
        );
        assert_eq!(written_like("'ruin'", "in ruins"), "'in ruins'");
        assert_eq!(written_like("extant", "in operation"), "in operation");
        assert_eq!(written_like("extant", "true"), "\"true\"");
        assert_eq!(written_like("extant", "24"), "\"24\"");
        assert_eq!(written_like("extant", "a, b"), "\"a, b\"");
        assert_eq!(written_like("extant", "say \"so\""), "'say \"so\"'");
    }

    /// A lone quote both opens and closes.
    #[test]
    fn a_one_character_value_is_not_a_quoted_one() {
        assert_eq!(unquoted("\""), None);
        assert_eq!(unquoted("'"), None);
        assert_eq!(unquoted("\"\""), Some(""));
        assert_eq!(unquoted("24"), None);
    }

    /// The finding in #1044: `property-type` says *unquote it*, and this carries that out.
    #[test]
    fn a_quoted_number_unquotes() {
        assert_eq!(tried("number", "\"24\""), Ok("24".to_string()));
        assert_eq!(tried("number", "'24.5'"), Ok("24.5".to_string()));
        assert_eq!(tried("number", "\"-1e3\""), Ok("-1e3".to_string()));
    }

    /// The mirror the issue left to decide. `property-type`'s `string` arm says *quote it*, so
    /// a retype that left the values bare would put the corpus in the same
    /// one-violation-per-instance state from the other side.
    #[test]
    fn a_bare_value_a_type_wants_as_text_is_quoted() {
        assert_eq!(tried("string", "24"), Ok("\"24\"".to_string()));
        assert_eq!(tried("text", "true"), Ok("\"true\"".to_string()));
        // A `date` refuses a bare year for the reason a `string` refuses a bare number, and
        // 71 instances in one derived corpus write one.
        assert_eq!(tried("date", "1985"), Ok("\"1985\"".to_string()));
        // Byte for byte: serde_yaml reads `0x1A` as 26, and the corpus wrote the hex.
        assert_eq!(tried("string", "0x1A"), Ok("\"0x1A\"".to_string()));
    }

    /// What stays a refusal. There is no number in `about 24` to unquote, and the tilde was
    /// carrying a meaning the number cannot.
    #[test]
    fn prose_that_mentions_a_number_is_not_one() {
        for written in ["\"about 24\"", "\"~24\"", "\"24 km\"", "\"\""] {
            assert!(
                tried("number", written).is_err(),
                "{written} was converted to a number"
            );
        }
    }

    /// A bare string is already text. Quoting it changes nothing, and the quotes were never
    /// what the `date` arm was complaining about — so this is `Nothing`, not a clause claiming
    /// something was attempted.
    #[test]
    fn a_bare_string_the_new_type_rejects_has_nothing_to_try() {
        assert_eq!(tried("date", "cubic feet per second"), Err(String::new()));
    }

    /// The first guard, and the reason it is a parse rather than an argument: dropping the
    /// quotes is not the same thing as producing a number. `00060` is text to serde_yaml, so
    /// unquoting it would leave `property-type` reporting the instance it reported before.
    #[test]
    fn an_unquoted_candidate_that_is_still_not_a_number_is_refused() {
        assert_eq!(
            tried("number", "\"00060\""),
            Err("unquoting it gives `00060`, which `number` does not accept either".to_string())
        );
        // `1__0` likewise, and `.inf` is a number serde_yaml reads and the gate will not admit.
        assert!(tried("number", "\"1__0\"").is_err());
        assert!(tried("number", "\".inf\"").is_err());
    }

    /// The second guard. serde_yaml reads bases the gate's own parser does not, so unquoting
    /// `"0x1A"` produces a valid number that is not the value the corpus wrote.
    #[test]
    fn an_unquoted_candidate_that_changes_the_value_is_refused() {
        let why = tried("number", "\"0x1A\"").unwrap_err();
        assert!(why.contains("reads as `26`"), "{why}");
        assert!(tried("number", "\"0b101\"").is_err());
    }

    /// The line has to be the value's own. A key repeated deeper in the document offers bytes
    /// that say something else, and rewriting those would corrupt a file the retype never named.
    #[test]
    fn a_line_that_does_not_read_back_as_the_value_is_refused() {
        assert!(matches!(
            requote("number", &yaml("\"24\""), "\"6\""),
            Requote::Nothing
        ));
        // A block scalar keeps its value on the lines after this one.
        assert!(matches!(
            requote("string", &yaml("24"), "|"),
            Requote::Nothing
        ));
    }

    #[test]
    fn the_value_line_is_the_one_under_properties() {
        let text = "class: reach\nproperties:\n  length_km: 24\n  regulated: \"yes\"\nlinks:\n  - target: ../x.yml\n    length_km: 9\n";
        assert_eq!(
            property_value_line(text, "length_km"),
            Some((3, "24".to_string()))
        );
        assert_eq!(property_value_line(text, "nonesuch"), None);
    }

    /// A property's name under `links:` and nowhere else is not a property value.
    #[test]
    fn a_key_outside_the_properties_block_is_not_read() {
        let text = "class: reach\nlinks:\n  - target: ../x.yml\n    length_km: 9\n";
        assert_eq!(property_value_line(text, "length_km"), None);
    }
}
