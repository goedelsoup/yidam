//! The corpus at a commit, reconstructed from blobs — `--at` and `--between` (#262).
//!
//! # Why this is not `lint::history::replay`
//!
//! #262 argues that `--at` is nearly free because `replay` already reconstructs the corpus at
//! a commit. It reconstructs *something* at a commit, and it is not this. RFC-0018 wrote the
//! gap down rather than discovering it here:
//!
//! | What a query at a commit needs | What `replay` does |
//! |---|---|
//! | that commit's ontology | [`is_instance`] excludes `.ont.yml` outright |
//! | declared properties, types, targets, `edge_policy` | [`blob_expectation`] deserializes **one** field from a class blob — `direction` |
//! | relationship names on edges | `targets_of` drops them: `.filter_map(\|l\| l.target.as_ref())` |
//! | a revision to stop at | `change_stream` runs `git log --reverse … -- .yidam/corpus` with no revision argument and no parameter to supply one |
//!
//! [`is_instance`]: crate::cmd::lint::history::is_instance
//! [`blob_expectation`]: crate::cmd::lint::history
//!
//! `replay` is the right *shape* and the wrong function. It folds forward over a stream of
//! changes and keeps, per commit, exactly what `orphan-in` needs to be dated: which nodes
//! exist and what points at what. A query needs the whole schema and the relationship names,
//! at one commit, and nothing about the commits before it.
//!
//! So this reads the **tree** at a revision instead of the log up to it. It is one
//! `git ls-tree -r` and one `git cat-file --batch` — the same two-subprocess shape `replay`
//! uses, for the same reason — and it costs the size of the corpus rather than the length of
//! history.
//!
//! # What it actually costs
//!
//! `--at` is one reconstruction and is not worth measuring. `--between` is one per commit in
//! range, and #262 called that nearly free, so here is the number: **2.4s in release for 79
//! commits over an 80-node corpus**, of which about a second is the 79 `ls-tree` subprocesses
//! and the rest is parsing each tree's YAML and running the walk. [`Blobs`] takes 3,240 object
//! reads down to 80 and saved a third of it; the remainder is the corpus being parsed once per
//! commit, which is what a series *is*. Not free, affordable, and linear in
//! `commits × corpus` rather than in history.
//!
//! # What is preserved
//!
//! The property #262 actually names: **nothing here touches the working tree.** No checkout,
//! no stash, no temporary index. `git ls-tree` and `git cat-file` read objects; the paths on
//! the reconstructed nodes are *keys*, joined and normalized to resolve edges, and never
//! opened. That is why [`crate::cmd::lint::checks::Node`] carries its own text: the one
//! remaining disk read on this path — `--select body` — would otherwise have answered from
//! whatever is checked out right now.
//!
//! Edges resolve through [`crate::cmd::query::exec`]'s own reader, which is the gate's, so
//! the historical graph and the current one cannot disagree about what points at what.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::Graph;
use crate::cmd::lint::checks::{Class, ClassFields, Node};
use crate::cmd::lint::history::{is_class, is_instance, read_blobs};

/// A commit, resolved.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Revision {
    /// What the caller wrote — `HEAD~5`, `v0.2.0`, a bare sha.
    pub rev: String,
    pub commit: String,
    /// Author date, ISO-8601, as `replay` prints it.
    pub date: String,
}

