//! `yidam vault` — the artifact store, from the command line.
//!
//! # Which of these need a repository
//!
//! The rule is one sentence: **a command that reads the vault configuration needs a
//! repository; a command that only touches the cache does not.**
//!
//! `list`, `get`, `push`, `pull` and `status` read `.yidam/config.toml`, so they require one.
//! `put`, `path` and `verify` work entirely against the machine-wide cache, which belongs to
//! no repository — demanding one would be friction with nothing behind it, and would make the
//! cache harder to inspect exactly when something has gone wrong with it.
//!
//! # Routing happens once, before anything opens a store
//!
//! Every command that moves bytes builds the same plan first: each artifact the corpus names,
//! paired with the vault it goes to. That plan is a function of two committed files and
//! nothing else — no credentials, no network, no cache — so it answers identically in every
//! clone, and `--dry-run` shows the real one rather than a rehearsal of it.
//!
//! **Stores are then opened lazily, one per vault that has work.** This is not an
//! optimisation. Opening an S3 store resolves that vault's credentials, and a runner that has
//! keys for `default` and none for `sources` must be able to push to `default` — which is the
//! whole reason `--vault` exists. Opening every declared store up front would make the flag
//! useless in the situation it was added for.
//!
//! # `--vault` narrows and never re-routes
//!
//! An artifact routed to `sources` is not pushed to `default` because somebody typed a flag.
//! Moving an artifact between stores is an edit to its record, in a commit, like every other
//! assertion this repository makes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Subcommand;

use crate::config::load_yidam_config;
use crate::paths::{repo_root, require_yidam_repo};
use crate::vault::{self, Cache, ContentHash, Route, Vaults, Verdict};

#[derive(Debug, Subcommand)]
pub enum VaultCommand {
    /// List the stores this repository declares, what each holds, and who can read it
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
        /// Restrict to one vault. Narrows what is sent; never re-routes an artifact into a
        /// store its record did not send it to.
        #[arg(long)]
        vault: Option<String>,
        /// Send the built index (`.yidam/index/`) instead of the catalog's artifacts, and
        /// record it in `.yidam/index.lock`
        #[arg(long)]
        index: bool,
        /// Send the embeddings the index was built from
        #[arg(long)]
        embeddings: bool,
        /// Send `.yidam/bundle.yiz`
        #[arg(long)]
        bundle: bool,
    },
    /// Fetch what the corpus names and the cache lacks
    Pull {
        /// Restrict to one vault — useful where one store is reachable and another is not
        #[arg(long)]
        vault: Option<String>,
        /// Fetch the index `.yidam/index.lock` records and unpack it into `.yidam/index/`
        #[arg(long)]
        index: bool,
        /// Fetch the recorded embeddings
        #[arg(long)]
        embeddings: bool,
        /// Fetch the recorded bundle
        #[arg(long)]
        bundle: bool,
    },
    /// What the corpus names, where each artifact goes, and where it is
    Status {
        /// Ask each vault about the artifacts routed to it. One HEAD per record — bounded by
        /// the catalog, never by the bucket, because nothing here lists a store.
        #[arg(long)]
        remote: bool,
        /// Restrict to one vault
        #[arg(long)]
        vault: Option<String>,
    },
}

pub fn run(sub: VaultCommand) -> Result<()> {
    match sub {
        VaultCommand::List => list(),
        VaultCommand::Put { path } => put(&path),
        VaultCommand::Get { sha256, out } => get(&sha256, out.as_deref()),
        VaultCommand::Path { sha256 } => path_of(&sha256),
        VaultCommand::Verify => verify(),
        VaultCommand::Push {
            dry_run,
            artifact,
            vault,
            index,
            embeddings,
            bundle,
        } => match chosen(index, embeddings, bundle) {
            picked if picked.is_empty() => push(dry_run, artifact.as_deref(), vault.as_deref()),
            picked => push_derived(&picked, dry_run, vault.as_deref()),
        },
        VaultCommand::Pull {
            vault,
            index,
            embeddings,
            bundle,
        } => match chosen(index, embeddings, bundle) {
            picked if picked.is_empty() => pull(vault.as_deref()),
            picked => pull_derived(&picked, vault.as_deref()),
        },
        VaultCommand::Status { remote, vault } => status(remote, vault.as_deref()),
    }
}

