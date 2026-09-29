//! `yidam rename` — moving a node without severing the edges into it.
//!
//! RFC-0014 proposal 2. Proposal 1 already landed: `dangling_edge` is `Severity::Error`, so a
//! corpus with a link to a nonexistent target *fails*. What was missing is the operation that
//! lets a legitimate rename not trip it — and until now the reverse rewrite was manual, which
//! made *"choose the name well"* the whole defence. The hazard is documented three times and
//! guarded zero.
//!
//! # Three edits, not one
//!
//! The obvious one is inbound: every other node's `target:` that resolves to the old path.
//!
//! The second is the moved node's **own outgoing** links. Instances all sit at
//! `corpus/<class>/<file>`, so a `../other/x.yml` survives a move between classes — but a
//! same-directory `./sibling.yml` or a bare `sibling.yml` does not, and only the moved file
//! knows about it. A rename that fixed every other file and broke the one it moved would be a
//! strange kind of correct.
//!
//! Only the ones the move actually breaks, though. The test is whether the target still lands
//! where it did once resolved from the destination — so a `./sibling.yml` in a node renamed
//! *within* its class passes it, and keeps the words its author wrote. Re-relativizing it to
//! the equivalent long form put a line in the diff nobody had asked for, in a commit whose
//! subject counted the other three (#920). `migrate` reached the same rule by its own route:
//! a link changes when its target moved, never merely because its owner did.
//!
//! The third is the file itself, via `git mv` where there is a repository, so history follows
//! the node rather than stopping at its old name.
//!
//! # What it deliberately does not do
//!
//! **It does not commit.** RFC-0014 asks for one atomic commit so the tree never passes
//! through the broken state the gate forbids. That property holds without committing: the
//! gate reads the working tree, every edit lands together, and the working tree is never
//! broken. Committing on a user's behalf — from an editor's F2, say — is a bigger surprise
//! than printing the message and letting them.
//!
//! **It does not rewrite prose links.** RFC-0014 scopes the rewrite to the corpus walk, and a
//! `[label](../concept/old.yml)` in a README is outside it. Those are *reported* rather than
//! silently left: the difference between "renamed" and "renamed and quietly broke the README"
//! is whether anybody was told.
//!
//! # Catalog entries
//!
//! A catalog entry is renamed the same way (#1159). The edges into it are every corpus
//! `source:` that resolves to it, and every quotation's `of:` (RFC-0046), in either spelling `edge-source-unresolved` admits: the
//! entry's stem, or a path written the way a `target:` is. Each is rewritten in the spelling
//! its author chose — a stem stays a stem, a path is re-relativized — because the two mean
//! the same thing and a rename is not the moment to change how a corpus writes it. Nothing
//! else about the entry changes, and there is no class to move it between: the catalog is one
//! directory deep, and a name with a slash in it is refused.
//!
//! One more thing is rewritten here that a node's rename leaves alone: a markdown link whose
//! target resolves to the entry, in any file under `.yidam/` that lint reads. A link from a
//! node's prose is the second of the three forms `catalog-uncited` and `verified-unsourced`
//! count as a citation, and the first real corpus this was tried on carried seventy of them
//! against thirty-seven `source:` lines; a link from another entry, a decision record or the
//! catalog README is one `broken-prose-link` fails the build over, at Error. A rename that
//! repaired the edges and severed the rest would have moved a source out from under every
//! `[verified]` claim that rested on it and failed the gate besides. The links are read with
//! the parser those checks read them with, so the two cannot disagree about what a link is.
//! What is left — a label, a table cell, a sentence that names the file without linking it —
//! is reported, as a node's prose is.
//!
//! The old name is reached as `catalog/old`, `catalog/old.md` or `.yidam/catalog/old.md`. A
//! class that is itself called `catalog` is reached by its repository-relative path, which is
//! the one spelling the two cannot share.

use anyhow::Result;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::lint::checks::{prose_links, source_targets};
use crate::corpus::{normalize, resolve_target};
use crate::paths::{repo_root, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_linkable_files};

/// One `target:` rewrite, located.
#[derive(Debug, serde::Serialize)]
pub struct Edit {
    /// Repository-relative.
    pub file: String,
    /// 1-based.
    pub line: usize,
    pub from: String,
    pub to: String,
}

/// A reference this command will not touch, and the caller should know about.
#[derive(Debug, serde::Serialize)]
pub struct Unhandled {
    pub file: String,
    pub line: usize,
    /// The line, trimmed. Prose links carry their own syntax and their own intent.
    pub text: String,
}

#[derive(Debug, serde::Serialize)]
pub struct RenameReport {
    /// What [`Self::from`] and [`Self::to`] are relative to: the corpus for a node, `.yidam`
    /// for a catalog entry, so that the moved file is always `corpus_dir/to`.
    pub corpus_dir: String,
    /// Corpus-relative source, or empty when it did not resolve.
    pub from: String,
    /// Corpus-relative destination.
    pub to: String,
    /// Whether the thing being moved is a catalog entry, whose inbound edges are `source:`
    /// values rather than `target:` ones. Not emitted: a consumer can read it off
    /// [`Self::corpus_dir`], and a new field is a contract change the wording is not worth.
    #[serde(skip)]
    pub(crate) catalog: bool,
    /// Whether anything was written. False for `--dry-run`, and false whenever `blocked` is
    /// non-empty.
    pub applied: bool,
    /// Repository-relative move. One entry, or none when blocked.
    pub moves: Vec<Edit>,
    pub edits: Vec<Edit>,
    /// Markdown references to the old path. Reported, never rewritten.
    pub unhandled: Vec<Unhandled>,
    /// Why this cannot proceed. Non-empty means nothing was touched.
    pub blocked: Vec<String>,
    /// A commit subject in the closed vocabulary.
    ///
    /// `migrate` and not `rename`: RFC-0014's own example says `rename:`, which is in no verb
    /// list — `lint --commits` reports it and `classify_commit` files an operational commit as
    /// Epistemic, the exact double cost GRAPH.md describes. `migrate` is "Data or schema
    /// moved", and GRAPH.md is explicit that reaching for the closest existing verb beats
    /// inventing one.
    pub commit_subject: String,
}

