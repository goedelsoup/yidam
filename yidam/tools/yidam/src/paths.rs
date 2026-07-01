use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn repo_root() -> Result<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("running git rev-parse")?;
    if out.status.success() {
        let s = String::from_utf8(out.stdout).context("git output utf8")?;
        Ok(PathBuf::from(s.trim()))
    } else {
        std::env::current_dir().context("current dir fallback")
    }
}

pub fn yidam_corpus_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("corpus")
}

pub fn yidam_catalog_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("catalog")
}

pub fn yidam_skills_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("skills")
}

pub fn yidam_decisions_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("decisions")
}