/// Where one artifact goes, resolved.
enum Dest {
    /// A declared vault, by name.
    Vault(String),
    /// The local cache and nowhere else — `vault: none`.
    Local,
    /// Nowhere, and why.
    Nowhere(String),
}

/// Artifacts grouped by the vault they go to.
type ByVault = BTreeMap<String, Vec<vault::Named>>;

/// Artifacts that reach no store, each under a heading saying why not.
type Aside = Vec<(&'static str, String)>;

/// One artifact with its destination settled.
struct Routed {
    a: vault::Named,
    dest: Dest,
}

/// The artifacts the corpus names, each paired with where it goes.
///
/// **`--artifact` narrows and never widens.** A digest the catalog does not name is refused
/// rather than pushed from the cache: an artifact with no record has no `redistributable`, no
/// path to check against `.yidam/private-paths`, and therefore nothing for the guard to read.
/// Allowing it would be a hole in the guard that looked like a convenience flag.
///
/// **`--vault` narrows too, and by *destination* rather than by name.** The filter is applied
/// after routing, so a flag can only ever remove artifacts from the plan. There is no code
/// path by which typing a vault's name puts something into it.
fn plan(
    root: &Path,
    vaults: &Vaults,
    artifact: Option<&str>,
    only: Option<&str>,
) -> Result<Vec<Routed>> {
    if let Some(name) = only {
        if vaults.get(name).is_none() {
            bail!(
                "`--vault {name}` names no declared vault ({}).\n  \
                 The flag narrows what this command touches; it does not create a route.",
                describe_declared(vaults)
            );
        }
    }

    let mut all = vault::named_artifacts(root);
    if let Some(want) = artifact {
        let hash = ContentHash::parse(want)?;
        all.retain(|a| a.hash == hash);
        if all.is_empty() {
            bail!(
                "no catalog entry names {hash}.\n  \
                 `--artifact` narrows what is pushed; it does not push something the corpus \
                 has not recorded. An artifact with no record carries no `redistributable` \
                 and no path to check for privacy, so there would be nothing for the guard \
                 to read."
            );
        }
    }

    let mut out = Vec::new();
    for a in all {
        let dest = match vaults.route(&a.kind, a.vault.as_deref()) {
            Route::Local => Dest::Local,
            Route::To(name, _) => Dest::Vault(name.to_string()),
            Route::Unroutable(why) => Dest::Nowhere(why),
        };
        if let Some(name) = only {
            if !matches!(&dest, Dest::Vault(v) if v == name) {
                continue;
            }
        }
        out.push(Routed { a, dest });
    }
    Ok(out)
}

fn describe_declared(vaults: &Vaults) -> String {
    if vaults.is_empty() {
        return "this repository declares none".to_string();
    }
    format!(
        "declared: {}",
        vaults
            .names()
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Group a plan by the vault each artifact goes to, keeping the refusals aside.
///
/// The vault map is a `BTreeMap` so the output is ordered by vault name — which makes a run
/// over several stores diffable, and means the section a person is looking for is where it
/// was last time.
fn group(plan: Vec<Routed>) -> (ByVault, Aside) {
    let mut by_vault: ByVault = BTreeMap::new();
    let mut aside: Aside = Vec::new();
    for r in plan {
        match r.dest {
            Dest::Vault(name) => by_vault.entry(name).or_default().push(r.a),
            // Kept apart from the unroutable ones on purpose. `vault: none` is a decision
            // somebody made, and listing it under the same heading as a broken route would
            // report a deliberate choice as a defect.
            Dest::Local => aside.push((
                KEPT_LOCAL,
                format!(
                    "{} — the local cache and nowhere else ({})",
                    r.a.hash, r.a.rel
                ),
            )),
            Dest::Nowhere(why) => {
                aside.push((NO_ROUTE, format!("{} — {why} ({})", r.a.hash, r.a.rel)))
            }
        }
    }
    (by_vault, aside)
}

const KEPT_LOCAL: &str = "kept local (`vault: none`)";
const NO_ROUTE: &str = "no route";

/// How to describe the set a summary line counted.
///
/// A narrowed run has counted a subset, and calling that subset "named by the corpus" would
/// report a smaller corpus than the one on disk — the kind of number that looks true.
fn scope(only: Option<&str>) -> String {
    match only {
        Some(n) => format!("routed to `{n}`"),
        None => "named by the corpus".to_string(),
    }
}

/// A store and who reads it, on one line. The same shape in `push` and in `status`, because
/// a person comparing the two outputs is looking for the same store in both.
fn heading(name: &str, cfg: &vault::VaultConfig) -> String {
    format!("{name} — {}", cfg.audience())
}

fn push(dry_run: bool, artifact: Option<&str>, only: Option<&str>) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let vaults = configured(&root)?;
    if vaults.is_empty() {
        bail!("this repository declares no vault — `yidam vault list` shows the shape.");
    }
    let cache = cache()?;
    let private = vault::read_private_paths(&root)?;
    let planned = plan(&root, &vaults, artifact, only)?;
    if planned.is_empty() {
        println!("Nothing to push.");
        return Ok(());
    }
    let total = planned.len();
    let (by_vault, aside) = group(planned);

    // Refusals are grouped by the store they were headed for, and each heading carries that
    // store's own audience. A reader learns what they were about to publish *to*, which is
    // the fact that makes a refusal reassuring rather than merely obstructive — and with
    // several vaults it is the only way to tell which boundary held.
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut refused_count = aside.len();
    for (heading, line) in aside {
        refused.entry(heading.to_string()).or_default().push(line);
    }

    let (mut sent, mut present, mut uncached) = (0usize, 0usize, 0usize);
    for (name, artifacts) in &by_vault {
        // Only what the licence and the private paths allow is worth opening a store for. A
        // run in which every artifact is refused must not ask for credentials it will not use.
        let cfg = vaults.get(name).expect("routed to a declared vault");
        let heading = heading(name, cfg);
        let allowed: Vec<&vault::Named> = artifacts
            .iter()
            .filter(|a| match vault::may_push(a, &private) {
                vault::Disposition::Push => true,
                vault::Disposition::Refused(why) => {
                    refused_count += 1;
                    refused
                        .entry(heading.clone())
                        .or_default()
                        .push(format!("{} — {why}", a.hash));
                    false
                }
            })
            .collect();
        if allowed.is_empty() {
            continue;
        }

        let store = vault::open(name, cfg)?;
        // The audience first, the destination under it. Somebody about to move bytes should
        // read who will be able to see them before they read where they are going.
        println!("{heading}");
        println!("  → {}", store.describe());
        for a in allowed {
            if !cache.contains(&a.hash) {
                // Nothing to send. Not an error: a clone that has never fetched an artifact
                // is a normal state, and reporting it as a failure would make `push` red on
                // every fresh checkout.
                eprintln!("  not cached, nothing to send: {} ({})", a.hash, a.rel);
                uncached += 1;
                continue;
            }
            if dry_run {
                println!("  would send {} ({})", a.hash, a.rel);
                if let Some(explain) = store.explain_put(&a.hash) {
                    for line in explain.lines() {
                        println!("      {line}");
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
            println!("  sent {} ({})", a.hash, a.rel);
            sent += 1;
        }
        println!();
    }

    println!(
        "{total} artifact{} {}; {sent} {}; {present} already stored; {uncached} not cached; \
         {refused_count} refused",
        if total == 1 { "" } else { "s" },
        scope(only),
        if dry_run { "would be sent" } else { "sent" },
    );
    if !refused.is_empty() {
        println!();
        println!("Refused:");
        for (heading, lines) in &refused {
            println!("  {heading}");
            for l in lines {
                println!("    {l}");
            }
        }
    }
    Ok(())
}

fn pull(only: Option<&str>) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let vaults = configured(&root)?;
    if vaults.is_empty() {
        bail!("this repository declares no vault — `yidam vault list` shows the shape.");
    }
    let cache = cache()?;
    let planned = plan(&root, &vaults, None, only)?;
    if planned.is_empty() {
        println!("Nothing to pull.");
        return Ok(());
    }
    let total = planned.len();
    let (by_vault, aside) = group(planned);

    let (mut fetched, mut held) = (0usize, 0usize);
    let mut absent = aside.len();
    for (heading, line) in &aside {
        eprintln!("{heading}: {line}");
    }
    for (name, artifacts) in &by_vault {
        let outstanding: Vec<&vault::Named> = artifacts
            .iter()
            .filter(|a| !cache.contains(&a.hash))
            .collect();
        held += artifacts.len() - outstanding.len();
        if outstanding.is_empty() {
            continue;
        }
        let cfg = vaults.get(name).expect("routed to a declared vault");
        let store = vault::open(name, cfg)?;
        for a in outstanding {
            if !store.has(&a.hash)? {
                absent += 1;
                eprintln!("{} is not in `{name}` ({})", a.hash, a.rel);
                continue;
            }
            let staged = cache.path_of(&a.hash).with_extension("incoming");
            store.get(&a.hash, &staged)?;
            // Verified before it may count as present, for the reason `get` gives: a store
            // that hands back the wrong bytes is what content addressing exists to catch, and
            // catching it later means a corrupt artifact was already readable.
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
            println!("fetched {} from `{name}` ({})", a.hash, a.rel);
            fetched += 1;
        }
    }
    println!();
    println!(
        "{total} {}; {fetched} fetched; {held} already cached; {absent} unavailable",
        scope(only)
    );
    if absent > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn status(remote: bool, only: Option<&str>) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let cache = cache()?;
    let vaults = configured(&root)?;
    if remote && vaults.is_empty() {
        bail!("`--remote` needs a vault, and this repository declares none.");
    }
    let planned = plan(&root, &vaults, None, only)?;
    if planned.is_empty() {
        println!("The corpus names no artifacts.");
        return Ok(());
    }
    let total = planned.len();
    let (by_vault, aside) = group(planned);

    let mut mismatched = 0usize;
    // Cache first, and re-hashed rather than trusted: the local answer is the one that can be
    // wrong in a way nothing else would notice.
    let mut local = |a: &vault::Named| -> Result<&'static str> {
        Ok(match cache.verify(&a.hash)? {
            Verdict::Intact => "cached",
            Verdict::Absent => "-",
            Verdict::Corrupt { .. } => {
                mismatched += 1;
                "CORRUPT"
            }
        })
    };

    for (name, artifacts) in &by_vault {
        let cfg = vaults.get(name).expect("routed to a declared vault");
        println!("{}", heading(name, cfg));
        let store = match remote {
            true => Some(vault::open(name, cfg)?),
            false => None,
        };
        for a in artifacts {
            let here = local(a)?;
            let there = match &store {
                None => String::new(),
                Some(s) => match s.has(&a.hash)? {
                    true => "  stored".to_string(),
                    false => "  absent".to_string(),
                },
            };
            println!("  {here:<8}{there:<10}  {}  {}", a.hash, a.rel);
        }
        println!();
    }

    if !aside.is_empty() {
        let mut by_heading: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
        for (heading, line) in &aside {
            by_heading.entry(heading).or_default().push(line);
        }
        for (heading, lines) in by_heading {
            println!("{heading}");
            for l in lines {
                println!("  {l}");
            }
            println!();
        }
    }

    println!(
        "{total} artifact{} {}",
        if total == 1 { "" } else { "s" },
        scope(only)
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

/// The vaults this repository declares, checked for coherence.
///
/// Empty is the common case and is not an error: it is every corpus until somebody configures
/// a store.
fn configured(root: &Path) -> Result<Vaults> {
    let config = load_yidam_config(root)?;
    vault::resolve(&config.vault)
}

fn list() -> Result<()> {
    let cache = cache()?;
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let vaults = configured(&root)?;
    if vaults.is_empty() {
        println!("No vault configured.");
        println!();
        println!(
            "  Artifacts are kept in the local cache at {} and go nowhere else.",
            cache.root().display()
        );
        println!("  Declare a store in `.yidam/config.toml` to keep them somewhere durable:");
        println!();
        println!("    [vault.{}]", vault::DEFAULT_VAULT);
        println!("    url      = \"file:///mnt/archive/yidam\"");
        println!("    audience = \"Who can read this store, and why that is acceptable.\"");
        println!();
        println!(
            "  A second vault declares `holds` — one of {} — and so does the first.",
            vault::ARTIFACT_KINDS.join(", ")
        );
        return Ok(());
    }

    // What actually routes where, rather than what each vault claims. The two differ wherever
    // a record names a vault itself, and the claim alone would not show that.
    let routed = plan(&root, &vaults, None, None).unwrap_or_default();
    for (name, cfg) in vaults.iter() {
        println!("{name}");
        println!("  url       {}", cfg.url);
        println!("  audience  {}", cfg.audience());
        println!("  holds     {}", cfg.holds_display());
        let n = routed
            .iter()
            .filter(|r| matches!(&r.dest, Dest::Vault(v) if v == name))
            .count();
        println!(
            "  routed    {n} artifact{} the corpus names",
            if n == 1 { "" } else { "s" }
        );
        // Whether the store can actually be opened is worth knowing here, because the
        // alternative is learning it from the first `get` that needed it. The path is already
        // on the line above, so say only whether it worked — and when it did not, say why, at
        // the width the rest of the block uses.
        match vault::open(name, cfg) {
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
    }
    println!("  cache     {}", cache.root().display());
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

/// Fetch one artifact by content address.
///
/// **The record is the authority on where it lives.** If the corpus names this digest, its
/// route decides which vault is asked, and no other vault is contacted — asking `sources` for
/// a digest routed to `default` would resolve credentials for a store that was never meant to
/// have these bytes.
///
/// A digest the corpus does not name has no route, and then every declared vault is asked in
/// name order. That is the honest reading of a bare digest — *fetch this from wherever this
/// repository can reach* — and it is safe because asking is a `HEAD`: nothing is uploaded, and
/// a vault that cannot be opened is reported rather than fatal, so one unreachable store does
/// not hide an artifact sitting in another.
fn get(sha256: &str, out: Option<&Path>) -> Result<()> {
    let hash = ContentHash::parse(sha256)?;
    let cache = cache()?;

    if !cache.contains(&hash) {
        let root = repo_root()?;
        require_yidam_repo(&root)?;
        let vaults = configured(&root)?;
        if vaults.is_empty() {
            bail!(
                "{hash} is not in the local cache, and this repository declares no vault to \
                 fetch it from.\n  \
                 Declare one in `.yidam/config.toml` — `yidam vault list` shows the shape."
            );
        }
        let named = vault::named_artifacts(&root)
            .into_iter()
            .find(|a| a.hash == hash);
        let candidates: Vec<String> = match &named {
            Some(a) => match vaults.route(&a.kind, a.vault.as_deref()) {
                Route::To(name, _) => vec![name.to_string()],
                Route::Local => bail!(
                    "{hash} is recorded as `vault: none` by {} — the local cache and nowhere \
                     else — and it is not cached here.\n  \
                     Nothing will fetch it. Whoever has these bytes has to hand them over.",
                    a.rel
                ),
                Route::Unroutable(why) => bail!("{hash} has no route: {why}"),
            },
            None => vaults.names().iter().map(|n| n.to_string()).collect(),
        };

        let mut tried = Vec::new();
        let mut found = None;
        for name in &candidates {
            let cfg = vaults.get(name).expect("candidates come from the map");
            let store = match vault::open(name, cfg) {
                Ok(s) => s,
                Err(e) => {
                    tried.push(format!("`{name}` could not be opened: {}", first_line(&e)));
                    continue;
                }
            };
            match store.has(&hash) {
                Ok(true) => {
                    found = Some((name.clone(), store));
                    break;
                }
                Ok(false) => {
                    tried.push(format!("`{name}` ({}) does not hold it", store.describe()))
                }
                Err(e) => tried.push(format!("`{name}` could not be asked: {}", first_line(&e))),
            }
        }
        let Some((name, store)) = found else {
            bail!(
                "{hash} is in neither the local cache nor any vault that was asked.\n  {}",
                tried.join("\n  ")
            );
        };

        // Into the cache through the cache's own atomic write, then verified before it is
        // allowed to count as present. A store that hands back the wrong bytes under a name
        // is exactly what content addressing exists to catch, and catching it after the fact
        // would mean a corrupt artifact had already been readable.
        let staged = cache.path_of(&hash).with_extension("incoming");
        store.get(&hash, &staged)?;
        let got = ContentHash::of_file(&staged)?;
        if got != hash {
            let _ = std::fs::remove_file(&staged);
            bail!(
                "vault `{name}` returned bytes that are not {hash}.\n  \
                 asked for {hash}\n  \
                 received {got}\n  \
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

/// The first line of an error, for a message that has its own structure.
fn first_line(e: &anyhow::Error) -> String {
    e.to_string().lines().next().unwrap_or_default().to_string()
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
    match std::fs::metadata(path) {
        Ok(m) => human_size(m.len()),
        Err(_) => "unknown size".to_string(),
    }
}

// ── the artifacts this repository computes ───────────────────────────────────
//
// `push` and `pull` without a derived flag do the catalog and nothing else. With one, they
// do *only* what was named. An either/or rather than an addition, because an index is
// hundreds of megabytes and `--index` quietly also uploading a corpus of PDFs would be a
// surprise in the direction nobody wants.

/// The derived artifacts a pair of flags asked for, in a fixed order.
fn chosen(index: bool, embeddings: bool, bundle: bool) -> Vec<vault::Derived> {
    [
        (index, vault::Derived::Index),
        (embeddings, vault::Derived::Embeddings),
        (bundle, vault::Derived::Bundle),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, d)| d)
    .collect()
}

fn push_derived(picked: &[vault::Derived], dry_run: bool, only: Option<&str>) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let vaults = configured(&root)?;
    if vaults.is_empty() {
        bail!("this repository declares no vault — `yidam vault list` shows the shape.");
    }
    if let Some(name) = only {
        if vaults.get(name).is_none() {
            bail!(
                "`--vault {name}` names no declared vault ({}).",
                describe_declared(&vaults)
            );
        }
    }
    let cache = cache()?;
    let private = vault::read_private_paths(&root)?;
    let mut lock = vault::load_lock(&root)?;
    let mut changed = false;

    for &d in picked {
        let (name, cfg) = match vaults.route(d.kind(), None) {
            Route::To(n, c) => (n, c),
            Route::Local => {
                eprintln!("{}: routed to the local cache; nothing to send", d.kind());
                continue;
            }
            Route::Unroutable(why) => {
                eprintln!("{}: {why}", d.kind());
                continue;
            }
        };
        if only.is_some_and(|o| o != name) {
            continue;
        }

        // The guard, before anything is packed. A refusal here is about what the artifact
        // *encodes*, not about where it is going, so it costs nothing to ask first and
        // avoids compressing a few hundred megabytes that were never going to be sent.
        if let vault::Disposition::Refused(why) = vault::derived_may_push(&root, d, &private) {
            println!("refused: {why}");
            continue;
        }

        // Packed into the cache, hashed, and pushed from there — the same path a catalog
        // artifact takes, so `yidam vault verify` covers an index too.
        let staging = cache
            .root()
            .join(format!(".{}.{}.packing", d.kind(), std::process::id()));
        vault::pack_to(&root, d, &staging)?;
        let (hash, bytes) = vault::hash_file(&staging)?;
        cache.put_file(&staging, &hash)?;
        let _ = std::fs::remove_file(&staging);

        let recorded = lock.get(d).filter(|e| e.sha256 == hash.as_str()).is_some();
        if dry_run {
            println!(
                "would send {} {hash} ({}) to `{name}`{}",
                d.kind(),
                human_size(bytes),
                if recorded {
                    " — already recorded"
                } else {
                    ""
                }
            );
            continue;
        }

        let store = vault::open(name, cfg)?;
        if store.has(&hash)? {
            println!("{} {hash} already in `{name}`", d.kind());
        } else {
            store.put(&hash, &cache.path_of(&hash))?;
            println!(
                "sent {} {hash} ({}) to `{name}`",
                d.kind(),
                human_size(bytes)
            );
        }
        let entry = vault::Entry {
            sha256: hash.as_str().to_string(),
            bytes,
            vault: name.to_string(),
        };
        if lock.get(d) != Some(&entry) {
            lock.set(d, entry);
            changed = true;
        }
    }

    if changed && !dry_run {
        vault::save_lock(&root, &lock)?;
        println!();
        println!(
            "{} updated — commit it, or nothing else can find these bytes.",
            vault::lock_path(&root)
                .strip_prefix(&root)
                .unwrap_or(&vault::lock_path(&root))
                .display()
        );
    }
    Ok(())
}

fn pull_derived(picked: &[vault::Derived], only: Option<&str>) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let vaults = configured(&root)?;
    let cache = cache()?;
    let lock = vault::load_lock(&root)?;
    let mut missing = 0usize;

    for &d in picked {
        let Some(entry) = lock.get(d) else {
            eprintln!(
                "{} names no {} — `yidam vault push --{}` on a machine that has one \
                 records it.",
                vault::lock_path(&root).display(),
                d.kind(),
                d.kind()
            );
            missing += 1;
            continue;
        };
        // **The lock decides where to look, not the routing.** A `holds` edit made after the
        // push would otherwise send this to the wrong store, which is a mutable ref wearing a
        // lock file's clothes.
        let Some(cfg) = vaults.get(&entry.vault) else {
            bail!(
                "{} records the {} in vault `{}`, which `.yidam/config.toml` does not declare \
                 ({}).\n  \
                 The lock names the store the bytes were actually put in; re-push if the \
                 store has genuinely moved.",
                vault::lock_path(&root).display(),
                d.kind(),
                entry.vault,
                describe_declared(&vaults)
            );
        };
        if only.is_some_and(|o| o != entry.vault) {
            continue;
        }
        let hash = ContentHash::parse(&entry.sha256)?;

        if !cache.contains(&hash) {
            let store = vault::open(&entry.vault, cfg)?;
            if !store.has(&hash)? {
                eprintln!(
                    "{} {hash} is not in `{}` ({})",
                    d.kind(),
                    entry.vault,
                    store.describe()
                );
                missing += 1;
                continue;
            }
            let staged = cache.path_of(&hash).with_extension("incoming");
            store.get(&hash, &staged)?;
            let found = ContentHash::of_file(&staged)?;
            if found != hash {
                let _ = std::fs::remove_file(&staged);
                bail!(
                    "vault `{}` returned bytes that are not {hash} for the {}.\n  \
                     received {found}\n  Nothing was cached and nothing was unpacked.",
                    entry.vault,
                    d.kind()
                );
            }
            cache.put_file(&staged, &hash)?;
            let _ = std::fs::remove_file(&staged);
        }

        // Verified before it is allowed anywhere near the working tree. A cached artifact
        // is re-hashed rather than trusted, because the local copy is the one that can be
        // wrong in a way nothing else would notice.
        if !matches!(cache.verify(&hash)?, Verdict::Intact) {
            bail!(
                "the cached {} does not hash to {hash}. Delete it and pull again.",
                d.kind()
            );
        }
        vault::unpack_from(&root, d, &cache.path_of(&hash))?;
        println!(
            "{} {hash} → {}",
            d.kind(),
            d.path(&root)
                .strip_prefix(&root)
                .unwrap_or(&d.path(&root))
                .display()
        );
    }

    if missing > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// A byte count a person can read, from a number rather than a path.
fn human_size(n: u64) -> String {
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