impl RenameReport {
    /// The moved node, repository-relative, at the name it ends up with.
    ///
    /// Every edit is reported under the path its file has once the command finishes, so this
    /// is the one path in [`Self::edits`] that belongs to the renamed node itself.
    fn moved_file(&self) -> String {
        format!("{}/{}", self.corpus_dir, self.to)
    }

    /// Edits in files other than the one being moved.
    ///
    /// The summary line and the commit subject both count *this*, so the number the reader
    /// sees printed and the number the commit message claims cannot disagree — they did, and
    /// the difference was a link the rename had no business touching.
    fn inbound(&self) -> impl Iterator<Item = &Edit> {
        let moved = self.moved_file();
        self.edits.iter().filter(move |e| e.file != moved)
    }
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Accept `concept/old.yml`, `concept/old`, or a repository-relative path.
///
/// The same tolerance `neighbors` and the MCP surface offer: an id is written by hand at least
/// as often as it is copied.
pub(crate) fn corpus_id(corpus_dir: &str, id: &str) -> String {
    let want = id.trim().trim_start_matches('/');
    let stripped = want.strip_prefix(&format!("{corpus_dir}/")).unwrap_or(want);
    match stripped.ends_with(".yml") {
        true => stripped.to_string(),
        false => format!("{stripped}.yml"),
    }
}

/// `concept/a.yml` seen from `gauge/b.yml` → `../concept/a.yml`.
///
/// Always relative and always explicit about the class directory, even for a sibling: a bare
/// `a.yml` is legal and the corpus writes the long form, so producing it keeps a rewritten
/// line looking like the ones around it.
pub(crate) fn relative_target(from_id: &str, to_id: &str) -> String {
    let from = from_id.split('/').collect::<Vec<_>>();
    let to = to_id.split('/').collect::<Vec<_>>();
    let up = "../".repeat(from.len().saturating_sub(1));
    format!("{up}{}", to.join("/"))
}

/// `catalog/old`, `catalog/old.md` or `.yidam/catalog/old.md` → `catalog/old.md`. A corpus
/// id, or anything else, is none.
///
/// `.yidam/corpus/catalog/x.yml` is a node in a class called `catalog`, and stays one: only
/// the leading `.yidam/` is stripped before the prefix is read.
pub(crate) fn catalog_id(id: &str) -> Option<String> {
    let want = id.trim().trim_start_matches('/');
    let want = want.strip_prefix(".yidam/").unwrap_or(want);
    let name = want.strip_prefix("catalog/")?;
    Some(catalog_entry(name))
}

/// A new name for a catalog entry, in every form the old one is accepted in and as a bare
/// stem besides: `new`, `new.md`, `catalog/new`, `.yidam/catalog/new.md` → `catalog/new.md`.
fn catalog_entry(name: &str) -> String {
    let name = name.trim().trim_start_matches('/');
    let name = name.strip_prefix(".yidam/").unwrap_or(name);
    let name = name.strip_prefix("catalog/").unwrap_or(name);
    match name.ends_with(".md") {
        true => format!("catalog/{name}"),
        false => format!("catalog/{name}.md"),
    }
}

/// `.yidam/catalog/new.md` seen from `.yidam/corpus/concept/x.yml` → `../../catalog/new.md`.
///
/// The general case of [`relative_target`], which can assume every node sits exactly one
/// class directory below the corpus. A catalog entry and a node share only `.yidam/`.
fn relative_from(node: &str, target: &str) -> String {
    let node: Vec<&str> = node.split('/').collect();
    let dir = &node[..node.len().saturating_sub(1)];
    let target: Vec<&str> = target.split('/').collect();
    let common = dir.iter().zip(&target).take_while(|(a, b)| a == b).count();
    format!(
        "{}{}",
        "../".repeat(dir.len() - common),
        target[common..].join("/")
    )
}

/// The `target:` value on this line, with its byte range, or none.
pub(crate) fn target_on(line: &str) -> Option<(usize, usize, String)> {
    value_on(line, "target")
}

/// The value of `key:` on this line, with its byte range, or none.
///
/// The key has to be in key position — nothing but indentation and a list dash before it —
/// because a `resource:` is not a `source:`, and `target_on` matched by substring for as long
/// as no other key ended in its name.
pub(crate) fn value_on(line: &str, key: &str) -> Option<(usize, usize, String)> {
    let needle = format!("{key}:");
    let at = line.find(&needle)?;
    if !line[..at].chars().all(|c| c.is_whitespace() || c == '-') {
        return None;
    }
    let after = at + needle.len();
    let rest = &line[after..];
    let lead = rest.len() - rest.trim_start().len();
    let mut value = rest.trim_start();
    // A trailing comment is not part of the path.
    if let Some(hash) = value.find(" #") {
        value = &value[..hash];
    }
    let value = value.trim_end();
    if value.is_empty() {
        return None;
    }
    let quoted = (value.starts_with('"') && value.ends_with('"') && value.len() > 1)
        || (value.starts_with('\'') && value.ends_with('\'') && value.len() > 1);
    let inner = if quoted {
        &value[1..value.len() - 1]
    } else {
        value
    };
    let start = after + lead + usize::from(quoted);
    Some((start, start + inner.len(), inner.to_string()))
}

pub(crate) fn plan(root: &Path, corpus: &Path, old: &str, new: &str) -> RenameReport {
    if let Some(from) = catalog_id(old) {
        return plan_catalog(root, corpus, from, new);
    }
    let corpus_dir = slash(corpus.strip_prefix(root).unwrap_or(corpus));
    let from = corpus_id(&corpus_dir, old);
    let to = corpus_id(&corpus_dir, new);
    let mut report = RenameReport {
        commit_subject: String::new(),
        corpus_dir,
        from: from.clone(),
        to: to.clone(),
        catalog: false,
        applied: false,
        moves: vec![],
        edits: vec![],
        unhandled: vec![],
        blocked: vec![],
    };

    let old_path = corpus.join(&from);
    let new_path = corpus.join(&to);
    // Instance, not merely a file under the corpus. `walk_corpus_instances`'s rule: depth ≥ 2,
    // and `<class>.ont.yml` at depth 1 is a class definition rather than a node.
    //
    // Found by a test on the LSP's `prepareRename`, which offered F2 on `concept.ont.yml` —
    // and this would have moved it. Renaming a class definition without renaming the directory
    // beside it breaks every instance in that class at once, because `graph-check` matches the
    // two by name. That is a different operation and it is not this one.
    let is_instance = from.contains('/') && !from.ends_with(".ont.yml");
    if !old_path.is_file() || !is_instance {
        report.blocked.push(format!("{from} is not a corpus node"));
    }
    if new_path.exists() {
        report.blocked.push(format!(
            "{to} already exists — renaming onto it would lose it"
        ));
    }
    if from == to {
        report
            .blocked
            .push("the two names are the same".to_string());
    }
    // A destination class with no `.ont.yml` is a `graph-check` failure the moment the file
    // lands. Refusing here beats moving the node and letting the gate explain it afterwards.
    if let Some(class) = to.split('/').next().filter(|c| !c.is_empty()) {
        if to.contains('/') && !corpus.join(format!("{class}.ont.yml")).is_file() {
            report
                .blocked
                .push(format!("class `{class}` has no {class}.ont.yml"));
        }
    }
    if !report.blocked.is_empty() {
        return report;
    }

    let moved_rel = report.moved_file();
    for path in walk_corpus_instances(corpus) {
        let id = slash(path.strip_prefix(corpus).unwrap_or(&path));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // The moved node's own links are re-relativized from where it lands; everybody else's
        // are rewritten only when they point at the node being moved. Either way the edit is
        // reported under the path its file has when the command finishes — for the mover that
        // is the new name, not the one it is leaving.
        let moving = id == from;
        let rel = match moving {
            true => moved_rel.clone(),
            false => slash(path.strip_prefix(root).unwrap_or(&path)),
        };
        for (i, line) in text.lines().enumerate() {
            let Some((_, _, value)) = target_on(line) else {
                continue;
            };
            let resolved = slash(&resolve_target(Path::new(&id), &value));
            let (owner, target) = match moving {
                // The mover: every target keeps its destination and gains a new origin. One of
                // them may be the node itself, which is a self-edge and stays one.
                true => (
                    to.clone(),
                    if resolved == from {
                        to.clone()
                    } else {
                        resolved
                    },
                ),
                false if resolved == from => (id.clone(), to.clone()),
                false => continue,
            };
            // A link the move does not break keeps the words its author wrote. The mover's
            // `./sibling.yml` still finds that sibling from a destination in the same class,
            // and rewriting it to the equivalent `../<class>/sibling.yml` put a line nobody
            // asked for into the diff — counted in the summary, absent from the commit
            // subject, and attributed to a file that no longer answered to that name.
            //
            // This subsumes the older "is the rewrite a no-op?" test: a target already
            // written the long way resolves to itself from the same origin.
            if slash(&resolve_target(Path::new(&owner), &value)) == target {
                continue;
            }
            report.edits.push(Edit {
                file: rel.clone(),
                line: i + 1,
                from: value,
                to: relative_target(&owner, &target),
            });
        }
    }

    // Prose links, reported and not rewritten.
    //
    // A text scan for the filename, deliberately over-inclusive: a line saying
    // `matt-huffman.yml:68` in an argument about edge symmetry is not a link and is still worth
    // seeing, because the reader is being asked to check rather than to act. Renaming one hub
    // node in a real corpus surfaced eleven of these across a catalog entry, a REGEN table, six
    // sangha positions, a resolution record and a skill — every one of which would have broken
    // silently.
    //
    // `.yidam/.vendor/` is excluded for the reason lint excludes it: it is read-only here, and a
    // finding there is one nobody can act on.
    let vendor = root.join(".yidam").join(".vendor");
    let stem = Path::new(&from)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    for path in walk_linkable_files(&root.join(".yidam")) {
        if path.starts_with(&vendor) || path.extension().is_some_and(|x| x == "yml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if !line.contains(&stem) {
                continue;
            }
            report.unhandled.push(Unhandled {
                file: slash(path.strip_prefix(root).unwrap_or(&path)),
                line: i + 1,
                text: line.trim().to_string(),
            });
        }
    }

