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
//! It was a property this module asserted and did not hold, in two directions at once, and
//! both are worth naming because neither is visible from the answer:
//!
//! - A revision reached the git argv unseparated, so `--at=--output=<path>` was an *option* by
//!   the time git read it and the read-only query truncated the named file. [`resolve`] has
//!   the reproduction; [`END_OF_OPTIONS`] is the fix, on all three invocations.
//! - The divergence note compared against `Graph::load` — a walk of `.yidam/corpus` on disk —
//!   while telling the reader it had compared against HEAD. `run_at` reconstructs HEAD through
//!   this module instead, so an uncommitted edit can neither create the note nor suppress it.
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

/// The separator after which git reads no more options. See [`resolve`] for what it prevents.
const END_OF_OPTIONS: &str = "--end-of-options";

/// A commit, resolved.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Revision {
    /// What the caller wrote — `HEAD~5`, `v0.2.0`, a bare sha.
    pub rev: String,
    pub commit: String,
    /// Committer date, ISO-8601.
    ///
    /// The committer's and not the author's. A series is read down this column looking for
    /// where the answer moved, and an author date is not monotonic across a rebase, a
    /// cherry-pick or an import — it would put the rows in an order the history never had.
    /// Same reason [`commits_in`] orders topologically instead of trusting a date sort.
    pub date: String,
}

