//! `cluster land` — the one command that writes a ref, and what it checks before it does.
//!
//! The lander holds the write credential, so everything the invariant means on a cluster is
//! decided here, and the decisions are made from the commit rather than from the pod that
//! built it:
//!
//! - **Class is read off the commit's subject**, with `classify_commit`, the same parity
//!   function every other reader of the vocabulary uses. The step's record carries a `class`
//!   too, and it is compared and refused on mismatch, never used. A step that lied about its
//!   class would have to also forge its own subject line, and the subject line is what
//!   decides — so a lie in the record buys nothing.
//! - **The ref follows the class.** Operational lands on the branch; epistemic lands on
//!   `propose/<input>` and never on the branch. This is `Capability::route` restated over a
//!   commit rather than a declaration, and it is the whole of the manifest's say in where a
//!   commit goes: there is no field that moves an epistemic commit to the branch, because
//!   the lander does not read one.
//! - **The write is a compare-and-swap.** `--force-with-lease=<ref>:<expected>` is git's
//!   spelling of *only if nobody else moved it*, and an empty expected value is *only if it
//!   does not exist* — the two cases `run` handles with `update-ref <ref> <new> <old>` and
//!   the all-zero sha.
//!
//! # Retry on conflict is a re-parent, and it is refused when it should be
//!
//! Two runs can be in flight on one corpus, or a person can push while a step is computing.
//! Then the branch has moved since the pin, the lease fails, and the commit's parent is no
//! longer the tip. The commit is rebuilt on the new tip — same tree changes, same message,
//! same author — **only if the move did not touch what the step reads.** If it did, the
//! step's receipt would record an input state the commit's parent does not have, which is a
//! receipt that lies. So the lander lands nothing, says so, and the next admission finds the
//! step stale and runs it against the new tip. A step's result is a function of its inputs;
//! moving it under inputs that changed is not a retry, it is a different run.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{bail, Context, Result};

use super::{
    bundle, clone_branch, deliver, pin, read_step_output, Landed, RemoteArgs, StepOutput,
    VaultArgs, CONTRACT_VERSION,
};
use crate::cmd::propose::write::{branch_for, commit_tree, git, short_of, TempIndex};
use crate::cmd::run::exec::Scratch;
use crate::cmd::run::manifest::{Capability, Manifest, MANIFEST};
use crate::cmd::run::{AUTHOR_EMAIL, AUTHOR_NAME};
use crate::git::Git;
use crate::report::Format;
use crate::vault::Store;

/// The ref a step's bundle is fetched into, in the lander's scratch clone.
const INCOMING: &str = "refs/yidam/incoming";

/// How many times the lease is retried before the lander gives up on a busy ref.
///
/// Each retry is a fetch and a re-parent, and a ref that moves faster than that for five
/// rounds is not a race, it is a corpus being pushed to continuously; landing into that
/// blind is not what this is for.
const ATTEMPTS: u32 = 5;

pub(super) fn run(
    step_output: &str,
    remote: &RemoteArgs,
    vault: &VaultArgs,
    out: Option<&Path>,
    format: Format,
) -> Result<()> {
    let claim = read_step_output(step_output)?;
    let store = vault.open()?;
    let scratch = Scratch::new("land")?;
    let root = scratch.path().join("corpus");
    clone_branch(&remote.remote, &remote.branch, &root)?;
    super::commits_as_the_pod(&root)?;
    let record = land_in(&root, scratch.path(), &claim, remote, store.as_ref())?;
    deliver(&root, format, out, record, render)
}

