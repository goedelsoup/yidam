//! Authoring an operational commit on the current branch.
//!
//! Every command that performs one of `GRAPH.md`'s pipeline verbs and records it in a tracked
//! file writes its commit here: the `catalog-*` commands (`refresh:`, `reconcile:`,
//! `extract:`), `regen --commit` (`regen:`), and `vault push --commit` (`index:`, `bundle:`).
//! It lived under `catalog/` until those last two arrived (#1215), and one writer rather than
//! four is the point: the refusal below is the permission argument, and a copy of it is a
//! second place for it to be weakened.
//!
//! # What licenses this to write a commit at all
//!
//! RFC-0026 states the invariant the orchestrator layer is built on, and it is the whole of
//! the permission argument here:
//!
//! > A run authors **operational** commits directly. Every **epistemic** commit it produces
//! > goes to a proposal branch, and nothing merges itself.
//!
//! Every verb above is in `OPERATIONAL_VERBS`, so each lands on the current branch. That is
//! not an assertion this module makes about itself — [`author`] classifies its own subject
//! line with `classify_commit`, the parity function with fixtures in three SDKs, and refuses
//! to write anything the vocabulary calls epistemic. The check is cheap, and it is what stops
//! a later patch from adding an `establish:` path here without going red.
//!
//! # Who the author is
//!
//! The caller says, as `(name, email)` — the shape `commit_tree` takes. Each command that
//! writes commits names itself (`yidam catalog`, `yidam regen`, `yidam vault`), for the reason
//! `propose` separates author from committer: the tool performed the act, a person ran it,
//! and on the current branch the author is the only thing distinguishing a tool's commit from
//! a person's own work. One shared identity would say which module wrote the commit and not
//! which act it was.
//!
//! # Why this does not write the way `propose` writes
//!
//! `cmd/propose/write.rs` builds commits against a temporary index and touches neither the
//! working tree nor `.git/index`, because it writes to a *different* branch: a person must be
//! able to run it mid-edit and see nothing change. RFC-0026 §5 points at those four
//! properties, and three of them are still wanted here.
//!
//! The fourth is not, and the difference is which branch is being written. An operational
//! commit lands on the branch the person is on, so afterwards their working tree must
//! *contain* the change — a `refresh:` commit recording a digest, with the entry on disk
//! still lacking it, would leave the repository reporting the artifact as uncommitted work.
//! So the edit is written to the working tree and committed from it under a pathspec, which
//! is the one git invocation that commits named paths and leaves everything else staged
//! exactly as it was.
//!
//! What replaces the safety propose gets from a temporary index is [`require_clean`]: the
//! files this is about to rewrite must already agree with `HEAD`. That is the same refusal
//! `require_committed_corpus` makes, narrowed from `.yidam/` to the paths actually touched —
//! a fetch has no business refusing to run because a corpus node is being edited elsewhere.
//!
//! # And on a cluster, it does
//!
//! The argument above is about *which branch is being written*, and in a pod no branch is.
//! A step pod computes a commit and hands back its sha; the lander, holding the only ref
//! credential, decides where it lands (RFC-0026 §7). There is no person whose working tree
//! must contain the change, and a working-tree commit would move the scratch clone's branch,
//! which is a ref the pod has no business writing even locally.
//!
//! So [`Writer`] has two modes. [`Writer::WorkingTree`] is everything above and the default
//! every shipped catalog command uses. [`Writer::Detached`] is `propose`'s machinery — a
//! [`TempIndex`] and [`commit_tree`] — chaining each commit onto the last from a pinned
//! parent and moving no ref at all. Both refuse an epistemic subject before anything is
//! written, through one check, [`refuse_epistemic`]; that refusal is the permission
//! argument, and a mode that skipped it would be the code path the cluster exists to not
//! depend on.

use std::path::Path;

use anyhow::{bail, Context, Result};
use yidam_core::git::{classify_commit, CommitKind};

use crate::cmd::propose::write::{commit_tree, git as git_at, short_of, TempIndex};

/// Who a commit written here is authored by: `(name, email)`. See the module doc.
pub type Who<'a> = (&'a str, &'a str);

/// A commit this module wrote.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    crate::git::Git::new(root).args(args).run()
}

