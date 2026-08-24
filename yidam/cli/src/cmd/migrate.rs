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
//! | property retype | the declaration — and it **refuses** when an instance's value would not satisfy the new type |
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
//! The check that decides is [`crate::cmd::lint::checks::property_type_satisfied`] — the
//! same predicate `property-type` gates on, not a second reading of it. A migration that
//! disagreed with the gate about what a valid value is would be a migration into a failing
//! build.
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
use std::path::{Path, PathBuf};

use crate::paths::{repo_root, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_ont_files};

use super::rename::{normalize, target_on, Edit, Unhandled};

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
    /// A declared relationship points at a different class.
    EdgeRetarget {
        class: String,
        relationship: String,
        new_target: String,
    },
}

impl Operation {
    /// The kind, as the migration record and the report name it.
    pub fn kind(&self) -> &'static str {
        match self {
            Operation::ClassRename { .. } => "class-rename",
            Operation::PropertyRename { .. } => "property-rename",
            Operation::PropertyRetype { .. } => "property-retype",
            Operation::EdgeRetarget { .. } => "edge-retarget",
        }
    }

    /// One line, for the commit subject and the record's summary.
    pub fn summary(&self) -> String {
        match self {
            Operation::ClassRename { old, new } => format!("class `{old}` → `{new}`"),
            Operation::PropertyRename { class, old, new } => {
                format!("`{class}.{old}` → `{class}.{new}`")
            }
            Operation::PropertyRetype {
                class,
                property,
                new_type,
            } => format!("`{class}.{property}` is now `{new_type}`"),
            Operation::EdgeRetarget {
                class,
                relationship,
                new_target,
            } => format!("`{class}` — `{relationship}` now targets `{new_target}`"),
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
    /// `class-rename`, `property-rename`, `property-retype`, `edge-retarget`.
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

/// The value of a `key: value` line at any indent, with its byte range.
///
/// Deliberately not a YAML parse: this is used to rewrite one scalar in place, leaving
/// every other byte of the file — comments, block scalars, key order — exactly as written.
fn scalar_on(line: &str, key: &str) -> Option<(usize, usize, String)> {
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
    let quoted = value.len() > 1
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    let inner = if quoted {
        &value[1..value.len() - 1]
    } else {
        value
    };
    let start = indent + body.0 + key.len() + 1 + lead + usize::from(quoted);
    Some((start, start + inner.len(), inner.to_string()))
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
        Operation::EdgeRetarget {
            class,
            relationship,
            new_target,
        } => plan_edge_retarget(root, corpus, class, relationship, new_target, &mut report),
    }
    if report.blocked.is_empty() {
        report.commit_subject = format!(
            "migrate: {} ({} edit(s) across {} file(s))",
            op.summary(),
            report.edits.len(),
            report
                .edits
                .iter()
                .map(|e| e.file.as_str())
                .collect::<BTreeSet<_>>()
                .len()
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
            let dir = Path::new(&id).parent().unwrap_or(Path::new(""));
            let resolved = slash(&normalize(&dir.join(&value)));
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

    // The refusal. Every instance's value is tested against the new type by the predicate
    // `property-type` gates on — so a migration that succeeds leaves a corpus its own gate
    // accepts, and one that would not is not performed at all.
    for path in instances_of(corpus, class) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let inst: crate::parse::CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        let Some(value) = inst
            .properties
            .as_ref()
            .and_then(|m| m.get(serde_yaml::Value::String(property.to_string())))
        else {
            continue;
        };
        if let Some(why) = super::lint::checks::property_type_violation(new_type, value) {
            report.blocked.push(format!(
                "{}: `{property}` {why} — no mechanical conversion to `{new_type}`",
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
        let inst: crate::parse::CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        for link in inst.links.as_deref().unwrap_or(&[]) {
            if link.relationship.as_deref() != Some(relationship) {
                continue;
            }
            let Some(target) = link.target.as_deref() else {
                continue;
            };
            let dir = Path::new(&id).parent().unwrap_or(Path::new(""));
            let resolved = slash(&normalize(&dir.join(target)));
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
    /// `class-rename`, `property-rename`, `property-retype`, `edge-retarget`.
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

    let record = MigrationRecord {
        operation: op.kind(),
        summary: report.summary.clone(),
        files: by_file.keys().map(|f| f.to_string()).collect(),
        edits: report.edits.len(),
        moves: report.moves.len(),
        violations: report
            .violations
            .iter()
            .map(|v| format!("{}: {}", v.node, v.detail))
            .collect(),
    };
    let path = record_path(root, op, &slug(&report.summary));
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
    if !r.blocked.is_empty() {
        let mut out = format!("Cannot migrate — {}:\n", r.summary);
        for b in &r.blocked {
            out.push_str(&format!("  {b}\n"));
        }
        return out.trim_end().to_string();
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
        out.push_str(&format!("  {}:{}  {} → {}\n", e.file, e.line, e.from, e.to));
    }
    for m in &r.moves {
        out.push_str(&format!("  move  {} → {}\n", m.from, m.to));
    }
    if !r.violations.is_empty() {
        out.push_str(&format!(
            "\n{} instance(s) now in violation — this migration cannot decide these:\n",
            r.violations.len()
        ));
        for v in &r.violations {
            out.push_str(&format!("  {}: {}\n", v.node, v.detail));
        }
    }
    if !r.unhandled.is_empty() {
        out.push_str(&format!(
            "\n{} prose reference(s) NOT rewritten — check these by hand:\n",
            r.unhandled.len()
        ));
        for u in &r.unhandled {
            out.push_str(&format!("  {}:{}  {}\n", u.file, u.line, u.text));
        }
    }
    if !r.record.is_empty() {
        out.push_str(&format!("\nrecord: {}", r.record));
    }
    out.push_str(&format!("\ncommit: {}", r.commit_subject));
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

    if format.is_json() {
        crate::report::emit(&root, &report)?;
    } else {
        println!("{}", render_migrate(&report));
    }
    if !report.blocked.is_empty() {
        anyhow::bail!("migrate: blocked");
    }
    Ok(())
}
