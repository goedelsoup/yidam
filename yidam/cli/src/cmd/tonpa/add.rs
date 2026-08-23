use anyhow::{bail, Result};
use std::path::Path;

use super::config::{
    load_config, load_lock, name_from_url, resolve_url, save_config, save_lock, Dependency,
};
use super::install::install_package;

/// Where a path dependency's corpus should be, relative to the current repository.
fn repo_root_of(rel: &str) -> std::path::PathBuf {
    let root = crate::paths::repo_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    crate::paths::yidam_corpus_dir(&root.join(rel))
}

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

    // A path dependency is declared, not installed. There is nothing to fetch and nothing
    // worth hashing: the lock file records what a bundle *was* at a moment, and a working
    // tree that changes under you has no such moment. It stays out of the lock, and
    // `tonpa status` reports it as unpinned rather than as missing.
    if let Some(rel) = dep.path.clone() {
        let name = name_override
            .map(str::to_string)
            .or_else(|| {
                Path::new(rel.trim_end_matches('/'))
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| rel.clone());

        if config.dependencies.contains_key(&name) {
            bail!("dependency '{name}' already declared — remove it first with `yidam tonpa remove {name}`");
        }

        let corpus = repo_root_of(&rel);
        if !corpus.is_dir() {
            bail!(
                "no corpus at {}\n  a path dependency points at a yidam repository; expected \
                 {rel}/.yidam/corpus/ to exist",
                corpus.display()
            );
        }

        config.dependencies.insert(name.clone(), dep);
        save_config(&config, config_path)?;
        println!("Added '{name}' -> {rel} (path dependency, not locked)");
        println!("Commit .yidam/tonpa.toml");
        return Ok(());
    }

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

    // Local paths BEFORE the GitHub shorthand, because every one of them contains a slash
    // and would otherwise be read as `org/repo`. `yidam tonpa add ../sibling` did not reach
    // the "not yet supported" error it was supposed to — it built
    // `https://github.com/../sibling/releases/latest/download/bundle.yiz` and asked the
    // network about it.
    if source.starts_with('.') || source.starts_with('/') || source.starts_with('~') {
        return Ok(Dependency {
            url: None,
            github: None,
            tag: None,
            path: Some(source.to_string()),
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
        "cannot parse source '{source}'\n  expected a URL (https://...), GitHub shorthand (org/repo[@tag]), or a local path (./ ../ / ~)"
    );
}
