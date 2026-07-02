use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::paths::repo_root;

use super::copy::copy_dir;

pub fn clone(target: &Path) -> Result<()> {
    if target.exists() {
        bail!("target already exists: {}", target.display());
    }

    let root = repo_root()?;

    println!("Copying template → {} …", target.display());
    copy_dir(&root, target)?;

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
