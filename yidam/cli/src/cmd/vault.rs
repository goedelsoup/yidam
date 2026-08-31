//! `yidam vault` — the artifact store, from the command line.
//!
//! # Which of these need a repository
//!
//! The rule is one sentence: **a command that reads the vault configuration needs a
//! repository; a command that only touches the cache does not.**
//!
//! `list` and `get` read `.yidam/config.toml`, so they require one. `put`, `path` and
//! `verify` work entirely against the machine-wide cache, which belongs to no repository —
//! demanding one would be friction with nothing behind it, and would make the cache harder to
//! inspect exactly when something has gone wrong with it.
//!
//! # No network, in any of them
//!
//! This is the offline half of RFC-0023. `get` reaches a store, and the only store this build
//! can open is a directory. When the S3 transport lands it arrives behind [`crate::vault::Store`]
//! and none of these commands changes shape.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Subcommand;

use crate::config::load_yidam_config;
use crate::paths::{repo_root, require_yidam_repo};
use crate::vault::{self, Cache, ContentHash, Verdict};

#[derive(Debug, Subcommand)]
pub enum VaultCommand {
    /// List the stores this repository declares, and who each says can read it
    List,
    /// Hash a file and keep it in the local cache, printing its content address
    Put {
        /// The file to take in
        path: PathBuf,
    },
    /// Fetch an artifact by content address — from the cache, else from the vault
    Get {
        /// The artifact's sha256, as 64 lowercase hex characters
        sha256: String,
        /// Write a copy here as well, with a name a person can use
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print where an artifact sits locally, or exit nonzero if it does not
    Path {
        /// The artifact's sha256, as 64 lowercase hex characters
        sha256: String,
    },
    /// Re-hash every cached artifact and report anything that is not what it claims
    Verify,
}

pub fn run(sub: VaultCommand) -> Result<()> {
    match sub {
        VaultCommand::List => list(),
        VaultCommand::Put { path } => put(&path),
        VaultCommand::Get { sha256, out } => get(&sha256, out.as_deref()),
        VaultCommand::Path { sha256 } => path_of(&sha256),
        VaultCommand::Verify => verify(),
    }
}

/// The cache this machine uses.
fn cache() -> Result<Cache> {
    Cache::resolve(|k| std::env::var(k).ok())
}

/// The vault this repository declares, having established there is a repository.
///
/// Returns `None` when no vault is configured, which is every corpus until somebody
/// configures one and is not an error.
fn configured() -> Result<Option<(String, vault::VaultConfig)>> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let config = load_yidam_config(&root)?;
    Ok(vault::resolve(&config.vault)?.map(|(n, c)| (n.to_string(), c.clone())))
}

fn list() -> Result<()> {
    let cache = cache()?;
    match configured()? {
        None => {
            println!("No vault configured.");
            println!();
            println!(
                "  Artifacts are kept in the local cache at {} and go nowhere else.",
                cache.root().display()
            );
            println!("  Declare a store in `.yidam/config.toml` to keep them somewhere durable:");
            println!();
            println!("    [vault.{}]", vault::ONLY_VAULT);
            println!("    url      = \"file:///mnt/archive/yidam\"");
            println!("    audience = \"Who can read this store, and why that is acceptable.\"");
        }
        Some((name, cfg)) => {
            println!("{name}");
            println!("  url       {}", cfg.url);
            println!("  audience  {}", cfg.audience());
            // Whether the store can actually be opened is worth knowing here, because the
            // alternative is learning it from the first `get` that needed it. The path is
            // already on the line above, so say only whether it worked — and when it did
            // not, say why, at the width the rest of the block uses.
            match vault::open(&cfg) {
                Ok(_) => println!("  store     ready"),
                Err(e) => {
                    let mut lines = e
                        .to_string()
                        .lines()
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                    let first = lines.first().cloned().unwrap_or_default();
                    println!("  store     unusable — {first}");
                    for rest in lines.drain(1..) {
                        println!("            {}", rest.trim());
                    }
                }
            }
            println!();
            println!("  cache     {}", cache.root().display());
        }
    }
    Ok(())
}

fn put(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("{} is not a file", path.display());
    }
    let cache = cache()?;
    let hash = ContentHash::of_file(path).with_context(|| format!("hashing {}", path.display()))?;
    cache.put_file(path, &hash)?;
    // The digest alone on stdout, so `yidam vault put x | …` is usable. Everything a person
    // wants to read goes to stderr.
    eprintln!("cached {} ({})", path.display(), human_bytes(path));
    println!("{hash}");
    Ok(())
}

