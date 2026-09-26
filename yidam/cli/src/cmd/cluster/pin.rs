//! `cluster pin` — the branch tip, as a bundle in the vault.
//!
//! The first task of a workflow, and the only place a run reads the remote before anything
//! is computed. Every step downstream reads a pin, never the remote, which is what makes a
//! run a function of one commit rather than of whatever the branch said when each pod
//! happened to start.

use std::path::Path;

use anyhow::Result;

use super::{clone_branch, deliver, Pinned, RemoteArgs, VaultArgs, CONTRACT_VERSION};
use crate::cmd::run::exec::Scratch;
use crate::report::Format;
use crate::vault::Store;

pub(super) fn run(
    remote: &RemoteArgs,
    vault: &VaultArgs,
    out: Option<&Path>,
    format: Format,
) -> Result<()> {
    let store = vault.open()?;
    let scratch = Scratch::new("pin")?;
    let root = scratch.path().join("corpus");
    clone_branch(&remote.remote, &remote.branch, &root)?;
    let pinned = pin_of(&root, &remote.branch, store.as_ref(), scratch.path())?;
    deliver(&root, format, out, pinned, render)
}

/// Bundle `refs/heads/<branch>` of the repository at `root` and put it in the store.
///
/// Shared with the lander, whose last act is to pin the branch as it stands after landing,
/// so that the next step reads what this one wrote.
pub(super) fn pin_of(
    root: &Path,
    branch: &str,
    store: &dyn Store,
    scratch: &Path,
) -> Result<Pinned> {
    let sha = crate::git::Git::new(root)
        .args(["rev-parse", "--verify"])
        .rev(format!("refs/heads/{branch}"))
        .run()?;
    let file = scratch.join("pin.bundle");
    super::bundle::create(root, &file, &[&format!("refs/heads/{branch}")])?;
    let bundle = super::bundle::put(store, &file)?;
    Ok(Pinned {
        format_version: CONTRACT_VERSION,
        branch: branch.to_string(),
        sha,
        bundle,
    })
}

pub(super) fn render(p: &Pinned) -> String {
    format!(
        "pinned {} at {}\n  bundle {}\n",
        p.branch,
        &p.sha[..p.sha.len().min(12)],
        p.bundle
    )
}