    report.moves.push(Edit {
        file: slash(old_path.strip_prefix(root).unwrap_or(&old_path)),
        line: 0,
        from: from.clone(),
        to: to.clone(),
    });
    report.commit_subject = format!(
        "migrate: {from} → {to} ({} inbound link(s) rewritten)",
        report.inbound().count()
    );
    report
}

/// The keys whose value names a catalog entry: an edge's `source:`, and a quotation's `of:`
/// (RFC-0046), which lint resolves through the same function.
const CATALOG_KEYS: [&str; 2] = ["source", "of"];

/// The catalog half of [`plan`]. `from` is `catalog/old.md`, as [`catalog_id`] spells it.
///
/// Two kinds of edge come into a catalog entry, and both are rewritten. A `source:` on a
/// corpus edge, or a quotation's `of:`, resolves the way `edge-source-unresolved` resolves it — the bare stem, or a
/// path from the node's own directory — and is rewritten in the form it was written in. A
/// markdown link, anywhere under `.yidam/` that lint reads, resolves the way
/// `broken-prose-link` and `catalog-uncited` resolve it, through the same parser, and is
/// re-relativized from the file that holds it.
///
/// A `source:` that resolves to nothing is not touched — it was broken before the rename and
/// the rename did not break it — but the prose scan still shows it when it names the old
/// file, which is the reader's cue to check. So is anything else that names the old file
/// without linking to it: a link's label, a table cell, a sentence.
fn plan_catalog(root: &Path, corpus: &Path, from: String, new: &str) -> RenameReport {
    let to = catalog_entry(new);
    let mut report = RenameReport {
        commit_subject: String::new(),
        corpus_dir: ".yidam".to_string(),
        from: from.clone(),
        to: to.clone(),
        catalog: true,
        applied: false,
        moves: vec![],
        edits: vec![],
        unhandled: vec![],
        blocked: vec![],
    };

    let yidam = root.join(".yidam");
    let old_path = yidam.join(&from);
    let new_path = yidam.join(&to);
    let name = |id: &str| id.strip_prefix("catalog/").unwrap_or(id).to_string();
    let (old_name, new_name) = (name(&from), name(&to));
    // `README.md` is the one file in the directory the catalog walk does not read as an entry.
    let is_entry = |n: &str| n != "README.md" && !n.contains('/');
    if !old_path.is_file() || !is_entry(&old_name) {
        report
            .blocked
            .push(format!("{from} is not a catalog entry"));
    }
    if !is_entry(&new_name) {
        report.blocked.push(format!(
            "{to} is not a catalog entry name — the catalog is one directory deep"
        ));
    }
    if new_path.exists() {
        report.blocked.push(format!(
            "{to} already exists — renaming onto it would lose it"
        ));
    }
    if from == to {
        report
            .blocked
            .push("the two names are the same".to_string());
    }
    if !report.blocked.is_empty() {
        return report;
    }

    let new_stem = new_name.trim_end_matches(".md").to_string();
    let old_rel = format!(".yidam/{from}");
    let new_rel = format!(".yidam/{to}");
    for path in walk_corpus_instances(corpus) {
        let rel = slash(path.strip_prefix(root).unwrap_or(&path));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            let Some((_, _, value)) = CATALOG_KEYS.iter().find_map(|k| value_on(line, k)) else {
                continue;
            };
            // The two readings `edge-source-unresolved` admits, from the function it reads
            // them with, so this cannot rewrite a value that check would not have resolved.
            let dir = Path::new(&rel).parent().unwrap_or(Path::new(""));
            let [by_stem, by_path] = source_targets(dir, Path::new(".yidam/catalog"), &value);
            let rewritten = if slash(&by_stem) == old_rel {
                new_stem.clone()
            } else if slash(&by_path) == old_rel {
                relative_from(&rel, &new_rel)
            } else {
                continue;
            };
            report.edits.push(Edit {
                file: rel.clone(),
                line: i + 1,
                from: value,
                to: rewritten,
            });
        }
    }

    // Markdown links, rewritten; every other mention, reported. One walk, because the second
    // list is defined against the first: a line is shown when it names the old file more
    // often than it is rewritten, so a mention beside a handled link is not lost and a
    // handled link is not shown as both done and undone. The entry's own lines are under the
    // name it ends up with, as every edit is. `.yidam/.vendor/` is excluded for the reason
    // lint excludes it: it is read-only here, and a finding there is one nobody can act on.
    let vendor = yidam.join(".vendor");
    let moved_rel = report.moved_file();
    for path in walk_linkable_files(&yidam) {
        if path.starts_with(&vendor) {
            continue;
        }
        let rel = match path == old_path {
            true => moved_rel.clone(),
            false => slash(path.strip_prefix(root).unwrap_or(&path)),
        };
        let dir = Path::new(&rel).parent().unwrap_or(Path::new(""));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut handled: std::collections::HashMap<usize, usize> = report
            .edits
            .iter()
            .filter(|e| e.file == rel)
            .fold(Default::default(), |mut m, e| {
                *m.entry(e.line).or_default() += 1;
                m
            });
        for link in prose_links(&rel, dir, &text) {
            if slash(&normalize(&link.resolved)) != old_rel {
                continue;
            }
            *handled.entry(link.line).or_default() += 1;
            report.edits.push(Edit {
                file: rel.clone(),
                line: link.line,
                from: link.target,
                to: relative_from(&rel, &new_rel),
            });
        }
        for (i, line) in text.lines().enumerate() {
            let mentions = line.matches(old_name.as_str()).count();
            if mentions <= handled.get(&(i + 1)).copied().unwrap_or(0) {
                continue;
            }
            report.unhandled.push(Unhandled {
                file: rel.clone(),
                line: i + 1,
                text: line.trim().to_string(),
            });
        }
    }

    report.moves.push(Edit {
        file: slash(old_path.strip_prefix(root).unwrap_or(&old_path)),
        line: 0,
        from: from.clone(),
        to: to.clone(),
    });
    report.commit_subject = format!(
        "migrate: {from} → {to} ({} citation(s) rewritten)",
        report.inbound().count()
    );
    report
}

