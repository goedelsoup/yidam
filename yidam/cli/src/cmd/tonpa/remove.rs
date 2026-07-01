use anyhow::{bail, Result};
use std::path::Path;

use super::config::{load_config, load_lock, save_config, save_lock};

pub fn cmd_remove(
    name: &str,
    tonpa_dir: &Path,
    config_path: &Path,
    lock_path: &Path,
) -> Result<()> {
    let mut config = load_config(config_path)?;
    let mut lock = load_lock(lock_path)?;

    if !config.dependencies.contains_key(name) {
        bail!("no declared dependency named '{name}'");
    }

    config.dependencies.remove(name);
    lock.packages.retain(|p| p.name != name);

    let dest = tonpa_dir.join(name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
        println!("Deleted {}", dest.display());
    }

    save_config(&config, config_path)?;
    save_lock(&lock, lock_path)?;

    println!("Removed dependency '{name}'");
    Ok(())
}
