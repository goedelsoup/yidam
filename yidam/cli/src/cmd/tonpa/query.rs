use anyhow::{bail, Result};
use std::path::Path;

use super::config::{load_config, load_lock, resolve_url, save_lock};
use super::install::{install_package, verify_installed};

// ── list ──────────────────────────────────────────────────────────────────────

pub fn cmd_list(lock_path: &Path) -> Result<()> {
    let lock = load_lock(lock_path)?;
    if lock.packages.is_empty() {
        println!("No installed packages.");
        return Ok(());
    }

    let w = lock
        .packages
        .iter()
        .map(|p| p.name.len())
        .max()
        .unwrap_or(4)
        .max(4);

    println!("{:<w$}  {:>8}  {:<36}  GENESIS", "NAME", "NODES", "MODEL");
    println!("{}", "─".repeat(w + 55));
    for p in &lock.packages {
        println!(
            "{:<w$}  {:>8}  {:<36}  {}",
            p.name,
            p.nodes.unwrap_or(0),
            p.model.as_deref().unwrap_or("—"),
            p.genesis.as_deref().unwrap_or("—"),
        );
    }
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

pub fn cmd_status(tonpa_dir: &Path, config_path: &Path, lock_path: &Path) -> Result<()> {
    let config = load_config(config_path)?;
    let lock = load_lock(lock_path)?;

    if config.dependencies.is_empty() {
        println!("No dependencies declared in .yidam/tonpa.toml");
        return Ok(());
    }

    let mut issues = 0usize;
    for name in config.dependencies.keys() {
        let locked = lock.packages.iter().find(|p| &p.name == name);
        match locked {
            None => {
                println!("  [missing lock]   {name}  — run `yidam tonpa install`");
                issues += 1;
            }
            Some(lk) => {
                if !tonpa_dir.join(name).exists() {
                    println!("  [not installed]  {name}  — run `yidam tonpa install`");
                    issues += 1;
                } else if verify_installed(name, tonpa_dir, lk).unwrap_or(false) {
                    println!(
                        "  [ok]             {name}  ({} nodes)",
                        lk.nodes.unwrap_or(0)
                    );
                } else {
                    println!("  [hash mismatch]  {name}  — run `yidam tonpa install`");
                    issues += 1;
                }
            }
        }
    }

    for p in &lock.packages {
        if !config.dependencies.contains_key(&p.name) {
            println!(
                "  [orphaned]       {}  — run `yidam tonpa remove {}`",
                p.name, p.name
            );
            issues += 1;
        }
    }

    if issues == 0 {
        println!("All {} package(s) up to date.", config.dependencies.len());
    }
    Ok(())
}

// ── verify ────────────────────────────────────────────────────────────────────

pub fn cmd_verify(tonpa_dir: &Path, lock_path: &Path) -> Result<()> {
    let lock = load_lock(lock_path)?;
    if lock.packages.is_empty() {
        println!("Nothing to verify.");
        return Ok(());
    }

    let mut failed = false;
    for p in &lock.packages {
        if verify_installed(&p.name, tonpa_dir, p)? {
            println!("  ok    {}", p.name);
        } else {
            println!("  FAIL  {} — hash mismatch or bundle.yiz missing", p.name);
            failed = true;
        }
    }

    if failed {
        bail!("verification failed — run `yidam tonpa install` to restore");
    }
    println!("All {} package(s) verified.", lock.packages.len());
    Ok(())
}

// ── install ───────────────────────────────────────────────────────────────────

pub async fn cmd_install(tonpa_dir: &Path, config_path: &Path, lock_path: &Path) -> Result<()> {
    let config = load_config(config_path)?;
    let mut lock = load_lock(lock_path)?;

    if config.dependencies.is_empty() {
        println!("No dependencies declared.");
        return Ok(());
    }

    let mut changed = false;
    for (name, dep) in &config.dependencies {
        let url = resolve_url(dep)?;
        let existing = lock.packages.iter().find(|p| &p.name == name).cloned();

        match existing {
            Some(ref lk) if tonpa_dir.join(name).exists() => {
                if verify_installed(name, tonpa_dir, lk)? {
                    println!("  ok  {name} (already installed)");
                    continue;
                }
                // Files present but hash wrong — re-fetch at the pinned hash
                println!("  reinstalling {name} (hash mismatch) …");
                let locked = install_package(name, &url, tonpa_dir, Some(&lk.sha256)).await?;
                lock.packages.retain(|p| p.name != *name);
                lock.packages.push(locked);
                changed = true;
            }
            Some(ref lk) => {
                // In lock but not on disk — fetch at the pinned hash
                let locked = install_package(name, &url, tonpa_dir, Some(&lk.sha256)).await?;
                lock.packages.retain(|p| p.name != *name);
                lock.packages.push(locked);
                changed = true;
            }
            None => {
                // Not in lock yet — fetch freely, hash, add
                let locked = install_package(name, &url, tonpa_dir, None).await?;
                lock.packages.push(locked);
                changed = true;
            }
        }
    }

    if changed {
        save_lock(&lock, lock_path)?;
        println!("tonpa.lock updated.");
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

pub async fn cmd_update(
    name: Option<&str>,
    tonpa_dir: &Path,
    config_path: &Path,
    lock_path: &Path,
) -> Result<()> {
    let config = load_config(config_path)?;
    let mut lock = load_lock(lock_path)?;

    let targets: Vec<String> = match name {
        Some(n) => {
            if !config.dependencies.contains_key(n) {
                bail!("no dependency named '{n}'");
            }
            vec![n.to_string()]
        }
        None => config.dependencies.keys().cloned().collect(),
    };

    let mut changed = false;
    for n in &targets {
        let dep = &config.dependencies[n];
        let url = resolve_url(dep)?;
        let old_hash = lock
            .packages
            .iter()
            .find(|p| &p.name == n)
            .map(|p| p.sha256.clone());

        println!("Updating {n} …");
        let locked = install_package(n, &url, tonpa_dir, None).await?;

        match old_hash {
            Some(h) if h == locked.sha256 => {
                println!("  {n} is already up to date");
                continue;
            }
            Some(_) => println!("  {n} updated"),
            None => println!("  {n} added to lock"),
        }

        lock.packages.retain(|p| &p.name != n);
        lock.packages.push(locked);
        changed = true;
    }

    if changed {
        save_lock(&lock, lock_path)?;
    }
    Ok(())
}