fn get(sha256: &str, out: Option<&Path>) -> Result<()> {
    let hash = ContentHash::parse(sha256)?;
    let cache = cache()?;

    if !cache.contains(&hash) {
        let Some((name, cfg)) = configured()? else {
            bail!(
                "{hash} is not in the local cache, and this repository declares no vault to \
                 fetch it from.\n  \
                 Declare one in `.yidam/config.toml` — `yidam vault list` shows the shape."
            );
        };
        let store = vault::open(&cfg)?;
        if !store.has(&hash)? {
            bail!(
                "{hash} is in neither the local cache nor vault `{name}` ({}).",
                store.describe()
            );
        }
        // Into the cache through the cache's own atomic write, then verified before it is
        // allowed to count as present. A store that hands back the wrong bytes under a name
        // is exactly what content addressing exists to catch, and catching it after the fact
        // would mean a corrupt artifact had already been readable.
        let staged = cache.path_of(&hash).with_extension("incoming");
        store.get(&hash, &staged)?;
        let found = ContentHash::of_file(&staged)?;
        if found != hash {
            let _ = std::fs::remove_file(&staged);
            bail!(
                "vault `{name}` returned bytes that are not {hash}.\n  \
                 asked for {hash}\n  \
                 received {found}\n  \
                 Nothing was cached. The store's copy is wrong, or something rewrote it."
            );
        }
        cache.put_file(&staged, &hash)?;
        let _ = std::fs::remove_file(&staged);
        eprintln!("fetched {hash} from `{name}`");
    }

    let at = cache.path_of(&hash);
    if let Some(dest) = out {
        if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::copy(&at, dest).with_context(|| format!("writing {}", dest.display()))?;
        println!("{}", dest.display());
    } else {
        println!("{}", at.display());
    }
    Ok(())
}

fn path_of(sha256: &str) -> Result<()> {
    let hash = ContentHash::parse(sha256)?;
    let cache = cache()?;
    if !cache.contains(&hash) {
        // An error rather than an empty line, so `yidam vault path $h || fetch` works.
        bail!(
            "{hash} is not in the local cache at {}",
            cache.root().display()
        );
    }
    println!("{}", cache.path_of(&hash).display());
    Ok(())
}

fn verify() -> Result<()> {
    let cache = cache()?;
    let entries = cache.entries()?;
    if entries.is_empty() {
        println!("Nothing cached at {}.", cache.root().display());
        return Ok(());
    }

    let mut corrupt = Vec::new();
    for hash in &entries {
        match cache.verify(hash)? {
            Verdict::Intact => {}
            Verdict::Corrupt { found } => corrupt.push((hash.clone(), found)),
            // Listed a moment ago and gone now. Rare, and not a corpus problem — report it
            // as what it is rather than folding it into corruption, which means something
            // much worse.
            Verdict::Absent => eprintln!("warning: {hash} disappeared while being verified"),
        }
    }

    println!(
        "{} artifact{} at {}",
        entries.len(),
        if entries.len() == 1 { "" } else { "s" },
        cache.root().display()
    );
    if corrupt.is_empty() {
        println!("All intact.");
        return Ok(());
    }
    println!();
    for (expected, found) in &corrupt {
        println!("corrupt  {expected}");
        println!("         hashes to {found}");
    }
    bail!(
        "{} cached artifact{} do not match the name they are filed under. Delete them and \
         fetch again; the digest in the corpus is what is authoritative.",
        corrupt.len(),
        if corrupt.len() == 1 { "" } else { "s" }
    )
}

fn human_bytes(path: &Path) -> String {
    let Ok(meta) = std::fs::metadata(path) else {
        return "unknown size".to_string();
    };
    let n = meta.len();
    const UNITS: [(u64, &str); 4] = [
        (1 << 30, "GiB"),
        (1 << 20, "MiB"),
        (1 << 10, "KiB"),
        (1, "bytes"),
    ];
    for (scale, unit) in UNITS {
        if n >= scale {
            return if scale == 1 {
                format!("{n} {unit}")
            } else {
                format!("{:.1} {unit}", n as f64 / scale as f64)
            };
        }
    }
    "0 bytes".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_the_way_a_person_would_write_them() {
        let tmp = tempfile::TempDir::new().unwrap();
        let p = tmp.path().join("f");
        std::fs::write(&p, vec![0u8; 2048]).unwrap();
        assert_eq!(human_bytes(&p), "2.0 KiB");
        std::fs::write(&p, vec![0u8; 3]).unwrap();
        assert_eq!(human_bytes(&p), "3 bytes");
        std::fs::write(&p, Vec::new()).unwrap();
        assert_eq!(human_bytes(&p), "0 bytes");
        assert_eq!(human_bytes(Path::new("/nonexistent")), "unknown size");
    }
}
