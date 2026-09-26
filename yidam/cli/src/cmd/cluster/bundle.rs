//! Git bundles as the thing that crosses between pods, by content digest.
//!
//! Every helper here is a wrapper over one `git bundle` verb or one [`Store`] call, and the
//! reason they are wrapped at all is to fix the one convention the pods share: a bundle in
//! the vault is named by the digest of its bytes, and nothing else — no branch name, no
//! run id, nothing a second run could collide with. Two pins of the same tip put one file.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::git::Git;
use crate::vault::{ContentHash, Store};

/// `git bundle create <file> <what...>`, standing in `root`.
///
/// `what` is a rev-list argument: a ref name for a whole history, `<base>..<ref>` for one
/// with a prerequisite. A bare sha is refused by git — a bundle needs a ref to name what it
/// carries — which is why the step pod points `refs/yidam/out` at its commit first.
pub(super) fn create(root: &Path, file: &Path, what: &[&str]) -> Result<()> {
    Git::new(root)
        .args(["bundle", "create"])
        .arg(file)
        .args(what)
        .run()
        .with_context(|| format!("bundling {}", what.join(" ")))?;
    Ok(())
}

/// Put a bundle in the store and return its digest, which is the only name it has.
pub(super) fn put(store: &dyn Store, file: &Path) -> Result<String> {
    let hash = ContentHash::of_file(file)?;
    store
        .put(&hash, file)
        .with_context(|| format!("putting the bundle in {}", store.describe()))?;
    Ok(hash.as_str().to_string())
}

/// Fetch a bundle by digest to `dest`.
pub(super) fn fetch(store: &dyn Store, digest: &str, dest: &Path) -> Result<()> {
    let hash =
        ContentHash::parse(digest).with_context(|| format!("`{digest}` is not a vault digest"))?;
    store
        .get(&hash, dest)
        .with_context(|| format!("fetching bundle {digest} from {}", store.describe()))
}

/// The refs a bundle carries, as `(sha, refname)`.
pub(super) fn heads(cwd: &Path, file: &Path) -> Result<Vec<(String, String)>> {
    let out = Git::new(cwd)
        .args(["bundle", "list-heads"])
        .arg(file)
        .run()
        .with_context(|| format!("listing the heads of {}", file.display()))?;
    Ok(out
        .lines()
        .filter_map(|l| {
            let (sha, name) = l.split_once(' ')?;
            Some((sha.to_string(), name.to_string()))
        })
        .collect())
}

/// The one branch a pinned bundle carries, as `(sha, branch)`.
pub(super) fn pinned_branch(cwd: &Path, file: &Path) -> Result<(String, String)> {
    let heads = heads(cwd, file)?;
    let mut branches = heads.iter().filter_map(|(sha, name)| {
        Some((sha.clone(), name.strip_prefix("refs/heads/")?.to_string()))
    });
    match (branches.next(), branches.next()) {
        (Some(one), None) => Ok(one),
        (None, _) => bail!(
            "the bundle carries no branch — its refs are {}",
            heads
                .iter()
                .map(|(_, n)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        (Some(a), Some(b)) => bail!(
            "the bundle carries more than one branch ({}, {}), and a pin carries exactly one",
            a.1,
            b.1
        ),
    }
}

/// Clone one branch out of a bundle into `dest`, full history.
pub(super) fn clone(file: &Path, branch: &str, dest: &Path) -> Result<()> {
    let parent = dest.parent().unwrap_or(dest);
    Git::new(parent)
        .args(["clone", "-q", "--branch", branch])
        .arg(file)
        .arg(dest)
        .run()
        .with_context(|| format!("cloning {branch} out of {}", file.display()))?;
    Ok(())
}