/// Land `claim`'s commit from the clone at `root`, whose `origin` is the remote.
pub(super) fn land_in(
    root: &Path,
    scratch: &Path,
    claim: &StepOutput,
    remote: &RemoteArgs,
    store: &dyn Store,
) -> Result<Landed> {
    let branch = &remote.branch;
    let (Some(sha), Some(digest)) = (&claim.sha, &claim.bundle) else {
        // Fresh or unchanged: nothing to land, and the next step reads the branch as it is.
        let next = pin::pin_of(root, branch, store, scratch)?;
        return Ok(Landed {
            format_version: CONTRACT_VERSION,
            step: claim.step.clone(),
            landed: None,
            target: None,
            class: claim.class.clone(),
            reparented: false,
            attempts: 0,
            next,
        });
    };

    // ── the commit, as the bundle carries it ──────────────────────────────────
    let file = scratch.join("in.bundle");
    bundle::fetch(store, digest, &file)?;
    Git::new(root)
        .args(["bundle", "verify", "-q"])
        .arg(&file)
        .run()
        .with_context(|| {
            format!(
                "the bundle for `{}` does not verify against this clone",
                claim.step
            )
        })?;
    Git::new(root)
        .args(["fetch", "-q"])
        .arg(&file)
        .arg(format!("{}:{INCOMING}", super::OUT_REF))
        .run()?;
    let incoming = git(root, None, &["rev-parse", INCOMING], None)?;
    if incoming != *sha {
        bail!("the bundle carries {incoming} and the step record claims {sha}; nothing was landed");
    }

    // ── what the commit says about itself, which is what decides ──────────────
    let subject = git(root, None, &["log", "-1", "--format=%s", sha], None)?;
    let event = yidam_core::git::classify_commit(sha, &subject);
    let class = format!("{:?}", event.kind).to_lowercase();
    if class != claim.class {
        bail!(
            "the step record says `{}` and the commit says `{class}` — its subject is \
             `{subject}`.\n  \
             A claim in a pod's output is not a permission either; the class is read off the \
             commit, and nothing was landed.",
            claim.class
        );
    }
    let parent = git(root, None, &["rev-parse", &format!("{sha}^")], None)?;
    if parent != claim.input {
        bail!(
            "the commit's parent is {parent} and the step record says it ran against {}; \
             nothing was landed",
            claim.input
        );
    }
    if !Git::new(root)
        .args(["cat-file", "-e"])
        .rev(format!("{sha}:{}", claim.receipt))
        .succeeded()
    {
        bail!(
            "the commit carries no receipt at {}; a commit without one is not a run's, and \
             nothing was landed",
            claim.receipt
        );
    }

    // The manifest at the tip, for what the step reads — the re-parent check needs it.
    let m = Manifest::load(root)?;
    let cap = m.get(&claim.step)?;

    let input_short = short_of(root, &claim.input);
    let target = match class.as_str() {
        "operational" => branch.clone(),
        _ => branch_for(&input_short),
    };

    // ── the compare-and-swap, retried by re-parenting ─────────────────────────
    let mut candidate = sha.clone();
    let mut reparented = false;
    let mut attempts = 0;
    let landed = loop {
        attempts += 1;
        let tip = remote_tip(root, &target)?;
        let wanted_parent = tip.clone().unwrap_or_else(|| claim.input.clone());
        let candidate_parent = git(root, None, &["rev-parse", &format!("{candidate}^")], None)?;
        if candidate_parent != wanted_parent {
            match reparent(root, &candidate, &wanted_parent, cap, &claim.step)? {
                Some(moved) => candidate = moved,
                // The tip already holds this result: an earlier attempt landed it and this
                // record was handed to the lander twice, or the same proposal was made on two
                // pins. Stacking an identical commit would say something happened. Nothing did.
                None => break None,
            }
            reparented = true;
        }
        let out = Git::new(root)
            .args(["push", "-q", "origin"])
            .arg(format!("{candidate}:refs/heads/{target}"))
            .arg(format!(
                "--force-with-lease=refs/heads/{target}:{}",
                tip.as_deref().unwrap_or("")
            ))
            .output()?;
        if out.status.success() {
            break Some(candidate.clone());
        }
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        // `(stale info)` is the lease failing: the ref moved between the read and the write.
        // Anything else is the remote's answer, in the remote's words — a permission refused,
        // a hook, a missing repository — and the lander does not paraphrase it.
        if attempts < ATTEMPTS && stderr.contains("stale info") {
            continue;
        }
        bail!(
            "the lander could not write refs/heads/{target} on {}:\n{stderr}",
            remote.remote
        );
    };

    // ── the branch after landing, as the next step reads it ───────────────────
    // A proposal leaves the branch where it was, but "where it was" is where the remote says
    // it is, not where this clone was taken; refresh before pinning.
    Git::new(root)
        .args(["fetch", "-q", "origin"])
        .arg(format!("+refs/heads/{branch}:refs/remotes/origin/{branch}"))
        .run()?;
    git(
        root,
        None,
        &[
            "update-ref",
            &format!("refs/heads/{branch}"),
            &format!("refs/remotes/origin/{branch}"),
        ],
        None,
    )?;
    let next = pin::pin_of(root, branch, store, scratch)?;

    Ok(Landed {
        format_version: CONTRACT_VERSION,
        step: claim.step.clone(),
        landed,
        target: Some(target),
        class,
        reparented,
        attempts,
        next,
    })
}

