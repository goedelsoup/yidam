//! The tracked set, and copying out of it.
//!
//! Two commands copy this repository into another one — `yidam clone` into a new directory,
//! `yidam overlay` into an existing repository — and both take their source from a checkout
//! of the template. **The tracked set is what the template is**, so both ask git the same
//! question here rather than walking a working tree and subtracting conventions.
//!
//! The walk is what this replaced, twice. `clone`'s was #912: a copy taken from a working
//! checkout was 2,931 files / 22.7 MB, of which 1,436 files and 14 MB was
//! `.claude/worktrees/` — the operator's own agent scratch worktrees, gitignored since the
//! day the entry was written, and absent from `git ls-files` all along. `overlay`'s was #984,
//! found the same way and one command over: six gitignored paths under `yidam/` alone that no
//! entry in its exclusion list reached, travelling into every repository `overlay` is run
//! against — which is the half of bootstrap where the target already holds someone's work.
//!
//! A list of what does *not* travel grows one entry per accident, and the entry is always
//! written after the accident. Being untracked is the whole of the rule instead: a file this
//! checkout does not track is not part of the template by construction, so there is no
//! convention left to name and no third instance to wait for.

use anyhow::{bail, Context, Result};
use std::path::Path;

/// Every path git tracks at `root`, repository-relative.
///
/// It refuses rather than falling back to a walk. A fallback would restore the whole hole in
/// exactly the circumstance nobody tests, and the source of either copy has to be a checkout
/// for a reason that predates this: [`crate::provenance::Provenance`] reads the commit being
/// pinned out of the same git directory.
pub(super) fn paths(root: &Path) -> Result<Vec<String>> {
    // Through [`crate::git::Git`], which is the only production code that spawns git
    // (#929). It owns `-C`, strips an inherited `GIT_DIR` that would otherwise redirect
    // this read past the directory named here, and pins the config — all three of which
    // this function would have had to re-decide.
    let out = crate::git::Git::new(root)
        .args(["ls-files", "-z", "--full-name"])
        .output()?;

    let paths: Vec<String> = if out.status.success() {
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        Vec::new()
    };

    if paths.is_empty() {
        bail!(
            "cannot read the template: git tracks nothing at {}\n  \
             the copy is what git tracks, because the tracked set is what the template \
             is — a working tree also holds build output, install prefixes and agent \
             worktrees no derived repository should inherit. Run this from a git checkout \
             of yidam with its history intact.",
            root.display()
        )
    }
    Ok(paths)
}

/// The tracked paths inside one subtree, which is how `overlay` names the three it copies.
///
/// Matched on a segment boundary, not as a string prefix. `dist` versus `dist-*` was #900 and
/// `target` versus `target-pin/` was the miss before it: a name that admits a suffix will be
/// handed one, and here the direction is inverted — a bare `starts_with` asked for `yidam`
/// would also claim a top-level `yidam-notes.md` that belongs to nobody's subtree.
pub(super) fn under(paths: &[String], prefix: &str) -> Vec<String> {
    let nested = format!("{prefix}/");
    paths
        .iter()
        .filter(|p| p.as_str() == prefix || p.starts_with(&nested))
        .cloned()
        .collect()
}

/// Copy each tracked path into `target`, skipping the ones whose first segment is excluded.
///
/// The exclusions are matched on the **first segment only**, which is the same rule the
/// filesystem walk applied at the first level and is load-bearing for the same reason:
/// `docs` is yidam's own and must not travel, `sadhana/docs/` is the scaffold step 3 of the
/// bootstrap skill reads and must. The two differ in depth, not in name.
///
/// Every path keeps its repository-relative position, which is what lets `overlay` pass a
/// subtree of the same list and land it at the same place in the target.
///
/// Returns how many files were written.
pub(super) fn copy_into(
    root: &Path,
    target: &Path,
    paths: &[String],
    excluded: &[&str],
) -> Result<usize> {
    std::fs::create_dir_all(target).with_context(|| format!("creating {}", target.display()))?;

    let mut copied = 0;
    for rel in paths {
        let top = rel.split('/').next().unwrap_or(rel);
        if excluded.contains(&top) {
            continue;
        }
        let src = root.join(rel);
        // `symlink_metadata`, not `is_file`: a tracked path can be a symlink — three under
        // `yidam/cli/` are — or a submodule's gitlink, and the walk this replaced skipped
        // both. It can also be absent, when the working tree has a deletion that is not
        // staged, and there is nothing to copy from a file that is not there.
        let Ok(meta) = src.symlink_metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let dst = target.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::copy(&src, &dst).with_context(|| format!("copying {rel}"))?;
        copied += 1;
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source git cannot describe is refused, not walked.
    ///
    /// The fallback is the tempting thing to write and it is the defect: a walk restores the
    /// whole hole in exactly the circumstance nobody tests, and the message a reader gets
    /// would be about the copy rather than about the source.
    #[test]
    fn a_source_with_no_git_is_refused_rather_than_walked() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("yidam/prelude")).unwrap();
        std::fs::create_dir_all(root.join("sadhana")).unwrap();
        std::fs::write(root.join("yidam/prelude/GRAPH.md"), "the graph").unwrap();

        let err = paths(root).unwrap_err().to_string();
        assert!(
            err.contains("git tracks nothing"),
            "a source git cannot describe must be refused, not walked: {err}"
        );
    }

    /// A subtree is named by whole segments. The listed paths are the shape the miss would
    /// take in this repository: `yidam/` is a subtree `overlay` copies, and a top-level file
    /// whose name merely begins with it is not part of it.
    #[test]
    fn a_subtree_is_matched_on_a_segment_boundary() {
        let all: Vec<String> = [
            "yidam/prelude/GRAPH.md",
            "yidam-notes.md",
            "sadhana/root/README.md",
            "LICENSE",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(under(&all, "yidam"), vec!["yidam/prelude/GRAPH.md"]);
        assert_eq!(under(&all, "sadhana"), vec!["sadhana/root/README.md"]);
        assert!(under(&all, "samudaya").is_empty());
    }
}
