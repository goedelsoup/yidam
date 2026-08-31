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
    /// Upload what the corpus names and the vault lacks
    Push {
        /// Show what would be sent — including the exact string that would be signed — and
        /// send nothing
        #[arg(long)]
        dry_run: bool,
        /// Restrict to one artifact, by content address. It must still be named by a catalog
        /// entry: `--artifact` narrows what is pushed, it never bypasses the guard.
        #[arg(long)]
        artifact: Option<String>,
    },
    /// Fetch what the corpus names and the cache lacks
    Pull,
    /// What the corpus names, and where each artifact is
    Status {
        /// Ask the vault about each artifact. One HEAD per record — bounded by the catalog,
        /// never by the bucket, because nothing here lists a store.
        #[arg(long)]
        remote: bool,
    },
}

pub fn run(sub: VaultCommand) -> Result<()> {
    match sub {
        VaultCommand::List => list(),
        VaultCommand::Put { path } => put(&path),
        VaultCommand::Get { sha256, out } => get(&sha256, out.as_deref()),
        VaultCommand::Path { sha256 } => path_of(&sha256),
        VaultCommand::Verify => verify(),
        VaultCommand::Push { dry_run, artifact } => push(dry_run, artifact.as_deref()),
        VaultCommand::Pull => pull(),
        VaultCommand::Status { remote } => status(remote),
    }
}

/// The artifacts the corpus names, narrowed to one if asked.
///
/// **`--artifact` narrows and never widens.** A digest the catalog does not name is refused
/// rather than pushed from the cache: an artifact with no record has no `redistributable`, no
/// path to check against `.yidam/private-paths`, and therefore nothing for the guard to read.
/// Allowing it would be a hole in the guard that looked like a convenience flag.
fn selected(root: &Path, artifact: Option<&str>) -> Result<Vec<vault::Named>> {
    let all = vault::named_artifacts(root);
    let Some(want) = artifact else {
        return Ok(all);
    };
    let hash = ContentHash::parse(want)?;
    let picked: Vec<_> = all.into_iter().filter(|a| a.hash == hash).collect();
    if picked.is_empty() {
        bail!(
            "no catalog entry names {hash}.\n  \
             `--artifact` narrows what is pushed; it does not push something the corpus has \
             not recorded. An artifact with no record carries no `redistributable` and no \
             path to check for privacy, so there would be nothing for the guard to read."
        );
    }
    Ok(picked)
}

fn push(dry_run: bool, artifact: Option<&str>) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let Some((name, cfg)) = configured()? else {
        bail!("this repository declares no vault — `yidam vault list` shows the shape.");
    };
    let cache = cache()?;
    let private = vault::read_private_paths(&root)?;
    let wanted = selected(&root, artifact)?;
    if wanted.is_empty() {
        println!("The corpus names no artifacts.");
        return Ok(());
    }
    let store = vault::open(&name, &cfg)?;

    let (mut sent, mut present, mut uncached) = (0usize, 0usize, 0usize);
    let mut refused: Vec<String> = Vec::new();

    for a in &wanted {
        match vault::may_push(a, &private) {
            vault::Disposition::Refused(why) => {
                refused.push(format!("{} — {why}", a.hash));
                continue;
            }
            vault::Disposition::Push => {}
        }
        if !cache.contains(&a.hash) {
            // Nothing to send. Not an error: a clone that has never fetched an artifact is a
            // normal state, and reporting it as a failure would make `push` red on every
            // fresh checkout.
            eprintln!("not cached, nothing to send: {} ({})", a.hash, a.rel);
            uncached += 1;
            continue;
        }
        if dry_run {
            println!("would send {} ({})", a.hash, a.rel);
            if let Some(explain) = store.explain_put(&a.hash) {
                for line in explain.lines() {
                    println!("    {line}");
                }
            }
            println!();
            sent += 1;
            continue;
        }
        if store.has(&a.hash)? {
            present += 1;
            continue;
        }
        store.put(&a.hash, &cache.path_of(&a.hash))?;
        println!("sent {} ({})", a.hash, a.rel);
        sent += 1;
    }

    println!();
    println!(
        "{} artifact{} named; {sent} {}; {present} already in `{name}`; {uncached} not cached; \
         {} refused",
        wanted.len(),
        if wanted.len() == 1 { "" } else { "s" },
        if dry_run { "would be sent" } else { "sent" },
        refused.len()
    );
    if !refused.is_empty() {
        println!();
        println!("Refused — `{name}` serves: {}", cfg.audience());
        for r in &refused {
            println!("  {r}");
        }
    }
    Ok(())
}