/// The byte range of `target` where this line writes `[label](target)` or
/// `[label](target#fragment)`, or none.
fn link_span(line: &str, target: &str) -> Option<(usize, usize)> {
    let at = line
        .find(&format!("]({target})"))
        .or_else(|| line.find(&format!("]({target}#")))?;
    let start = at + 2;
    Some((start, start + target.len()))
}

/// `pub(crate)` for `migrate`, which moves a whole class directory one instance at a time
/// and wants history to follow each node for the same reason a rename does.
pub(crate) fn git_mv(root: &Path, from: &Path, to: &Path) -> bool {
    crate::git::Git::new(root)
        .arg("mv")
        .paths([from, to])
        .succeeded()
}

/// Apply the plan.
///
/// The move goes first, because every edit is recorded under the path its file has when the
/// command finishes and one of those files is the node being moved. Ordering the other way
/// would mean translating that one path back to the name it is leaving, on every write.
///
/// Nothing observes a half-state either way: the gate reads the working tree, and the tree is
/// only read once the command returns.
fn apply(root: &Path, report: &mut RenameReport) -> Result<()> {
    let base = root.join(&report.corpus_dir);
    let old_path = base.join(&report.from);
    let new_path = base.join(&report.to);
    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !git_mv(root, &old_path, &new_path) {
        // Not a repository, or the file is untracked. Moving it is still the right outcome —
        // `git mv` is for history, not for correctness.
        std::fs::rename(&old_path, &new_path)?;
    }

    let mut by_file: std::collections::BTreeMap<&str, Vec<&Edit>> = Default::default();
    for e in &report.edits {
        by_file.entry(&e.file).or_default().push(e);
    }
    for (file, edits) in by_file {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path)?;
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        for e in edits {
            let Some(line) = lines.get_mut(e.line - 1) else {
                continue;
            };
            // Re-read rather than trusting the recorded span: the plan and the apply are two
            // reads of the same file, and rewriting a range that has moved would corrupt it.
            // A catalog entry's edits are `source:` and `of:` values and markdown links; the
            // last is found by its own syntax, and a line holding two of them is rewritten
            // twice, each pass finding the first one still spelled the old way.
            let keys: &[&str] = if report.catalog {
                &CATALOG_KEYS
            } else {
                &["target"]
            };
            let span = match keys.iter().find_map(|k| value_on(line, k)) {
                Some((start, end, value)) if value == e.from => Some((start, end)),
                _ if report.catalog => link_span(line, &e.from),
                _ => None,
            };
            let Some((start, end)) = span else {
                continue;
            };
            line.replace_range(start..end, &e.to);
        }
        let mut out = lines.join("\n");
        if text.ends_with('\n') {
            out.push('\n');
        }
        std::fs::write(&path, out)?;
    }

    report.applied = true;
    Ok(())
}