/// Resolve a revision to a commit and its date, without moving anything.
///
/// # Why every git call here ends its options
///
/// A revision is caller input, and git parses a leading `-` as an option wherever the
/// argument lands. Without [`END_OF_OPTIONS`], `--at=--output=<path>` reached `git rev-list`
/// as `--output`, and a command whose headline property is that it *neither reads nor writes
/// the working tree* truncated the named file to zero bytes before printing an error about
/// the revision. `--between='--output=<path>..'` did the same past the `..` guard, then
/// reported an empty range at exit 0. Both are reproducible against a built binary, and both
/// are one argument away from being impossible — so all three invocations in this module pass
/// it, and none of them may stop.
pub fn resolve(root: &Path, rev: &str) -> Result<Revision> {
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "rev-list",
            "-n",
            "1",
            // A signature-verifying configuration prepends lines to the output of anything
            // that prints a commit. `commits_in` explains what that cost; this is cheaper
            // than finding out whether `rev-list` is one of them.
            "--no-show-signature",
            "--format=R %H %cI",
            END_OF_OPTIONS,
            rev,
        ])
        .output()
        .context("running git rev-list")?;
    if !out.status.success() {
        bail!(
            "`{rev}` is not a revision in this repository: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    // `--format` prints a `commit <sha>` header line first, then the format. The `R `
    // sentinel is what tells the two apart — and anything else git decides to print.
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text
        .lines()
        .find_map(|l| l.strip_prefix("R "))
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
            // **Topological, not the default reverse-chronological.** A branchy history sorted
            // by date interleaves the branches, and a series is read as *what the corpus held
            // at each step*: a merge of a branch that added two nodes while main added a third
            // produced rows `[a,b] → [a,d] → [a,b,c] → [a,b,c,d]`, in which `d` appears,
            // disappears and returns. Every one of those transitions is a change that never
            // happened, handed to a reader scanning for the commit where the answer moved.
            "--topo-order",
            // `log.showSignature=true` makes git prepend `Good "git" signature with …` to
            // every commit, and the parser below took `Good` for a sha and asked for the tree
            // at it. `change_stream` guards the same output with the same `C ` sentinel; this
            // copied the shape without the guard, so both are here — the flag so the lines are
            // never printed, the sentinel so a line nobody anticipated cannot be read as one.
            "--no-show-signature",
            "--format=C %H %cI",
            END_OF_OPTIONS,
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
            let mut f = line.strip_prefix("C ")?.split_whitespace();
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
///
/// **`-z`, because the default output is quoted.** A path holding a non-ASCII byte, a quote or
/// a backslash comes back as `".yidam/corpus/gage/caf\303\251.yml"` — quotes and all — and the
/// split below kept them, so [`is_instance`] failed on the leading `"` and the node silently
/// left the corpus. `--at HEAD` on a clean tree stopped being the identity, with no diagnostic
/// and at exit 0. `-z` terminates records with NUL and never quotes.
///
/// [`is_instance`]: crate::cmd::lint::history::is_instance
fn tree(root: &Path, commit: &str) -> Result<Vec<Entry>> {
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "ls-tree",
            "-r",
            "--full-tree",
            "-z",
            END_OF_OPTIONS,
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
        .split('\0')
        .filter(|record| !record.is_empty())
        .filter_map(|record| {
            // `<mode> <type> <sha>\t<path>`
            let (meta, path) = record.split_once('\t')?;
            let f: Vec<&str> = meta.split_whitespace().collect();
            // **On the mode, not the type.** A symlink is mode `120000` and type `blob`, and
            // its content is the link target — which `serde_yaml` parses into a class-less,
            // link-less node that exists at every revision and at no point in the working
            // tree, because `walk_corpus_instances` filters on `is_file()` and walkdir does
            // not follow links. Reading the type alone made `--at HEAD` on a clean tree
            // answer with a node the live walk had never heard of. `100…` is git's regular
            // file; a gitlink (`160000`) and a subtree are excluded by the same test, and
            // neither is a node either.
            match (f.first(), f.get(1)) {
                (Some(mode), Some(&"blob")) if mode.starts_with("100") => Some(Entry {
                    path: path.to_string(),
                    blob: f.get(2)?.to_string(),
                }),
                _ => None,
            }
        })
        .collect())
}

/// What a corpus tree entry is, to a reconstruction that reads only three kinds of file.
enum Kind {
    Instance,
    Class,
    Universal,
}

/// The kind of a repo-relative corpus path, or `None` for a file this module never opens.
///
/// The instance and class tests are the gate's own, so the reconstructed corpus and the walked
/// one cannot come to disagree about what a node is.
fn kind(path: &str) -> Option<Kind> {
    match path {
        p if is_instance(p) => Some(Kind::Instance),
        p if is_class(p) => Some(Kind::Class),
        ".yidam/corpus/universal.yml" => Some(Kind::Universal),
        _ => None,
    }
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

    /// Drop everything the commit just reconstructed does not name.
    ///
    /// The cache is here because consecutive commits share almost every blob, and that is
    /// also its bound: what the *next* commit will ask for is this commit's tree plus
    /// whatever it changed. Without this, peak memory over a range is every revision of every
    /// file the range touched — the corpus multiplied by the length of the history being
    /// asked about, which is the shape the cache exists to avoid paying anywhere else.
    fn retain(&mut self, shas: &[String]) {
        let keep: std::collections::HashSet<&str> = shas.iter().map(String::as_str).collect();
        self.0.retain(|sha, _| keep.contains(sha.as_str()));
    }

    /// The blob's text, or an error naming the object git would not return.
    ///
    /// **Not `unwrap_or_default`.** [`read_blobs`] returns an empty map if the subprocess
    /// cannot be spawned, stops at a short read and drops every remaining sha, and skips a
    /// blob that is not UTF-8. Mapping all of those to `""` handed `serde_yaml` an empty
    /// document, which parses: an I/O failure became a well-formed node with no class, no
    /// label and no links, and a corpus quietly missing whatever the failure covered — at
    /// exit 0. On the ontology side it was worse, because an emptied class blob makes that
    /// commit's schema look bare, `check` rejects the query as `unknown-class`, and
    /// [`divergence`] then attributes an I/O failure to the ontology's history in a confident
    /// sentence about a year that never happened.
    ///
    /// Borrowed rather than cloned: only a node's `text` needs to own the string, and a class
    /// or `universal.yml` is parsed and dropped.
    ///
    /// This is also why nothing here caches the negative. A missing object now stops the
    /// reconstruction instead of being asked for again at every commit of a range.
    fn get(&self, sha: &str, path: &str) -> Result<&str> {
        self.0.get(sha).map(String::as_str).ok_or_else(|| {
            anyhow::anyhow!(
                "the corpus tree names `{path}` as blob {sha} and git could not return it —                  the object is missing, unreadable, or not text. Answering without it would                  report a corpus this commit never held."
            )
        })
    }
}

impl Graph {
    /// The corpus as it stood at `commit`, reusing blobs already read. See [`Blobs`].
    ///
    /// Node paths are built as `root.join(rel)` — the spelling [`crate::cmd::lint::checks`]
    /// produces from a live walk — so relative link targets resolve identically. They are
    /// keys, not handles: nothing opens them.
    ///
    /// There is deliberately no `at(root, commit)` convenience taking a fresh cache. Both
    /// callers reconstruct HEAD as well as the commit asked about, and the two trees overlap
    /// almost entirely; a wrapper that started empty would have made the cheaper call the
    /// longer one to write.
    pub fn at_with(root: &Path, commit: &str, blobs: &mut Blobs) -> Result<Graph> {
        // Classified before anything is read, so the `cat-file` below asks for the objects
        // this reconstruction will actually parse. A corpus directory also holds READMEs,
        // ACTIONS.md files and attachments; requesting them read every one of them at every
        // commit of a range and kept them resident for the whole of it, to build a graph
        // that never looks at them.
        let entries: Vec<(Kind, Entry)> = tree(root, commit)?
            .into_iter()
            .filter_map(|e| kind(&e.path).map(|k| (k, e)))
            .collect();
        // Deduplicated: a corpus routinely holds two identical blobs, and a range holds the
        // same blob at every commit that did not change it.
        let mut shas: Vec<String> = entries.iter().map(|(_, e)| e.blob.clone()).collect();
        shas.sort_unstable();
        shas.dedup();
        blobs.fill(root, &shas);
        blobs.retain(&shas);

        let mut nodes = Vec::new();
        let mut classes = Vec::new();
        let mut universal = crate::universal::Universal::empty();
        for (kind, entry) in &entries {
            let content = blobs.get(&entry.blob, &entry.path)?;
            match kind {
                Kind::Instance => nodes.push(Node {
                    path: root.join(&entry.path),
                    rel: entry.path.clone(),
                    inst: serde_yaml::from_str(content).unwrap_or_default(),
                    text: content.to_string(),
                }),
                Kind::Class => {
                    let fields: ClassFields = serde_yaml::from_str(content).unwrap_or_default();
                    classes.push(Class::from_fields(&entry.path, fields));
                }
                // Neither an instance nor a class, and the check reads it for every property
                // predicate. A reconstruction that skipped it would reject property names a
                // corpus legitimately declared corpus-wide at that commit.
                Kind::Universal => universal = crate::universal::Universal::parse(content),
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
            root: root.to_path_buf(),
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
    let at = super::short(&revision.commit);
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
        //
        // Ordered, and the first arm is the one this function was silent on for the whole
        // stretch of history where the ontology moved most. `check` carves out a corpus with
        // no `.ont.yml`: every class name is accepted, `unknown-class` is never raised, and
        // the verdict is `Ok`. So at every commit before `.yidam/corpus` had a schema, both
        // sides came back `Ok` with no diagnostics, `codes(a) == codes(b)` held, and a
        // zero-result answer was reported as *nothing matched* when the truth is that nothing
        // was checked. That commit is precisely the one a reader asks about.
        (Ok(a), Ok(b)) => {
            let note = |message: String| {
                vec![super::check::Diagnostic {
                    level: "info",
                    step: 0,
                    code: "ontology-moved",
                    message,
                }]
            };
            if a.unschematised && !b.unschematised {
                return note(format!(
                    "the corpus declared no classes at {at}, so no name in this query was \
                     checked against it — the answer below is a walk, not a typed query. \
                     HEAD's ontology would check them."
                ));
            }
            if !a.unschematised && b.unschematised {
                return note(format!(
                    "HEAD's corpus declares no classes, so the same query would not be \
                     typechecked today. At {at} it was, and that is the verdict reported here."
                ));
            }
            // The class set a `*` step resolves to, which is what the answer is actually
            // drawn from. Two runs can agree on every diagnostic and disagree on this.
            if let Some(step) = (0..a.narrowed.len().max(b.narrowed.len()))
                .find(|i| a.narrowed.get(*i) != b.narrowed.get(*i))
            {
                let names = |c: &super::check::Checked| match c.narrowed.get(step) {
                    Some(classes) if !classes.is_empty() => classes.join(", "),
                    _ => "nothing".to_string(),
                };
                return note(format!(
                    "step {} narrows to {} at {at} and to {} at HEAD, so the same query is \
                     drawn from different classes. The ones reported here are that commit's.",
                    step + 1,
                    names(a),
                    names(b),
                ));
            }
            let codes = |c: &super::check::Checked| {
                c.diagnostics
                    .iter()
                    .map(|d| (d.step, d.code))
                    .collect::<Vec<_>>()
            };
            match codes(a) == codes(b) {
                true => Vec::new(),
                false => note(format!(
                    "the ontology has changed since {at}: this query typechecks at both, \
                     with different notes. The ones reported here are that commit's."
                )),
            }
        }
        // Rejected at both. Whether for the same reason or not, the answer is the same
        // answer and a second note about HEAD would be noise on top of a refusal.
        (Err(_), Err(_)) => Vec::new(),
    }
}
