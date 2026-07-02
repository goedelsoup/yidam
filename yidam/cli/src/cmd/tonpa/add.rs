use anyhow::{bail, Result};
use std::path::Path;

use super::config::{
    load_config, load_lock, name_from_url, resolve_url, save_config, save_lock, Dependency,
};
use super::install::install_package;

pub async fn cmd_add(
    source: &str,
    name_override: Option<&str>,
    tonpa_dir: &Path,
    config_path: &Path,
    lock_path: &Path,
) -> Result<()> {
    let mut config = load_config(config_path)?;
    let mut lock = load_lock(lock_path)?;

    let dep = parse_source(source)?;
    let url = resolve_url(&dep)?;
    let name = name_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| name_from_url(&url));

    if config.dependencies.contains_key(&name) {
        bail!("dependency '{name}' already declared — use `yidam tonpa update {name}` to refresh");
    }

    println!("Adding {name}");
    let locked = install_package(&name, &url, tonpa_dir, None).await?;

    config.dependencies.insert(name.clone(), dep);
    lock.packages.retain(|p| p.name != name);
    lock.packages.push(locked);

    save_config(&config, config_path)?;
    save_lock(&lock, lock_path)?;

    println!("Added '{name}' — commit .yidam/tonpa.toml and .yidam/tonpa/tonpa.lock");
    Ok(())
}

/// Parse a source string into a Dependency.
/// Accepts:
///   - Full URL: https://...
///   - GitHub shorthand: org/repo  or  org/repo@tag
fn parse_source(source: &str) -> Result<Dependency> {
    if source.starts_with("http://") || source.starts_with("https://") {
        return Ok(Dependency {
            url: Some(source.to_string()),
            github: None,
            tag: None,
            path: None,
        });
    }

    if source.contains('/') && !source.contains("://") {
        let (repo, tag) = source
            .split_once('@')
            .map(|(r, t)| (r, Some(t.to_string())))
            .unwrap_or((source, None));
        return Ok(Dependency {
            url: None,
            github: Some(repo.to_string()),
            tag,
            path: None,
        });
    }

    bail!(
        "cannot parse source '{source}'\n  expected a URL (https://...) or GitHub shorthand (org/repo[@tag])"
    );
}
