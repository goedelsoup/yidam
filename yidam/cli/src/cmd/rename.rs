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

use anyhow::Result;
use std::path::{Path, PathBuf};

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
    pub corpus_dir: String,
    /// Corpus-relative source, or empty when it did not resolve.
    pub from: String,
    /// Corpus-relative destination.
    pub to: String,
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

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Resolve `.` and `..` without touching the filesystem.
pub(crate) fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
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

/// The `target:` value on this line, with its byte range, or none.
pub(crate) fn target_on(line: &str) -> Option<(usize, usize, String)> {
    let key = line.find("target:")?;
    let after = key + "target:".len();
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
    let corpus_dir = slash(corpus.strip_prefix(root).unwrap_or(corpus));
    let from = corpus_id(&corpus_dir, old);
    let to = corpus_id(&corpus_dir, new);
    let mut report = RenameReport {
        commit_subject: String::new(),
        corpus_dir,
        from: from.clone(),
        to: to.clone(),
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

    for path in walk_corpus_instances(corpus) {
        let id = slash(path.strip_prefix(corpus).unwrap_or(&path));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let rel = slash(path.strip_prefix(root).unwrap_or(&path));
        // The moved node's own links are re-relativized from where it lands; everybody else's
        // are rewritten only when they point at the node being moved.
        let moving = id == from;
        for (i, line) in text.lines().enumerate() {
            let Some((_, _, value)) = target_on(line) else {
                continue;
            };
            let dir = Path::new(&id).parent().unwrap_or(Path::new(""));
            let resolved = slash(&normalize(&dir.join(&value)));
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
            let rewritten = relative_target(&owner, &target);
            if rewritten == value {
                continue;
            }
            report.edits.push(Edit {
                file: rel.clone(),
                line: i + 1,
                from: value,
                to: rewritten,
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
        report
            .edits
            .iter()
            .filter(|e| !e.file.ends_with(&from))
            .count()
    );
    report
}

/// `pub(crate)` for `migrate`, which moves a whole class directory one instance at a time
/// and wants history to follow each node for the same reason a rename does.
pub(crate) fn git_mv(root: &Path, from: &Path, to: &Path) -> bool {
    std::process::Command::new("git")
        .current_dir(root)
        .arg("mv")
        .arg(from)
        .arg(to)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Apply the plan. Every edit lands before the move, so nothing observes a half-state.
fn apply(root: &Path, corpus: &Path, report: &mut RenameReport) -> Result<()> {
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
            let Some((start, end, value)) = target_on(line) else {
                continue;
            };
            // Re-read rather than trusting the recorded span: the plan and the apply are two
            // reads of the same file, and rewriting a range that has moved would corrupt it.
            if value != e.from {
                continue;
            }
            line.replace_range(start..end, &e.to);
        }
        let mut out = lines.join("\n");
        if text.ends_with('\n') {
            out.push('\n');
        }
        std::fs::write(&path, out)?;
    }

    let old_path = corpus.join(&report.from);
    let new_path = corpus.join(&report.to);
    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !git_mv(root, &old_path, &new_path) {
        // Not a repository, or the file is untracked. Moving it is still the right outcome —
        // `git mv` is for history, not for correctness.
        std::fs::rename(&old_path, &new_path)?;
    }
    report.applied = true;
    Ok(())
}

pub(crate) fn render_rename(r: &RenameReport) -> String {
    if !r.blocked.is_empty() {
        let mut out = format!("Cannot rename {} → {}:\n", r.from, r.to);
        for b in &r.blocked {
            out.push_str(&format!("  {b}\n"));
        }
        return out.trim_end().to_string();
    }
    let mut out = format!(
        "{} {} → {}\n{} link(s) rewritten across {} file(s)\n",
        if r.applied { "Renamed" } else { "Would rename" },
        r.from,
        r.to,
        r.edits.len(),
        r.edits
            .iter()
            .map(|e| e.file.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
    for e in &r.edits {
        out.push_str(&format!("  {}:{}  {} → {}\n", e.file, e.line, e.from, e.to));
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
    out.push_str(&format!("\ncommit: {}", r.commit_subject));
    out.trim_end().to_string()
}

/// Rename a corpus node, rewriting every edge into it.
pub fn rename(old: &str, new: &str, dry_run: bool, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let mut report = plan(&root, &corpus, old, new);

    if report.blocked.is_empty() && !dry_run {
        apply(&root, &corpus, &mut report)?;
    }

    let blocked = !report.blocked.is_empty();
    if format.is_json() {
        crate::report::emit(&root, report)?;
    } else {
        println!("{}", render_rename(&report));
    }
    if blocked {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        apply(&root, &corpus, &mut r).unwrap();

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
        apply(&root, &corpus, &mut r).unwrap();

        let moved = read(&corpus.join("gauge/moved.yml"));
        assert!(
            moved.contains("target: ../concept/sibling.yml"),
            "the moved node still points at its old sibling: {moved}"
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
        apply(&root, &corpus, &mut r).unwrap();
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
        apply(&root, &corpus, &mut r).unwrap();
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
        apply(&root, &corpus, &mut r).unwrap();
        assert_eq!(
            read(&corpus.join("concept/b.yml")),
            original.replace("old.yml", "new.yml")
        );
    }
}