/// Resolve a revision to a commit and its date, without moving anything.
pub fn resolve(root: &Path, rev: &str) -> Result<Revision> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["rev-list", "-n", "1", "--format=%H %aI", rev])
        .output()
        .context("running git rev-list")?;
    if !out.status.success() {
        bail!(
            "`{rev}` is not a revision in this repository: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    // `--format` prints a `commit <sha>` header line first, then the format.
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text
        .lines()
        .find(|l| !l.starts_with("commit "))
        .unwrap_or_default();
    let mut fields = line.split_whitespace();
    let commit = fields.next().unwrap_or_default().to_string();
    let date = fields.next().unwrap_or_default().to_string();
    if commit.is_empty() {
        bail!("`{rev}` resolved to no commit");
    }
    Ok(Revision {
        rev: rev.to_string(),
        commit,
        date,
    })
}

/// Every commit in `a..b` that touched the corpus, oldest first.
///
/// Filtered by path, exactly as `change_stream` filters: a series with a row per commit in a
/// range would be mostly rows where nothing about the corpus changed, and a reader scanning
/// for the commit that changed the answer would be scanning past them.
pub fn commits_in(root: &Path, range: &str) -> Result<Vec<Revision>> {
    if !range.contains("..") {
        bail!(
            "`{range}` is not a range — `--between` takes `a..b`, and a single revision is \
             `--at`"
        );
    }
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "log",
            "--reverse",
            "--format=%H %aI",
            range,
            "--",
            ".yidam/corpus",
        ])
        .output()
        .context("running git log")?;
    if !out.status.success() {
        bail!(
            "`{range}` is not a range in this repository: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            let commit = f.next()?.to_string();
            Some(Revision {
                rev: commit.clone(),
                commit,
                date: f.next().unwrap_or_default().to_string(),
            })
        })
        .collect())
}

/// One entry in the corpus tree at a commit.
struct Entry {
    path: String,
    blob: String,
}

/// `git ls-tree -r <commit> -- .yidam/corpus`, as (path, blob) pairs.
fn tree(root: &Path, commit: &str) -> Result<Vec<Entry>> {
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "ls-tree",
            "-r",
            "--full-tree",
            commit,
            "--",
            ".yidam/corpus",
        ])
        .output()
        .context("running git ls-tree")?;
    if !out.status.success() {
        bail!(
            "reading the corpus tree at {commit}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            // `<mode> <type> <sha>\t<path>`
            let (meta, path) = line.split_once('\t')?;
            let f: Vec<&str> = meta.split_whitespace().collect();
            match f.get(1) {
                Some(&"blob") => Some(Entry {
                    path: path.to_string(),
                    blob: f.get(2)?.to_string(),
                }),
                // A submodule or a subtree. Neither is a node, and neither can be read as one.
                _ => None,
            }
        })
        .collect())
}

/// Blobs already read, kept across a series.
///
/// Consecutive commits share almost every blob — a commit that adds one node changes one
/// object and leaves the rest of the tree alone — so reconstructing each tree independently
/// re-reads the whole corpus once per commit. On an 80-commit fixture holding 80 nodes that
/// is 3,240 reads where 80 are distinct, and the cost is quadratic in a corpus that grows:
/// a repository at 700 commits and 200 nodes would read on the order of 10^5 blobs to answer
/// one series.
///
/// This is the half of #262's "nearly free" that *is* free, and the reason it is a cache
/// rather than the delta walk `replay` does: git already stores blobs by content, so
/// identity is a sha comparison and the whole optimisation is one `HashMap`. Reconstructing
/// incrementally would buy the remaining `ls-tree` per commit and cost a second
/// implementation of what a tree is.
#[derive(Default)]
pub struct Blobs(HashMap<String, String>);

impl Blobs {
    /// Read whichever of `shas` is not already held. One `cat-file` per commit, carrying
    /// only what that commit actually introduced.
    fn fill(&mut self, root: &Path, shas: &[String]) {
        let missing: Vec<String> = shas
            .iter()
            .filter(|s| !self.0.contains_key(*s))
            .cloned()
            .collect();
        if missing.is_empty() {
            return;
        }
        self.0.extend(read_blobs(root, &missing));
    }

    fn get(&self, sha: &str) -> String {
        self.0.get(sha).cloned().unwrap_or_default()
    }
}

impl Graph {
    /// The corpus as it stood at `commit`.
    ///
    /// Node paths are built as `root.join(rel)` — the spelling [`crate::cmd::lint::checks`]
    /// produces from a live walk — so relative link targets resolve identically. They are
    /// keys, not handles: nothing opens them.
    pub fn at(root: &Path, commit: &str) -> Result<Graph> {
        Graph::at_with(root, commit, &mut Blobs::default())
    }

