//! `yidam catalog-reconcile` — retired; it writes nothing (RFC-0053 §2.4).
//!
//! It rewrote a catalog entry's hand-maintained `used-by` list to match the citations, one
//! `reconcile:` commit per entry whenever a node was added. An entry no longer stores what
//! cites it: `yidam catalog-audit` reports that from the citations, and
//! `yidam migrate used-by` deletes the stored lists. With no list there is nothing to repair.
//!
//! The command stays for one release because derived guards call it. It writes nothing, with or
//! without `--dry-run`, and exits 0. Its report keeps its shape with `reconciled` always empty,
//! so a guard that fails when `reconciled` names an entry keeps passing, migrated or not. When
//! an entry still declares a list it says so on stderr, naming the migration, and changes
//! neither the exit code nor the report. The release after removes it.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::parse::parse_frontmatter;
use crate::paths::{repo_root, yidam_catalog_dir};
use crate::walk::walk_md_files;

pub struct ReconcileOptions {
    /// Restrict to one entry, by file stem or `name:`.
    pub entry: Option<String>,
    /// Accepted for the guards that pass it. Nothing is written either way.
    pub dry_run: bool,
    pub format: crate::report::Format,
}

/// `reconciled`, not `entries` — see [`super::fetch`]'s note on the same rename.
///
/// Always empty: a row was an entry this rewrote, and it rewrites none. Kept so a guard
/// reading it keeps passing for the release this command survives.
#[derive(serde::Serialize)]
struct ReconcileReport {
    reconciled: Vec<String>,
}

pub fn reconcile(opts: &ReconcileOptions) -> Result<()> {
    let root = repo_root()?;
    let declaring = declaring(&root, opts.entry.as_deref())?;
    if !declaring.is_empty() {
        eprintln!(
            "note: {} catalog entr(ies) still declare `used-by:`, which nothing reads. \
             `yidam migrate used-by` deletes them; `yidam catalog-audit` reports what cites \
             each entry from the citations.",
            declaring.len()
        );
    }
    crate::report::finish(
        &root,
        opts.format,
        ReconcileReport { reconciled: vec![] },
        |_| {
            println!(
                "Nothing to reconcile: a catalog entry does not store what cites it. This \
                 command writes nothing, and the next release removes it."
            )
        },
    )
}

/// The entries still declaring a `used-by:` key, restricted to `entry` when one is named.
///
/// An `entry` naming nothing is refused, as it was when this command rewrote lists: a typo
/// in a guard should not read as a clean catalog.
fn declaring(root: &Path, entry: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut matched = false;
    let mut out = Vec::new();
    for path in walk_md_files(&yidam_catalog_dir(root)) {
        if path.file_name().is_some_and(|n| n == "README.md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let fm = parse_frontmatter(&text);
        if let Some(want) = entry {
            let want = want.trim().trim_end_matches(".md");
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            if stem != want && fm.name.as_deref() != Some(want) {
                continue;
            }
        }
        matched = true;
        if fm.used_by.is_some() {
            out.push(path);
        }
    }
    if let (Some(want), false) = (entry, matched) {
        anyhow::bail!(
            "no catalog entry named `{want}`. `yidam catalog-audit` lists what this corpus holds."
        );
    }
    Ok(out)
}