/// The remote's tip of `refs/heads/<name>`, fetched so its objects are here, or `None`.
fn remote_tip(root: &Path, name: &str) -> Result<Option<String>> {
    let out = Git::new(root)
        .args(["ls-remote", "--exit-code", "origin"])
        .arg(format!("refs/heads/{name}"))
        .output()?;
    // `--exit-code` is 2 for "no such ref", which is an answer; anything else is a failure.
    if out.status.code() == Some(2) {
        return Ok(None);
    }
    if !out.status.success() {
        bail!(
            "asking origin for refs/heads/{name}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let Some(sha) = stdout.split_whitespace().next() else {
        return Ok(None);
    };
    Git::new(root)
        .args(["fetch", "-q", "origin"])
        .arg(format!("+refs/heads/{name}:refs/remotes/origin/{name}"))
        .run()?;
    Ok(Some(sha.to_string()))
}

/// Rebuild `sha`'s change on `new_parent`, or refuse if the move touched what the step reads.
///
/// `None` when the rebuilt tree is `new_parent`'s own tree: the change is already there, and
/// a commit that changes nothing is not a landing.
///
/// Not a merge and not a rebase: the commit's changed paths are applied as blobs onto the new
/// parent's tree, with the same message and the same author, which is what the step would
/// have produced had it been pinned there — **provided its inputs are the same there**. That
/// proviso is checked first, over the declaration's `reads` plus the manifest and config,
/// which are the three things the receipt's input state is a digest of.
pub(super) fn reparent(
    root: &Path,
    sha: &str,
    new_parent: &str,
    cap: &Capability,
    step: &str,
) -> Result<Option<String>> {
    let old_parent = git(root, None, &["rev-parse", &format!("{sha}^")], None)?;
    if read_set(root, &old_parent, cap)? != read_set(root, new_parent, cap)? {
        bail!(
            "refs moved under `{step}`: {} → {} touched what it reads, so its result does not \
             hold there.\n  \
             Nothing was landed. The next admission sees the step stale at the new tip and \
             runs it there.",
            short_of(root, &old_parent),
            short_of(root, new_parent)
        );
    }

    let changed = git(
        root,
        None,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-status",
            "-r",
            &old_parent,
            sha,
        ],
        None,
    )?;
    let scratch = TempIndex::new(root, "land")?;
    let index = scratch.path().to_path_buf();
    git(root, Some(&index), &["read-tree", new_parent], None)?;
    for line in changed.lines() {
        let Some((status, path)) = line.split_once('\t') else {
            continue;
        };
        if status.starts_with('D') {
            git(
                root,
                Some(&index),
                &["update-index", "--force-remove", path],
                None,
            )?;
            continue;
        }
        let entry = git(root, None, &["ls-tree", sha, "--", path], None)?;
        let Some((meta, _)) = entry.split_once('\t') else {
            bail!("`{path}` is in {sha}'s diff and not in its tree");
        };
        let mut parts = meta.split(' ');
        let (Some(mode), Some(_), Some(blob)) = (parts.next(), parts.next(), parts.next()) else {
            bail!("unreadable ls-tree entry for `{path}`: {entry}");
        };
        git(
            root,
            Some(&index),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("{mode},{blob},{path}"),
            ],
            None,
        )?;
    }
    let tree = git(root, Some(&index), &["write-tree"], None)?;
    if tree
        == git(
            root,
            None,
            &["rev-parse", &format!("{new_parent}^{{tree}}")],
            None,
        )?
    {
        return Ok(None);
    }
    let message = git(root, None, &["log", "-1", "--format=%B", sha], None)?;
    commit_tree(
        root,
        &tree,
        new_parent,
        &message,
        (AUTHOR_NAME, AUTHOR_EMAIL),
    )
    .map(Some)
}

/// Every `(path, blob)` at `commit` that the step's input state is computed over.
fn read_set(root: &Path, commit: &str, cap: &Capability) -> Result<BTreeSet<(String, String)>> {
    let listing = git(root, None, &["ls-tree", "-r", "--full-tree", commit], None)?;
    Ok(listing
        .lines()
        .filter_map(|l| {
            let (meta, path) = l.split_once('\t')?;
            let blob = meta.split(' ').nth(2)?;
            let read = path == MANIFEST
                || path == ".yidam/config.toml"
                || cap.reads.iter().any(|g| crate::kuten::glob_covers(g, path));
            read.then(|| (path.to_string(), blob.to_string()))
        })
        .collect())
}

pub(super) fn render(l: &Landed) -> String {
    let mut s = match (&l.landed, &l.target) {
        (Some(sha), Some(target)) => format!(
            "{}: landed {} ({}) on {target}{}, {} attempt{}\n",
            l.step,
            &sha[..sha.len().min(12)],
            l.class,
            if l.reparented { ", re-parented" } else { "" },
            l.attempts,
            if l.attempts == 1 { "" } else { "s" }
        ),
        (None, Some(target)) => format!(
            "{}: {target} already holds this result; nothing landed\n",
            l.step
        ),
        _ => format!("{}: nothing to land\n", l.step),
    };
    s.push_str(&pin::render(&l.next).replace("pinned", "  next: pinned"));
    s
}
