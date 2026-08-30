use anyhow::{bail, Context, Result};
use std::path::Path;

// ── tonpa.toml ────────────────────────────────────────────────────────────────

// The config shapes live in `crate::deps`, not here. They were defined in this module when
// only the fetching commands read them; the read layer needs them too, and two definitions of
// "what this repository depends on" is exactly the drift this repository keeps finding in
// other people's code.
pub use crate::deps::{Dependency, TonpaConfig};

// ── tonpa.lock ────────────────────────────────────────────────────────────────

// The same move, one section later, for the same reason. `doctor` reads the lock to say
// whether the corpora arrived, and it has to do that in a build where `cmd::tonpa` does not
// exist — the module is gated on the `tonpa` feature, and reading a file and hashing it are
// not. Writing the lock is still a fetching concern and stays below.
pub use crate::deps::{load_lock, LockFile, LockedPackage};

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
        .find(|s| !s.is_empty() && *s != "bundle.yiz" && *s != "download" && *s != "latest")
        .unwrap_or("unknown")
        .trim_end_matches(".yiz")
        .to_string()
}

// ── I/O ───────────────────────────────────────────────────────────────────────

pub fn load_config(path: &Path) -> Result<TonpaConfig> {
    if !path.exists() {
        return Ok(TonpaConfig::default());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
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

pub fn save_lock(lock: &LockFile, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(lock).context("serialising tonpa.lock")?;
    std::fs::write(path, text)?;
    Ok(())
}