pub(crate) fn render_rename(r: &RenameReport) -> String {
    if !r.blocked.is_empty() {
        let mut out = format!("Cannot rename {} → {}:\n", r.from, r.to);
        for b in &r.blocked {
            let _ = writeln!(out, "  {b}");
        }
        return out.trim_end().to_string();
    }
    // Inbound and the mover's own are counted apart, because only inbound reaches the commit
    // subject. One number for both is a summary that says four over a message that says three.
    let inbound: Vec<&Edit> = r.inbound().collect();
    let mut out = format!(
        "{} {} → {}\n{} {} rewritten across {} file(s)\n",
        if r.applied { "Renamed" } else { "Would rename" },
        r.from,
        r.to,
        inbound.len(),
        if r.catalog {
            "citation(s)"
        } else {
            "inbound link(s)"
        },
        inbound
            .iter()
            .map(|e| e.file.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
    for e in &inbound {
        let _ = writeln!(out, "  {}:{}  {} → {}", e.file, e.line, e.from, e.to);
    }
    let moved = r.moved_file();
    let own: Vec<&Edit> = r.edits.iter().filter(|e| e.file == moved).collect();
    if !own.is_empty() {
        let _ = write!(
            out,
            "\n{} link(s) inside the moved {} re-relativized:\n",
            own.len(),
            if r.catalog { "entry" } else { "node" }
        );
        for e in &own {
            let _ = writeln!(out, "  {}:{}  {} → {}", e.file, e.line, e.from, e.to);
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
    let _ = write!(out, "\ncommit: {}", r.commit_subject);
    out.trim_end().to_string()
}

/// Rename a corpus node or a catalog entry, rewriting every edge into it.
pub fn rename(old: &str, new: &str, dry_run: bool, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let mut report = plan(&root, &corpus, old, new);

    if report.blocked.is_empty() && !dry_run {
        apply(&root, &mut report)?;
    }

    let blocked = !report.blocked.is_empty();
    crate::report::gate(&root, format, report, !blocked, |r| {
        println!("{}", render_rename(r))
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use tempfile::TempDir;

    fn corpus() -> (TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let corpus = root.join(".yidam/corpus");
        for class in ["concept", "gauge"] {
            std::fs::create_dir_all(corpus.join(class)).unwrap();
            std::fs::write(
                corpus.join(format!("{class}.ont.yml")),
                format!("class: {class}\nlabel: {class}\n"),
            )
            .unwrap();
        }
        (tmp, root, corpus)
    }

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn an_id_is_accepted_bare_or_with_the_corpus_prefix() {
        assert_eq!(corpus_id(".yidam/corpus", "concept/a"), "concept/a.yml");
        assert_eq!(corpus_id(".yidam/corpus", "concept/a.yml"), "concept/a.yml");
        assert_eq!(
            corpus_id(".yidam/corpus", ".yidam/corpus/concept/a.yml"),
            "concept/a.yml"
        );
    }

    #[test]
    fn a_target_is_written_the_long_way_even_for_a_sibling() {
        assert_eq!(
            relative_target("concept/a.yml", "concept/b.yml"),
            "../concept/b.yml"
        );
        assert_eq!(
            relative_target("concept/a.yml", "gauge/g.yml"),
            "../gauge/g.yml"
        );
    }

    #[test]
    fn every_inbound_edge_is_rewritten() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/old.yml"),
            "class: concept\nlabel: Old\n",
        );
        write(
            &corpus.join("concept/b.yml"),
            "class: concept\nlinks:\n  - target: ../concept/old.yml\n    relationship: r\n",
        );
        write(
            &corpus.join("gauge/g.yml"),
            "class: gauge\nlinks:\n  - target: ../concept/old.yml\n    relationship: r\n",
        );

        let mut r = plan(&root, &corpus, "concept/old", "concept/new");
        assert!(r.blocked.is_empty(), "{:?}", r.blocked);
        assert_eq!(r.edits.len(), 2);
        apply(&root, &mut r).unwrap();

        assert!(corpus.join("concept/new.yml").is_file());
        assert!(!corpus.join("concept/old.yml").exists());
        assert!(read(&corpus.join("concept/b.yml")).contains("target: ../concept/new.yml"));
        assert!(read(&corpus.join("gauge/g.yml")).contains("target: ../concept/new.yml"));
    }

    /// The half only the moved file knows about.
    ///
    /// A same-directory target survives a rename inside one class and breaks the moment the
    /// node moves to another. Fixing every other file and breaking the one you moved would be
    /// a strange kind of correct.
    #[test]
    fn the_moved_nodes_own_links_are_re_relativized() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/old.yml"),
            "class: concept\nlinks:\n  - target: sibling.yml\n    relationship: r\n",
        );
        write(&corpus.join("concept/sibling.yml"), "class: concept\n");

        let mut r = plan(&root, &corpus, "concept/old", "gauge/moved");
        assert!(r.blocked.is_empty(), "{:?}", r.blocked);
        apply(&root, &mut r).unwrap();

        let moved = read(&corpus.join("gauge/moved.yml"));
        assert!(
            moved.contains("target: ../concept/sibling.yml"),
            "the moved node still points at its old sibling: {moved}"
        );
    }

    /// The other half: a link the move does not break is left alone.
    ///
    /// `concept/low-flow.yml` pointed at `./base-flow-separation.yml`, and renaming it within
    /// `concept/` churned that into the equivalent `../concept/base-flow-separation.yml` —
    /// a fourth line in a diff whose commit message said three (#920).
    #[test]
    fn a_link_the_move_does_not_break_keeps_the_words_it_was_written_with() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/old.yml"),
            "class: concept\nlinks:\n  - target: ./sibling.yml\n    relationship: r\n",
        );
        write(&corpus.join("concept/sibling.yml"), "class: concept\n");

        let mut r = plan(&root, &corpus, "concept/old", "concept/new");
        assert!(
            r.edits.is_empty(),
            "nothing broke, so nothing moves: {:?}",
            r.edits
        );
        apply(&root, &mut r).unwrap();
        assert!(read(&corpus.join("concept/new.yml")).contains("target: ./sibling.yml"));
    }

    /// An edit inside the renamed file is reported at the name the reader will find it under.
    ///
    /// The plan runs before the move, so the obvious path to record is the one being left —
    /// and a reader who opened it found nothing there (#920).
    #[test]
    fn the_moved_nodes_own_edits_are_reported_under_its_new_path() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/old.yml"),
            "class: concept\nlinks:\n  - target: sibling.yml\n    relationship: r\n",
        );
        write(&corpus.join("concept/sibling.yml"), "class: concept\n");

        let r = plan(&root, &corpus, "concept/old", "gauge/moved");
        assert_eq!(r.edits.len(), 1);
        assert_eq!(r.edits[0].file, ".yidam/corpus/gauge/moved.yml");
    }

    /// The summary and the commit subject count the same thing.
    ///
    /// They disagreed: the summary counted every rewrite and the subject counted the inbound
    /// ones, so a rename that touched the moved node printed four over a message saying three
    /// with nothing to explain the gap (#920). Now the moved node's own rewrites are a second
    /// stanza with its own count.
    #[test]
    fn the_summary_count_and_the_commit_count_are_the_same_number() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/old.yml"),
            "class: concept\nlinks:\n  - target: sibling.yml\n    relationship: r\n",
        );
        write(&corpus.join("concept/sibling.yml"), "class: concept\n");
        write(
            &corpus.join("concept/b.yml"),
            "class: concept\nlinks:\n  - target: ../concept/old.yml\n    relationship: r\n",
        );

        let r = plan(&root, &corpus, "concept/old", "gauge/moved");
        assert_eq!(r.edits.len(), 2, "one inbound and one of the mover's own");
        let rendered = render_rename(&r);
        assert!(
            rendered.contains("1 inbound link(s) rewritten across 1 file(s)"),
            "{rendered}"
        );
        assert!(
            rendered.contains("(1 inbound link(s) rewritten)"),
            "the subject must carry the number the summary printed: {rendered}"
        );
        assert!(
            rendered.contains("1 link(s) inside the moved node re-relativized:"),
            "the rewrite the subject does not count must still be shown: {rendered}"
        );
    }

    /// A node linking to itself keeps linking to itself.
    #[test]
    fn a_self_edge_follows_the_node() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/old.yml"),
            "class: concept\nlinks:\n  - target: ../concept/old.yml\n    relationship: r\n",
        );
        let mut r = plan(&root, &corpus, "concept/old", "concept/new");
        apply(&root, &mut r).unwrap();
        assert!(read(&corpus.join("concept/new.yml")).contains("target: ../concept/new.yml"));
    }

    #[test]
    fn renaming_onto_an_existing_node_is_refused() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("concept/a.yml"), "class: concept\n");
        write(&corpus.join("concept/b.yml"), "class: concept\n");
        let r = plan(&root, &corpus, "concept/a", "concept/b");
        assert!(r.blocked.iter().any(|b| b.contains("already exists")));
        assert!(r.edits.is_empty(), "a blocked plan touches nothing");
    }

    /// A class definition is not a node.
    ///
    /// Renaming `<class>.ont.yml` without the directory beside it breaks every instance in
    /// that class at once — `graph-check` matches the two by name. Found by a test on the
    /// LSP's `prepareRename`, which offered F2 on one; this refused nothing until it did.
    #[test]
    fn a_class_definition_is_not_renameable_as_a_node() {
        let (_t, root, corpus) = corpus();
        let r = plan(&root, &corpus, "concept.ont.yml", "notion.ont.yml");
        assert!(r.blocked.iter().any(|b| b.contains("not a corpus node")));
    }

    #[test]
    fn renaming_something_that_is_not_a_node_is_refused() {
        let (_t, root, corpus) = corpus();
        let r = plan(&root, &corpus, "concept/ghost", "concept/x");
        assert!(r.blocked.iter().any(|b| b.contains("not a corpus node")));
    }

    /// Moving into a class with no `.ont.yml` is a `graph-check` failure the moment the file
    /// lands. Refusing beats moving the node and letting the gate explain it afterwards.
    #[test]
    fn moving_into_an_undeclared_class_is_refused() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("concept/a.yml"), "class: concept\n");
        let r = plan(&root, &corpus, "concept/a", "invented/a");
        assert!(r.blocked.iter().any(|b| b.contains("invented.ont.yml")));
    }

    /// Reported, never rewritten — and the report is the whole point.
    #[test]
    fn prose_references_are_reported_rather_than_silently_broken() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("concept/old.yml"), "class: concept\n");
        std::fs::write(
            root.join(".yidam/corpus/README.md"),
            "See [Old](concept/old.yml) for the details.\n",
        )
        .unwrap();
        let r = plan(&root, &corpus, "concept/old", "concept/new");
        assert_eq!(r.unhandled.len(), 1);
        assert!(r.unhandled[0].text.contains("concept/old.yml"));
        assert!(render_rename(&r).contains("NOT rewritten"));
    }

    /// The suggested subject has to be one `lint --commits` accepts.
    ///
    /// RFC-0014's own example says `rename:`, which is in no verb list — reported by the lint
    /// *and* filed as Epistemic, which is the double cost GRAPH.md names.
    #[test]
    fn the_suggested_commit_verb_is_in_the_vocabulary() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("concept/old.yml"), "class: concept\n");
        let r = plan(&root, &corpus, "concept/old", "concept/new");
        let verb = r.commit_subject.split(": ").next().unwrap();
        assert!(
            yidam_core::git::is_recognized_verb(verb),
            "suggested `{verb}:`, which the lint rejects"
        );
        assert_eq!(
            yidam_core::git::classify_commit("", &r.commit_subject).kind,
            yidam_core::git::CommitKind::Operational,
            "a rename is infrastructure, not a change in understanding"
        );
    }

    /// A dry run is a plan and nothing else.
    #[test]
    fn a_dry_run_touches_nothing() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("concept/old.yml"), "class: concept\n");
        write(
            &corpus.join("concept/b.yml"),
            "class: concept\nlinks:\n  - target: ../concept/old.yml\n    relationship: r\n",
        );
        let r = plan(&root, &corpus, "concept/old", "concept/new");
        assert!(!r.applied);
        assert_eq!(r.edits.len(), 1);
        assert!(corpus.join("concept/old.yml").is_file());
        assert!(read(&corpus.join("concept/b.yml")).contains("../concept/old.yml"));
    }

    /// Quoted targets and trailing comments are rewritten in place, not flattened.
    #[test]
    fn a_quoted_target_keeps_its_quotes_and_its_comment() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("concept/old.yml"), "class: concept\n");
        write(
            &corpus.join("concept/b.yml"),
            "class: concept\nlinks:\n  - target: \"../concept/old.yml\" # why\n    relationship: r\n",
        );
        let mut r = plan(&root, &corpus, "concept/old", "concept/new");
        apply(&root, &mut r).unwrap();
        assert!(
            read(&corpus.join("concept/b.yml")).contains("target: \"../concept/new.yml\" # why")
        );
    }

    /// The rewrite must not reflow a file it happens to touch.
    #[test]
    fn a_file_keeps_its_shape() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("concept/old.yml"), "class: concept\n");
        let original =
            "class: concept\n\n# a comment\nlinks:\n  - target: ../concept/old.yml\n    relationship: r\n";
        write(&corpus.join("concept/b.yml"), original);
        let mut r = plan(&root, &corpus, "concept/old", "concept/new");
        apply(&root, &mut r).unwrap();
        assert_eq!(
            read(&corpus.join("concept/b.yml")),
            original.replace("old.yml", "new.yml")
        );
    }

    /// A catalog entry is renamed with its own tolerance for how the name is written.
    #[test]
    fn a_catalog_entry_is_reached_by_any_of_its_spellings() {
        for id in [
            "catalog/old",
            "catalog/old.md",
            ".yidam/catalog/old.md",
            "/.yidam/catalog/old",
        ] {
            assert_eq!(catalog_id(id).as_deref(), Some("catalog/old.md"), "{id}");
        }
        assert_eq!(catalog_id("concept/old"), None);
        assert_eq!(
            catalog_id(".yidam/corpus/catalog/x.yml"),
            None,
            "a class called catalog"
        );
        for new in ["new", "new.md", "catalog/new", ".yidam/catalog/new.md"] {
            assert_eq!(catalog_entry(new), "catalog/new.md", "{new}");
        }
    }

    #[test]
    fn a_source_path_is_relative_to_the_node_that_writes_it() {
        assert_eq!(
            relative_from(".yidam/corpus/concept/x.yml", ".yidam/catalog/new.md"),
            "../../catalog/new.md"
        );
        assert_eq!(
            relative_from(".yidam/corpus/a/b/x.yml", ".yidam/catalog/new.md"),
            "../../../catalog/new.md"
        );
    }

    /// `resource:` is not `source:`, and a value that mentions the key is not the key.
    #[test]
    fn a_key_is_read_only_in_key_position() {
        assert_eq!(
            value_on("    source: acs # why", "source").map(|v| v.2),
            Some("acs".to_string())
        );
        assert_eq!(
            value_on("  - source: \"../../catalog/a.md\"", "source").map(|v| v.2),
            Some("../../catalog/a.md".to_string())
        );
        assert_eq!(value_on("    resource: acs", "source"), None);
        assert_eq!(value_on("    note: the source: of it", "source"), None);
        assert_eq!(
            target_on("  - target: ../concept/a.yml").map(|v| v.2),
            Some("../concept/a.yml".to_string())
        );
    }

    fn catalog(root: &Path, name: &str, text: &str) {
        let dir = root.join(".yidam/catalog");
        std::fs::create_dir_all(&dir).unwrap();
        write(&dir.join(name), text);
    }

    /// A quotation's `of:` names a catalog entry the way a `source:` does, and moves with it
    /// (RFC-0046) — a rename that left it behind would turn every quotation of the entry into
    /// `quotation-unresolved`.
    #[test]
    fn a_quotation_of_a_catalog_entry_is_rewritten_with_it() {
        let (_t, root, corpus) = corpus();
        catalog(&root, "old.md", "---\nobtained: true\n---\n# Old\n");
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\nanchors:\n  - of: old\n    span: the words\n  - of: ../../catalog/old.md\n    span: more words\n",
        );

        let mut r = plan(&root, &corpus, "catalog/old", "new");
        assert!(r.blocked.is_empty(), "{:?}", r.blocked);
        assert_eq!(r.edits.len(), 2, "{:?}", r.edits);
        assert!(r.unhandled.is_empty(), "{:?}", r.unhandled);
        apply(&root, &mut r).unwrap();

        let a = read(&corpus.join("concept/a.yml"));
        assert!(a.contains("  - of: new\n"), "{a}");
        assert!(a.contains("  - of: ../../catalog/new.md\n"), "{a}");
    }

    /// Both spellings `edge-source-unresolved` admits, each rewritten in the form it was
    /// written in (#1159).
    #[test]
    fn every_edge_source_into_a_catalog_entry_is_rewritten_in_its_own_spelling() {
        let (_t, root, corpus) = corpus();
        catalog(&root, "old.md", "---\nobtained: true\n---\n# Old\n");
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\nlinks:\n  - target: ../concept/b.yml\n    relationship: r\n    source: ../../catalog/old.md\n",
        );
        write(
            &corpus.join("gauge/g.yml"),
            "class: gauge\nlinks:\n  - target: ../concept/b.yml\n    relationship: r\n    source: old # by stem\n",
        );
        write(&corpus.join("concept/b.yml"), "class: concept\n");

        let mut r = plan(&root, &corpus, "catalog/old", "new");
        assert!(r.blocked.is_empty(), "{:?}", r.blocked);
        assert_eq!(r.corpus_dir, ".yidam");
        assert_eq!(
            (r.from.as_str(), r.to.as_str()),
            ("catalog/old.md", "catalog/new.md")
        );
        assert_eq!(r.edits.len(), 2, "{:?}", r.edits);
        assert!(r.unhandled.is_empty(), "{:?}", r.unhandled);
        assert_eq!(
            r.commit_subject,
            "migrate: catalog/old.md → catalog/new.md (2 citation(s) rewritten)"
        );
        apply(&root, &mut r).unwrap();

        assert!(root.join(".yidam/catalog/new.md").is_file());
        assert!(!root.join(".yidam/catalog/old.md").exists());
        assert!(read(&corpus.join("concept/a.yml")).contains("source: ../../catalog/new.md\n"));
        assert!(read(&corpus.join("gauge/g.yml")).contains("source: new # by stem\n"));
        assert!(!read(&corpus.join("concept/a.yml")).contains("target: ../concept/new"));
    }

    /// A `source:` that resolved to nothing is not the rename's to fix — but the reader is
    /// shown it when it names the old file, and a stem that merely contains the old one is
    /// not it.
    #[test]
    fn an_unresolving_source_is_left_but_shown() {
        let (_t, root, corpus) = corpus();
        catalog(&root, "old.md", "---\nobtained: true\n---\n");
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\nlinks:\n  - target: ../concept/a.yml\n    relationship: r\n    source: old.md\n  - target: ../concept/a.yml\n    relationship: r\n    source: older\n",
        );
        let r = plan(&root, &corpus, "catalog/old", "new");
        assert!(r.edits.is_empty(), "{:?}", r.edits);
        assert_eq!(r.unhandled.len(), 1, "{:?}", r.unhandled);
        assert!(r.unhandled[0].text.contains("source: old.md"));
    }

    /// A markdown link in a node's prose is a citation the gate reads, and is rewritten; `.md`
    /// prose is reported and never rewritten, the entry's own under the name it ends up with.
    /// A rewritten line is not also reported, unless it names the old file once more.
    #[test]
    fn prose_naming_a_catalog_entry_is_reported_under_the_name_it_ends_up_with() {
        let (_t, root, corpus) = corpus();
        catalog(
            &root,
            "old.md",
            "---\nobtained: true\n---\nSee also [itself](old.md).\n",
        );
        catalog(
            &root,
            "other.md",
            "---\nobtained: true\n---\nSupersedes [old](old.md), formerly old.md.\n",
        );
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\ndescription: Drawn from [old](../../catalog/old.md) and [again](../../catalog/old.md#L3), see old.md.\nlinks:\n  - target: ../concept/a.yml\n    relationship: r\n    source: ../../catalog/old.md\n",
        );
        let mut r = plan(&root, &corpus, "catalog/old", "new");
        assert_eq!(r.edits.len(), 5, "{:?}", r.edits);
        let files: Vec<(&str, usize)> = r
            .unhandled
            .iter()
            .map(|u| (u.file.as_str(), u.line))
            .collect();
        assert_eq!(
            files,
            vec![
                (".yidam/catalog/other.md", 4),
                (".yidam/corpus/concept/a.yml", 2),
            ],
            "{:?}",
            r.unhandled
        );
        let rendered = render_rename(&r);
        assert!(
            rendered.contains("4 citation(s) rewritten across 2 file(s)"),
            "{rendered}"
        );
        assert!(
            rendered.contains("1 link(s) inside the moved entry re-relativized:"),
            "{rendered}"
        );
        assert!(
            rendered.contains("2 prose reference(s) NOT rewritten"),
            "{rendered}"
        );
        apply(&root, &mut r).unwrap();
        let a = read(&corpus.join("concept/a.yml"));
        assert!(
            a.contains(
                "[old](../../catalog/new.md) and [again](../../catalog/new.md#L3), see old.md."
            ),
            "{a}"
        );
        assert!(a.contains("source: ../../catalog/new.md\n"), "{a}");
        assert!(read(&root.join(".yidam/catalog/new.md")).contains("[itself](new.md)"));
        assert!(
            read(&root.join(".yidam/catalog/other.md")).contains("[old](new.md), formerly old.md.")
        );
    }

    #[test]
    fn a_catalog_rename_is_refused_where_it_would_lose_or_nest_something() {
        let (_t, root, corpus) = corpus();
        catalog(&root, "a.md", "---\nobtained: true\n---\n");
        catalog(&root, "b.md", "---\nobtained: true\n---\n");
        let r = plan(&root, &corpus, "catalog/a", "b");
        assert!(
            r.blocked.iter().any(|b| b.contains("already exists")),
            "{:?}",
            r.blocked
        );
        let r = plan(&root, &corpus, "catalog/a", "sub/a");
        assert!(
            r.blocked.iter().any(|b| b.contains("one directory deep")),
            "{:?}",
            r.blocked
        );
        let r = plan(&root, &corpus, "catalog/ghost", "a");
        assert!(
            r.blocked.iter().any(|b| b.contains("not a catalog entry")),
            "{:?}",
            r.blocked
        );
        let r = plan(&root, &corpus, "catalog/README", "a");
        assert!(
            r.blocked.iter().any(|b| b.contains("not a catalog entry")),
            "{:?}",
            r.blocked
        );
        let r = plan(&root, &corpus, "catalog/a", "a.md");
        assert!(
            r.blocked.iter().any(|b| b.contains("the same")),
            "{:?}",
            r.blocked
        );
    }

    /// A catalog rename's suggested subject is in the vocabulary too.
    #[test]
    fn a_catalog_renames_commit_verb_is_in_the_vocabulary() {
        let (_t, root, corpus) = corpus();
        catalog(&root, "old.md", "---\nobtained: true\n---\n");
        let r = plan(&root, &corpus, "catalog/old", "new");
        assert!(r.blocked.is_empty(), "{:?}", r.blocked);
        let verb = r.commit_subject.split(": ").next().unwrap();
        assert!(yidam_core::git::is_recognized_verb(verb), "{verb}");
    }
}