fn pull() -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let Some((name, cfg)) = configured()? else {
        bail!("this repository declares no vault — `yidam vault list` shows the shape.");
    };
    let cache = cache()?;
    let wanted = vault::named_artifacts(&root);
    if wanted.is_empty() {
        println!("The corpus names no artifacts.");
        return Ok(());
    }
    let store = vault::open(&name, &cfg)?;

    let (mut fetched, mut held, mut absent) = (0usize, 0usize, 0usize);
    for a in &wanted {
        if cache.contains(&a.hash) {
            held += 1;
            continue;
        }
        if a.vault.as_deref() == Some("none") {
            // Routed to the local cache and nowhere else, so there is nothing to pull from.
            absent += 1;
            eprintln!("{} is `vault: none` and is not here ({})", a.hash, a.rel);
            continue;
        }
        if !store.has(&a.hash)? {
            absent += 1;
            eprintln!("{} is not in `{name}` ({})", a.hash, a.rel);
            continue;
        }
        let staged = cache.path_of(&a.hash).with_extension("incoming");
        store.get(&a.hash, &staged)?;
        // Verified before it may count as present, for the reason `get` gives: a store that
        // hands back the wrong bytes is what content addressing exists to catch, and catching
        // it later means a corrupt artifact was already readable.
        let found = ContentHash::of_file(&staged)?;
        if found != a.hash {
            let _ = std::fs::remove_file(&staged);
            bail!(
                "vault `{name}` returned bytes that are not {} for {}.\n  \
                 received {found}\n  Nothing was cached.",
                a.hash,
                a.rel
            );
        }
        cache.put_file(&staged, &a.hash)?;
        let _ = std::fs::remove_file(&staged);
        println!("fetched {} ({})", a.hash, a.rel);
        fetched += 1;
    }
    println!();
    println!(
        "{} named; {fetched} fetched; {held} already cached; {absent} unavailable",
        wanted.len()
    );
    if absent > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn status(remote: bool) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let cache = cache()?;
    let wanted = vault::named_artifacts(&root);
    if wanted.is_empty() {
        println!("The corpus names no artifacts.");
        return Ok(());
    }
    let configured = configured()?;
    let store = match (&configured, remote) {
        (Some((name, cfg)), true) => Some((name.clone(), vault::open(name, cfg)?)),
        _ => None,
    };
    if remote && store.is_none() {
        bail!("`--remote` needs a vault, and this repository declares none.");
    }

    let mut mismatched = 0usize;
    for a in &wanted {
        // Cache first, and re-hashed rather than trusted: the local answer is the one that
        // can be wrong in a way nothing else would notice.
        let local = match cache.verify(&a.hash)? {
            Verdict::Intact => "cached",
            Verdict::Absent => "-",
            Verdict::Corrupt { .. } => {
                mismatched += 1;
                "CORRUPT"
            }
        };
        let remote_state = match &store {
            None => "".to_string(),
            Some((_, s)) => {
                if a.vault.as_deref() == Some("none") {
                    "  local-only".to_string()
                } else if s.has(&a.hash)? {
                    "  stored".to_string()
                } else {
                    "  absent".to_string()
                }
            }
        };
        println!("{:<8}{remote_state:<10}  {}  {}", local, a.hash, a.rel);
    }
    println!();
    println!(
        "{} artifact{} named by the corpus",
        wanted.len(),
        if wanted.len() == 1 { "" } else { "s" }
    );
    if mismatched > 0 {
        bail!(
            "{mismatched} cached artifact{} do not match the digest the corpus records.",
            if mismatched == 1 { "" } else { "s" }
        );
    }
    Ok(())
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
            match vault::open(&name, &cfg) {
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
        let store = vault::open(&name, &cfg)?;
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
