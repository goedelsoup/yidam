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
    for (name, dep) in &config.dependencies {
        // A path dependency has nothing to install and nothing to pin, so every question
        // this loop asks below is the wrong one. Until this branch existed it fell through
        // to `[missing lock] — run \`yidam tonpa install\``, and that install then failed
        // with "not yet supported" — about a dependency `deps::resolved` was reading
        // correctly the whole time. Three commands disagreed about whether the feature
        // existed.
        if let Some(rel) = &dep.path {
            let corpus = crate::paths::yidam_corpus_dir(&repo_root().join(rel));
            if corpus.is_dir() {
                println!("  [linked]         {name}  → {rel}  (path, unpinned)");
            } else {
                println!("  [path missing]   {name}  → {rel}  — no {rel}/.yidam/corpus/");
                issues += 1;
            }
            continue;
        }

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

/// The repository root, or `.` when there is no repository.
///
/// Matching `add`'s resolution: a `path` in `tonpa.toml` is relative to the repository root,
/// which is what someone writing `../sibling` means.
fn repo_root() -> std::path::PathBuf {
    crate::paths::repo_root().unwrap_or_else(|_| std::path::PathBuf::from("."))
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
        // Skipped, not fetched. `resolve_url` bails on a path dependency, and because that
        // is a `?` it aborted the whole command — so one path dependency stopped every
        // fetched one after it from installing, under an error message saying the feature
        // was unsupported.
        if let Some(rel) = &dep.path {
            println!("  ok  {name} (path dependency → {rel}, nothing to fetch)");
            continue;
        }

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

    // What every citation in this corpus resolves to, **before** anything is fetched (#267).
    // An update moves a pin; the question it was never able to answer is what that does to the
    // claims resting on the other side of it, and the only way to answer it is to have looked
    // first. `.yidam/tonpa/<pkg>/` is overwritten in place a few lines down.
    //
    // The repository root is the tonpa directory's grandparent: `<root>/.yidam/tonpa`.
    let root = tonpa_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or(tonpa_dir)
        .to_path_buf();
    let before = crate::cmd::lint::citations::survey(&root);

    let targets: Vec<String> = match name {
        Some(n) => {
            let Some(dep) = config.dependencies.get(n) else {
                bail!("no dependency named '{n}'");
            };
            // Naming a path dependency explicitly is worth an error rather than a silent
            // skip: the person asked for this one and needs to know why nothing happened.
            if let Some(rel) = &dep.path {
                bail!(
                    "'{n}' is a path dependency → {rel}\n  \
                     There is nothing to update: it is read from that directory as it \
                     currently stands. Update it by editing it."
                );
            }
            vec![n.to_string()]
        }
        // Whereas updating everything should update everything updatable, and say so.
        None => config
            .dependencies
            .iter()
            .filter(|(_, d)| d.path.is_none())
            .map(|(k, _)| k.clone())
            .collect(),
    };

    // Every declared dependency is a path one, so `targets` is empty. Saying so beats
    // exiting 0 in silence, which reads as "updated, no changes" rather than "there was
    // nothing here that update applies to".
    if targets.is_empty() {
        println!("Nothing to update: every declared dependency is a path dependency.");
        return Ok(());
    }

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

    // In terms of *my* graph rather than theirs. A diff of the dependency would be the
    // producer's news; this is the consumer's, and the consumer is the one accountable for
    // the claims.
    //
    // Only when something was actually fetched. `changed` is false exactly when every target
    // was already at the installed hash, so no bundle was replaced and no citation can have
    // moved — the second survey would be the first one, byte for byte, and printing "nothing
    // moved" there is a reassurance about a comparison that had nothing to compare.
    if changed {
        println!();
        println!(
            "{}",
            crate::cmd::lint::citations::render_movements(&crate::cmd::lint::citations::moved(
                &before,
                &crate::cmd::lint::citations::survey(&root)
            ))
        );
    }
    Ok(())
}
