use anyhow::{bail, Context, Result};
use std::path::Path;

// ── tonpa.toml ────────────────────────────────────────────────────────────────

// The config shapes live in `crate::deps`, not here. They were defined in this module when
// only the fetching commands read them; the read layer needs them too, and two definitions of
// "what this repository depends on" is exactly the drift this repository keeps finding in
// other people's code. The loader moved with them (#1616): this module's strict one and the
// read layer's lenient one were two answers to whether a file that does not parse declares
// anything.
pub use crate::deps::{load_config, Dependency, TonpaConfig};

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

/// Why `export` is refused on a dependency that is not a pinned path (RFC-0063 §1). One
/// sentence for `add`, `install` and `update`, so the three cannot drift.
pub fn export_refusal(dep: &Dependency, export: &str) -> String {
    if dep.is_live_path() {
        format!(
            "export `{export}` needs a pinned path dependency\n  a working tree has no commit \
             to read the export at; pin it with `<path>@<commit>`"
        )
    } else {
        format!(
            "export `{export}` needs a pinned path dependency\n  a fetched bundle's export \
             belongs in its manifest, and tonpa reads none there"
        )
    }
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
