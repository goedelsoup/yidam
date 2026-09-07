//! Authoring the operational commit a connector run produces.
//!
//! # What licenses this to write a commit at all
//!
//! RFC-0026 states the invariant the orchestrator layer is built on, and it is the whole of
//! the permission argument here:
//!
//! > A run authors **operational** commits directly. Every **epistemic** commit it produces
//! > goes to a proposal branch, and nothing merges itself.
//!
//! `refresh:` and `reconcile:` are both in `OPERATIONAL_VERBS`, so both land on the current
//! branch. That is not an assertion this module makes about itself — [`author`] classifies
//! its own subject line with `classify_commit`, the parity function with fixtures in three
//! SDKs, and refuses to write anything the vocabulary calls epistemic. The check is cheap,
//! and it is what stops a later patch from adding an `establish:` path here without going
//! red.
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

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use yidam_core::git::{classify_commit, CommitKind};

/// The author every commit written here carries.
///
/// Author and committer are separated for `propose`'s reason, which applies unchanged: the
/// tool performed the fetch, a person ran it, and recording both is a true account without
/// borrowing an identity. It matters more here than there — an operational commit lands on
/// the branch rather than on a proposal nobody has merged yet, so the record of what wrote it
/// is the only thing distinguishing it from a person's own work.
const AUTHOR_NAME: &str = "yidam catalog";
const AUTHOR_EMAIL: &str = "catalog@yidam";

/// A commit this module wrote.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("running git")?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Refuse when any of these paths differs from `HEAD`.
///
/// Checked *before* the edit, so the refusal names work a person has not committed rather
/// than work this command just did. Narrowed to the paths being written for the reason given
/// in the module header: `require_committed_corpus` gates the whole of `.yidam/` because a
/// proposal is drafted from a corpus-wide read, and a fetch is not.
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
        "these entries have uncommitted changes, and a fetch records what a committed entry \
         declares:\n{}\n\n  Commit or stash them first — otherwise the `refresh:` commit \
         would carry edits nobody reviewed alongside the digest it went to record.",
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
fn is_operational(subject: &str) -> bool {
    classify_commit("", subject).kind == CommitKind::Operational
}

/// Write one commit containing exactly `paths`, and return it.
///
/// `None` when the paths hold nothing to commit — a re-run that found the same bytes edits
/// nothing, and an empty commit asserting a fetch happened would be the *"provenance invented
/// rather than recorded"* failure RFC-0026 opens with.
pub fn author(root: &Path, subject: &str, body: &str, paths: &[String]) -> Result<Option<Commit>> {
    if !is_operational(subject) {
        bail!(
            "`{subject}` is not an operational subject line, and a run may not author one.\n  \
             RFC-0026: a run authors operational commits directly; every epistemic commit it \
             produces goes to a proposal branch, and nothing merges itself.\n  \
             The operational verbs are: {}",
            yidam_core::git::OPERATIONAL_VERBS.join(", ")
        );
    }
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

    let message = if body.trim().is_empty() {
        subject.to_string()
    } else {
        format!("{subject}\n\n{body}")
    };
    // `commit -- <paths>` builds the commit from HEAD plus these paths only, leaving anything
    // else in the index staged and uncommitted. That is the property that makes this safe to
    // run in a repository somebody is working in.
    let mut args = vec!["commit", "--no-verify", "-m", &message, "--"];
    args.extend(paths.iter().map(String::as_str));
    let out = Command::new("git")
        .current_dir(root)
        .args(&args)
        .env("GIT_AUTHOR_NAME", AUTHOR_NAME)
        .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
        .stdin(Stdio::null())
        .output()
        .context("running git commit")?;
    if !out.status.success() {
        bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
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
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@t"],
            vec!["config", "user.name", "T"],
        ] {
            Command::new("git")
                .current_dir(root)
                .args(&args)
                .output()
                .unwrap();
        }
        std::fs::create_dir_all(root.join(".yidam/catalog")).unwrap();
        std::fs::write(
            root.join(".yidam/catalog/a.md"),
            "---\nname: a\n---\n\n# A\n",
        )
        .unwrap();
        Command::new("git")
            .current_dir(root)
            .args(["add", "-A"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(root)
            .args(["commit", "-qm", "scaffold: a catalog"])
            .output()
            .unwrap();
        dir
    }

    const ENTRY: &str = ".yidam/catalog/a.md";

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
            let err = author(dir.path(), subject, "", &[ENTRY.to_string()]).unwrap_err();
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

        let c = author(root, "refresh: a from its source", "", &[ENTRY.to_string()])
            .unwrap()
            .expect("an edit was made, so a commit is written");
        assert_eq!(c.subject, "refresh: a from its source");

        let log = git(root, &["log", "-1", "--format=%an <%ae>%n%ce%n%s"]).unwrap();
        let mut lines = log.lines();
        assert_eq!(lines.next().unwrap(), "yidam catalog <catalog@yidam>");
        assert_eq!(lines.next().unwrap(), "t@t", "the committer is the person");
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
        Command::new("git")
            .current_dir(root)
            .args(["add", "notes.txt"])
            .output()
            .unwrap();

        std::fs::write(
            root.join(ENTRY),
            "---\nname: a\nobtained: true\n---\n\n# A\n",
        )
        .unwrap();
        author(root, "refresh: a from its source", "", &[ENTRY.to_string()])
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
