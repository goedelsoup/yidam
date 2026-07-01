use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

// ── tonpa.toml ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct TonpaConfig {
    pub package: Option<PackageMeta>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    pub index: Option<IndexConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageMeta {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Dependency {
    pub url: Option<String>,
    pub github: Option<String>,
    pub tag: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IndexConfig {
    #[serde(default = "default_true")]
    pub merge_imported_index: bool,
}

fn default_true() -> bool {
    true
}

// ── tonpa.lock ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct LockFile {
    // TOML [[package]] array-of-tables
    #[serde(default, rename = "package")]
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LockedPackage {
    pub name: String,
    pub url: String,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dims: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<u64>,
}

// ── helpers ───────────────────────────────────────────────────────────────────

pub fn resolve_url(dep: &Dependency) -> Result<String> {
    if let Some(url) = &dep.url {
        return Ok(url.clone());
    }
    if let Some(github) = &dep.github {
        let tag = dep.tag.as_deref().unwrap_or("latest");
        if tag == "latest" {
            return Ok(format!(
                "https://github.com/{github}/releases/latest/download/bundle.yiz"
            ));
        }
        return Ok(format!(
            "https://github.com/{github}/releases/download/{tag}/bundle.yiz"
        ));
    }
    if let Some(path) = &dep.path {
        bail!("local path dependencies not yet supported: {path}");
    }
    bail!("dependency must specify at least one of: url, github, path");
}

/// Derive a package name from a URL when the user doesn't supply --name.
pub fn name_from_url(url: &str) -> String {
    // "https://github.com/org/my-repo/releases/.../bundle.yiz" → "my-repo"
    url.trim_end_matches('/')
        .rsplit('/')
        .find(|s| {
            !s.is_empty()
                && *s != "bundle.yiz"
                && *s != "download"
                && *s != "latest"
        })
        .unwrap_or("unknown")
        .trim_end_matches(".yiz")
        .to_string()
}

// ── I/O ───────────────────────────────────────────────────────────────────────

pub fn load_config(path: &Path) -> Result<TonpaConfig> {
    if !path.exists() {
        return Ok(TonpaConfig::default());
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn save_config(config: &TonpaConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(config).context("serialising tonpa.toml")?;
    std::fs::write(path, text)?;
    Ok(())
}

pub fn load_lock(path: &Path) -> Result<LockFile> {
    if !path.exists() {
        return Ok(LockFile::default());
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn save_lock(lock: &LockFile, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(lock).context("serialising tonpa.lock")?;
    std::fs::write(path, text)?;
    Ok(())
}
