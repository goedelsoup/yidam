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

/// The sangha's governance records: `PROTOCOL.md`, `electors.md`, `positions/`,
/// `resolutions/`. Absent in single-elector repositories, where collective mode is
/// opt-in — every caller must tolerate it not existing.
pub fn yidam_sangha_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("sangha")
}

pub fn yidam_embeddings_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("embeddings")
}

pub fn yidam_index_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("index")
}

pub fn samudaya_dir(root: &Path) -> PathBuf {
    root.join("samudaya")
}

#[cfg(feature = "tonpa")]
pub fn tonpa_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("tonpa")
}

#[cfg(feature = "tonpa")]
pub fn tonpa_config_path(root: &Path) -> PathBuf {
    root.join(".yidam").join("tonpa.toml")
}