    /// The same, reusing blobs already read. See [`Blobs`].
    pub fn at_with(root: &Path, commit: &str, blobs: &mut Blobs) -> Result<Graph> {
        let entries = tree(root, commit)?;
        let shas: Vec<String> = entries.iter().map(|e| e.blob.clone()).collect();
        blobs.fill(root, &shas);
        let text = |e: &Entry| blobs.get(&e.blob);

        let mut nodes = Vec::new();
        let mut classes = Vec::new();
        let mut universal = crate::universal::Universal::empty();
        for entry in &entries {
            let content = text(entry);
            if is_instance(&entry.path) {
                nodes.push(Node {
                    path: root.join(&entry.path),
                    rel: entry.path.clone(),
                    inst: serde_yaml::from_str(&content).unwrap_or_default(),
                    text: content,
                });
            } else if is_class(&entry.path) {
                let fields: ClassFields = serde_yaml::from_str(&content).unwrap_or_default();
                classes.push(Class::from_fields(&entry.path, fields));
            } else if entry.path == ".yidam/corpus/universal.yml" {
                // Neither an instance nor a class, and the check reads it for every property
                // predicate. A reconstruction that skipped it would reject property names a
                // corpus legitimately declared corpus-wide at that commit.
                universal = crate::universal::Universal::parse(&content);
            }
        }
        // Corpus order, which `walk_corpus_instances` sorts and every result ordering in this
        // surface depends on. `ls-tree` sorts by tree entry, which is close and is not the
        // same — and a golden pinned to one of them would be pinning git's ordering.
        nodes.sort_by(|a, b| a.path.cmp(&b.path));
        classes.sort_by(|a, b| a.rel.cmp(&b.rel));

        Ok(Graph {
            nodes,
            classes,
            universal,
            corpus_dir: ".yidam/corpus".to_string(),
            // A dependency's state at a past commit is not reconstructible from here: what
            // this repository has is whatever bundle is unpacked now, and a bundle carries no
            // history. `--across` and `--at` are refused together for that reason, in
            // `run_on`, rather than silently answering about today's dependencies.
            across: Vec::new(),
        })
    }
}

/// Where the same query would be judged differently at HEAD than at the commit it ran on.
///
/// #262 asks for this in one line — *"where the two disagree, say so rather than silently
/// using either"* — and the resolution RFC-0018 settled is that the **historical ontology
/// wins**, because it is the schema the historical data obeys. Saying so is this function's
/// whole job: a query that typechecks at a commit and would not today is answering correctly
/// and about a vocabulary that has since changed, and a reader who is not told that will read
/// the answer as current.
pub fn divergence(
    then: &Result<super::check::Checked, super::check::Rejection>,
    now: &Result<super::check::Checked, super::check::Rejection>,
    revision: &Revision,
) -> Vec<super::check::Diagnostic> {
    let at = &revision.commit[..revision.commit.len().min(8)];
    match (then, now) {
        (Ok(_), Err(rejection)) => vec![super::check::Diagnostic {
            level: "info",
            step: rejection.step.unwrap_or(0),
            code: "ontology-moved",
            message: format!(
                "this typechecks against the ontology at {at} and would be rejected against \
                 HEAD's ({}): {}",
                rejection.code, rejection.message
            ),
        }],
        (Err(rejection), Ok(_)) => vec![super::check::Diagnostic {
            level: "info",
            step: rejection.step.unwrap_or(0),
            code: "ontology-moved",
            message: format!(
                "this typechecks against HEAD's ontology and not against the one at {at}, \
                 which is the schema that commit's data obeys — so the rejection stands"
            ),
        }],
        // Both ran. A difference in what they *warned* about is the same fact one degree
        // quieter, and is worth one line rather than a second diagnostic per step: a
        // relationship undeclared then and declared now is the ordinary way an ontology grows.
        (Ok(a), Ok(b)) => {
            let codes = |c: &super::check::Checked| {
                c.diagnostics
                    .iter()
                    .map(|d| (d.step, d.code))
                    .collect::<Vec<_>>()
            };
            match codes(a) == codes(b) {
                true => Vec::new(),
                false => vec![super::check::Diagnostic {
                    level: "info",
                    step: 0,
                    code: "ontology-moved",
                    message: format!(
                        "the ontology has changed since {at}: this query typechecks at both, \
                         with different notes. The ones reported here are that commit's."
                    ),
                }],
            }
        }
        // Rejected at both. Whether for the same reason or not, the answer is the same
        // answer and a second note about HEAD would be noise on top of a refusal.
        (Err(_), Err(_)) => Vec::new(),
    }
}
