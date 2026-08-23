use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::paths::repo_root;
use crate::provenance::Provenance;

use super::copy::copy_dir_excluding_top;

pub fn clone(target: &Path) -> Result<()> {
    if target.exists() {
        bail!("target already exists: {}", target.display());
    }

    let root = repo_root()?;

    // Read before copying: `copy_dir` excludes `.git`, so once the copy is the only
    // thing left there is nothing to read the origin commit from.
    let provenance = Provenance::read(&root);

    println!("Copying template → {} …", target.display());
    // Leave yidam's own docs/ (the RFC set and docs-site source) behind — it documents
    // yidam, not the repo being created. `sadhana/docs/` is a different directory at a
    // different depth and must ship: step 3 of the bootstrap skill reads it.
    //
    // `examples/` likewise. It holds a whole worked corpus — its own `.yidam/corpus`,
    // catalog, decisions and skills — and copying it would deposit a foreign domain's
    // nodes into a repository at the moment of its creation, before its own ontology
    // exists. A newcomer reads the example in yidam; they do not inherit it.
    copy_dir_excluding_top(&root, target, &["docs", "examples"])?;

    provenance.write(target)?;
    println!(
        "Pinned to yidam {} ({})",
        &provenance.commit[..provenance.commit.len().min(8)],
        provenance.template
    );

    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(target)
        .status()
        .context("running git init")?;
    if !init.success() {
        bail!("git init failed in {}", target.display());
    }

    // mise trust is best-effort — not all environments have mise
    let _ = std::process::Command::new("mise")
        .args(["trust", "-q"])
        .current_dir(target)
        .status();

    println!("yidam template copied to {}", target.display());
    Ok(())
}