/// Refuse when any of these paths differs from `HEAD`.
///
/// Checked *before* the edit, so the refusal names work a person has not committed rather
/// than work this command just did. Narrowed to the paths being written for the reason given
/// in the module header: `require_committed_corpus` gates the whole of `.yidam/` because a
/// proposal is drafted from a corpus-wide read, and none of this module's callers is.
pub fn require_clean(root: &Path, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["status", "--porcelain", "--"];
    args.extend(paths.iter().map(String::as_str));
    let dirty = git(root, &args)?;
    if dirty.trim().is_empty() {
        return Ok(());
    }
    bail!(
        "these files have uncommitted changes, and this command is about to commit them:\n{}\n\n  \
         Commit or stash them first — otherwise the operational commit would carry edits \
         nobody reviewed alongside what it went to record.",
        dirty
            .lines()
            .map(|l| format!("  {}", l.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Whether a subject line is one an unsupervised run may author.
///
/// The vocabulary's own split is the permission model — `GRAPH.md` defines the operational
/// family as *"the pipeline advanced; no understanding changed"*, which is exactly the class
/// of act a machine may perform without a person in the loop. Asking `classify_commit` rather
/// than matching a local list is deliberate: a second copy of the vocabulary here is one that
/// can drift from the one three SDKs are held to.
pub(crate) fn is_operational(subject: &str) -> bool {
    classify_commit("", subject).kind == CommitKind::Operational
}

/// Refuse a subject the vocabulary calls epistemic. Both write modes call this first.
pub(crate) fn refuse_epistemic(subject: &str) -> Result<()> {
    if is_operational(subject) {
        return Ok(());
    }
    bail!(
        "`{subject}` is not an operational subject line, and a run may not author one.\n  \
         RFC-0026: a run authors operational commits directly; every epistemic commit it \
         produces goes to a proposal branch, and nothing merges itself.\n  \
         The operational verbs are: {}",
        yidam_core::git::OPERATIONAL_VERBS.join(", ")
    )
}

fn message_of(subject: &str, body: &str) -> String {
    if body.trim().is_empty() {
        subject.to_string()
    } else {
        format!("{subject}\n\n{body}")
    }
}

/// Where a catalog command's commits go. See the module doc's last section.
pub enum Writer {
    /// The working tree and the current branch: the laptop, and every shipped command.
    WorkingTree,
    /// A temporary index and no ref: a pod, which hands back a sha for the lander.
    Detached(Detached),
}

/// The state of a detached write: the index commits are built in, and the tip they chain to.
pub struct Detached {
    /// Held for the writer's life: dropping it deletes the index.
    index: TempIndex,
    tip: String,
    parent: String,
    written: Vec<(String, Vec<u8>)>,
}

impl Detached {
    /// A writer whose first commit has `parent` as its parent, starting from `parent`'s tree.
    pub fn new(root: &Path, parent: &str) -> Result<Self> {
        let index = TempIndex::new(root, "catalog")?;
        git_at(root, Some(index.path()), &["read-tree", parent], None)
            .with_context(|| format!("reading {parent}'s tree into a temporary index"))?;
        Ok(Self {
            index,
            tip: parent.to_string(),
            parent: parent.to_string(),
            written: Vec::new(),
        })
    }

    /// The last commit written, or the parent when nothing was.
    pub fn tip(&self) -> &str {
        &self.tip
    }

    /// Whether any commit was written.
    pub fn moved(&self) -> bool {
        self.tip != self.parent
    }

    /// Every path written and the bytes it holds at the tip, last write winning.
    pub fn written(&self) -> &[(String, Vec<u8>)] {
        &self.written
    }
}

impl Writer {
    /// A working-tree writer's [`require_clean`]. A detached one reads the pinned commit and
    /// writes into an index of its own, so no uncommitted work can be swept into it.
    pub fn require_clean(&self, root: &Path, paths: &[String]) -> Result<()> {
        match self {
            Self::WorkingTree => require_clean(root, paths),
            Self::Detached(_) => Ok(()),
        }
    }

    /// Write `files` as one commit under `subject`, and return it.
    ///
    /// The subject is classified before a byte is written, in either mode. `None` when the
    /// files hold what is already committed — the contract [`author`] states.
    pub fn commit(
        &mut self,
        root: &Path,
        who: Who,
        subject: &str,
        body: &str,
        files: &[(String, String)],
    ) -> Result<Option<Commit>> {
        refuse_epistemic(subject)?;
        match self {
            Self::WorkingTree => {
                for (rel, content) in files {
                    let path = root.join(rel);
                    std::fs::write(&path, content)
                        .with_context(|| format!("writing {}", path.display()))?;
                }
                let paths: Vec<String> = files.iter().map(|(rel, _)| rel.clone()).collect();
                author(root, who, subject, body, &paths)
            }
            Self::Detached(d) => {
                if files.is_empty() {
                    return Ok(None);
                }
                let index = Some(d.index.path());
                for (rel, content) in files {
                    let blob = git_at(
                        root,
                        index,
                        &["hash-object", "-w", "--stdin"],
                        Some(content),
                    )?;
                    git_at(
                        root,
                        index,
                        &[
                            "update-index",
                            "--add",
                            "--cacheinfo",
                            &format!("100644,{blob},{rel}"),
                        ],
                        None,
                    )?;
                }
                let tree = git_at(root, index, &["write-tree"], None)?;
                let before = git_at(
                    root,
                    None,
                    &["rev-parse", &format!("{}^{{tree}}", d.tip)],
                    None,
                )?;
                if tree == before {
                    return Ok(None);
                }
                let sha = commit_tree(root, &tree, &d.tip, &message_of(subject, body), who)?;
                d.tip = sha.clone();
                for (rel, content) in files {
                    d.written.retain(|(p, _)| p != rel);
                    d.written.push((rel.clone(), content.clone().into_bytes()));
                }
                Ok(Some(Commit {
                    sha: short_of(root, &sha),
                    subject: subject.to_string(),
                }))
            }
        }
    }
}

/// Write one commit containing exactly `paths`, and return it.
///
/// `None` when the paths hold nothing to commit — a re-run that found the same bytes edits
/// nothing, and an empty commit asserting a fetch happened would be the *"provenance invented
/// rather than recorded"* failure RFC-0026 opens with.
pub fn author(
    root: &Path,
    who: Who,
    subject: &str,
    body: &str,
    paths: &[String],
) -> Result<Option<Commit>> {
    refuse_epistemic(subject)?;
    if paths.is_empty() {
        return Ok(None);
    }

    let mut staged = vec!["add", "--"];
    staged.extend(paths.iter().map(String::as_str));
    git(root, &staged)?;

    // Against HEAD and restricted to these paths: this is what says "has anything actually
    // changed" without consulting the rest of the index, which may hold a person's staged
    // work that is none of this command's business.
    let mut diff = vec!["diff", "--cached", "--name-only", "HEAD", "--"];
    diff.extend(paths.iter().map(String::as_str));
    if git(root, &diff)?.trim().is_empty() {
        return Ok(None);
    }

    let message = message_of(subject, body);
    // `commit -- <paths>` builds the commit from HEAD plus these paths only, leaving anything
    // else in the index staged and uncommitted. That is the property that makes this safe to
    // run in a repository somebody is working in.
    crate::git::Git::new(root)
        .args(["commit", "--no-verify", "-m", &message])
        .paths(paths)
        .env("GIT_AUTHOR_NAME", who.0)
        .env("GIT_AUTHOR_EMAIL", who.1)
        .run()?;
    Ok(Some(Commit {
        sha: git(root, &["rev-parse", "--short", "HEAD"])?,
        subject: subject.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::git::fixture::init(root);
        crate::git::fixture::write(root, ".yidam/catalog/a.md", "---\nname: a\n---\n\n# A\n");
        crate::git::fixture::commit(root, "scaffold: a catalog");
        dir
    }

    const ENTRY: &str = ".yidam/catalog/a.md";

    /// A caller's identity. Not any real command's, so a test here cannot pass by agreeing
    /// with a constant this module no longer owns.
    const WHO: Who = ("yidam test", "test@yidam");

    const EPISTEMIC: [&str; 4] = [
        "establish: a new concept",
        "resolve: the tension",
        "question: what is this",
        "no verb at all",
    ];

    fn edited() -> Vec<(String, String)> {
        vec![(
            ENTRY.to_string(),
            "---\nname: a\nobtained: true\n---\n\n# A\n".to_string(),
        )]
    }

    /// Whether the object store holds the blob `content` would hash to.
    fn holds_blob(root: &Path, content: &str) -> bool {
        let id = crate::git::Git::new(root)
            .args(["hash-object", "--stdin"])
            .stdin(content.as_bytes().to_vec())
            .run()
            .unwrap();
        crate::git::Git::new(root)
            .args(["cat-file", "-e", &id])
            .succeeded()
    }

    /// The same invariant in the pod's mode, and the refusal comes before the object store is
    /// touched: a refused subject leaves no blob behind for anything to point a ref at.
    #[test]
    fn a_detached_writer_refuses_an_epistemic_subject_before_writing() {
        let dir = repo();
        let root = dir.path();
        let head = git(root, &["rev-parse", "HEAD"]).unwrap();
        for subject in EPISTEMIC {
            let mut w = Writer::Detached(Detached::new(root, &head).unwrap());
            let err = w.commit(root, WHO, subject, "", &edited()).unwrap_err();
            assert!(
                err.to_string().contains("not an operational subject line"),
                "{subject} should be refused: {err}"
            );
            let Writer::Detached(d) = &w else {
                unreachable!()
            };
            assert!(!d.moved(), "{subject}: nothing was written");
            assert!(
                !holds_blob(root, &edited()[0].1),
                "{subject}: the refusal came after a blob was written"
            );
        }
    }

    /// The working-tree mode refuses before the file is written, too — `Writer::commit` owns
    /// the write now, so the order is its to keep.
    #[test]
    fn a_working_tree_writer_refuses_before_touching_the_file() {
        let dir = repo();
        let root = dir.path();
        let before = std::fs::read_to_string(root.join(ENTRY)).unwrap();
        let err = Writer::WorkingTree
            .commit(root, WHO, "establish: a new concept", "", &edited())
            .unwrap_err();
        assert!(err.to_string().contains("not an operational subject line"));
        assert_eq!(std::fs::read_to_string(root.join(ENTRY)).unwrap(), before);
    }

    /// A pod's commit: built from the pinned parent, chained, and nowhere — no ref moved, no
    /// working tree or index touched. The tool is the author, whoever the clone says is the
    /// committer, which on a cluster is the pod (`commits_as_the_pod`).
    #[test]
    fn a_detached_writer_chains_commits_and_moves_no_ref() {
        let dir = repo();
        let root = dir.path();
        let head = git(root, &["rev-parse", "HEAD"]).unwrap();
        let refs = git(root, &["for-each-ref"]).unwrap();
        let mut w = Writer::Detached(Detached::new(root, &head).unwrap());
        let first = w
            .commit(root, WHO, "refresh: a from its source", "", &edited())
            .unwrap()
            .expect("an edit was made");
        let second = w
            .commit(
                root,
                WHO,
                "reconcile: a used-by",
                "",
                &[(
                    ENTRY.to_string(),
                    "---\nname: a\nused-by: []\n---\n\n# A\n".to_string(),
                )],
            )
            .unwrap()
            .expect("a second edit was made");
        let Writer::Detached(done) = &w else {
            unreachable!()
        };
        let tip = done.tip().to_string();

        assert_eq!(
            git(root, &["rev-parse", "HEAD"]).unwrap(),
            head,
            "HEAD did not move"
        );
        assert_eq!(
            git(root, &["for-each-ref"]).unwrap(),
            refs,
            "no ref was written"
        );
        assert!(
            git(root, &["status", "--porcelain"]).unwrap().is_empty(),
            "tree and index clean"
        );

        assert_eq!(
            git(root, &["rev-parse", &format!("{tip}^^")]).unwrap(),
            head
        );
        assert!(tip.starts_with(&second.sha));
        assert!(git(root, &["rev-parse", &format!("{tip}^")])
            .unwrap()
            .starts_with(&first.sha));
        assert_eq!(
            git(root, &["show", &format!("{tip}:{ENTRY}")]).unwrap(),
            "---\nname: a\nused-by: []\n---\n\n# A"
        );
        assert_eq!(done.written().len(), 1, "one path, last write winning");
        let log = git(root, &["log", "-1", "--format=%an <%ae>%n%ce", &tip]).unwrap();
        assert_eq!(
            log,
            format!(
                "yidam test <test@yidam>\n{}",
                crate::git::fixture::FIXTURE_EMAIL
            )
        );
    }

    /// The `Option<Commit>` contract holds detached: the same bytes are no commit.
    #[test]
    fn a_detached_writer_writes_nothing_for_unchanged_bytes() {
        let dir = repo();
        let root = dir.path();
        let head = git(root, &["rev-parse", "HEAD"]).unwrap();
        let same = std::fs::read_to_string(root.join(ENTRY)).unwrap();
        let mut w = Writer::Detached(Detached::new(root, &head).unwrap());
        assert!(w
            .commit(
                root,
                WHO,
                "refresh: nothing moved",
                "",
                &[(ENTRY.to_string(), same)]
            )
            .unwrap()
            .is_none());
        let Writer::Detached(d) = &w else {
            unreachable!()
        };
        assert!(!d.moved());
        assert_eq!(d.tip(), head);
    }

    /// The invariant, tested where it is enforced rather than only where it is argued.
    #[test]
    fn an_epistemic_subject_is_refused() {
        let dir = repo();
        for subject in [
            "establish: a new concept",
            "resolve: the tension",
            "question: what is this",
            "no verb at all",
        ] {
            let err = author(dir.path(), WHO, subject, "", &[ENTRY.to_string()]).unwrap_err();
            assert!(
                err.to_string().contains("not an operational subject line"),
                "{subject} should be refused: {err}"
            );
        }
    }

    #[test]
    fn an_operational_subject_writes_a_commit_authored_by_the_tool() {
        let dir = repo();
        let root = dir.path();
        std::fs::write(
            root.join(ENTRY),
            "---\nname: a\nobtained: true\n---\n\n# A\n",
        )
        .unwrap();

        let c = author(
            root,
            WHO,
            "refresh: a from its source",
            "",
            &[ENTRY.to_string()],
        )
        .unwrap()
        .expect("an edit was made, so a commit is written");
        assert_eq!(c.subject, "refresh: a from its source");

        let log = git(root, &["log", "-1", "--format=%an <%ae>%n%ce%n%s"]).unwrap();
        let mut lines = log.lines();
        assert_eq!(lines.next().unwrap(), "yidam test <test@yidam>");
        assert_eq!(
            lines.next().unwrap(),
            crate::git::fixture::FIXTURE_EMAIL,
            "the committer is the person"
        );
        assert_eq!(lines.next().unwrap(), "refresh: a from its source");
    }

    /// What makes the command safe to put on a clock: nothing changed, so nothing is
    /// committed. An empty commit asserting a fetch happened is the failure RFC-0026 opens
    /// with — provenance invented rather than recorded.
    #[test]
    fn an_unchanged_entry_produces_no_commit() {
        let dir = repo();
        let before = git(dir.path(), &["rev-parse", "HEAD"]).unwrap();
        assert!(author(
            dir.path(),
            WHO,
            "refresh: nothing moved",
            "",
            &[ENTRY.to_string()]
        )
        .unwrap()
        .is_none());
        assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]).unwrap(), before);
    }

    /// A person's staged work in another file must survive this command untouched — the
    /// property the pathspec commit is chosen for.
    #[test]
    fn work_staged_elsewhere_is_left_staged_and_uncommitted() {
        let dir = repo();
        let root = dir.path();
        std::fs::write(root.join("notes.txt"), "half-finished\n").unwrap();
        crate::git::fixture::git(root, &["add", "notes.txt"]);

        std::fs::write(
            root.join(ENTRY),
            "---\nname: a\nobtained: true\n---\n\n# A\n",
        )
        .unwrap();
        author(
            root,
            WHO,
            "refresh: a from its source",
            "",
            &[ENTRY.to_string()],
        )
        .unwrap()
        .unwrap();

        let committed = git(root, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
        assert_eq!(committed.trim(), ENTRY, "only the entry is in the commit");
        let staged = git(root, &["diff", "--cached", "--name-only"]).unwrap();
        assert_eq!(
            staged.trim(),
            "notes.txt",
            "the person's work is still staged"
        );
    }

    #[test]
    fn an_uncommitted_entry_is_refused_before_anything_is_written() {
        let dir = repo();
        let root = dir.path();
        std::fs::write(
            root.join(ENTRY),
            "---\nname: a\nedited: by hand\n---\n\n# A\n",
        )
        .unwrap();
        let err = require_clean(root, &[ENTRY.to_string()]).unwrap_err();
        assert!(err.to_string().contains("uncommitted changes"), "{err}");
    }

    #[test]
    fn a_clean_entry_passes_the_guard() {
        let dir = repo();
        require_clean(dir.path(), &[ENTRY.to_string()]).unwrap();
    }

    #[test]
    fn a_body_is_carried_under_the_subject() {
        let dir = repo();
        let root = dir.path();
        std::fs::write(
            root.join(ENTRY),
            "---\nname: a\nobtained: true\n---\n\n# A\n",
        )
        .unwrap();
        author(
            root,
            WHO,
            "refresh: a",
            "sha256:abc from location 0\n",
            &[ENTRY.to_string()],
        )
        .unwrap()
        .unwrap();
        let body = git(root, &["log", "-1", "--format=%b"]).unwrap();
        assert!(body.contains("sha256:abc from location 0"), "{body}");
    }
}
