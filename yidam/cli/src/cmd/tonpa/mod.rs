mod add;
mod config;
mod install;
mod query;
mod remove;

pub use config::{LockFile, LockedPackage, TonpaConfig};

use anyhow::Result;
use clap::Subcommand;

use crate::paths::{tonpa_config_path, tonpa_dir};

#[derive(Debug, Subcommand)]
pub enum TonpaCommand {
    /// Add a bundle dependency, fetch it, and update the lock file
    Add {
        /// Bundle URL (https://...) or GitHub shorthand (org/repo[@tag])
        source: String,
        /// Override the package name derived from the URL
        #[arg(long)]
        name: Option<String>,
    },
    /// Install all dependencies declared in .yidam/tonpa.toml
    Install,
    /// Remove a dependency and delete its installed files
    Remove {
        /// Package name to remove
        name: String,
    },
    /// List installed packages with node counts and model info
    List,
    /// Show the status of each declared dependency (installed / missing / stale)
    Status,
    /// Verify installed bundle hashes against the lock file
    Verify,
    /// Re-fetch and update one or all dependencies to the latest bundle
    Update {
        /// Package name to update (omit to update all)
        name: Option<String>,
    },
}

pub async fn run(sub: TonpaCommand) -> Result<()> {
    let root = crate::paths::repo_root()?;
    let dir = tonpa_dir(&root);
    let config_path = tonpa_config_path(&root);
    let lock_path = dir.join("tonpa.lock");

    match sub {
        TonpaCommand::Add { source, name } => {
            add::cmd_add(&source, name.as_deref(), &dir, &config_path, &lock_path).await
        }
        TonpaCommand::Install => query::cmd_install(&dir, &config_path, &lock_path).await,
        TonpaCommand::Remove { name } => remove::cmd_remove(&name, &dir, &config_path, &lock_path),
        TonpaCommand::List => query::cmd_list(&lock_path),
        TonpaCommand::Status => query::cmd_status(&dir, &config_path, &lock_path),
        TonpaCommand::Verify => query::cmd_verify(&dir, &lock_path),
        TonpaCommand::Update { name } => {
            query::cmd_update(name.as_deref(), &dir, &config_path, &lock_path).await
        }
    }
}
