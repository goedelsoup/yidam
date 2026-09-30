//! `cluster survey`, `cluster ask` and `cluster gather` — a gather as a cluster step (#1217).
//!
//! `yidam gather` is three phases in one process: plan who is asked and translate the query,
//! ask each peer from its own bytes, and settle every answer into one roll call. On a cluster
//! each phase is its own pod, and the split is the one RFC-0026 §7 draws for a step:
//!
//! - `survey` reads the pinned corpus and emits the plan — one [`Ask`] per peer, already in
//!   that peer's vocabulary. The correspondence file is read here and nowhere else.
//! - `ask` is handed one [`Ask`] and no corpus. It takes the peer's `.yiz` from the vault when
//!   the vault already has it (`Store::has` answers before any transfer), fetches it by its
//!   lock url otherwise, and answers from those bytes. It **never fails**: every way asking
//!   can go wrong is a `refused` record, so a peer that could not be asked is still in the
//!   roll call saying why.
//! - `gather` reads the pinned corpus again, re-plans it, and settles the askers' records
//!   against that plan. A planned peer with no record is `refused` — a pod that died before
//!   it could say so. What it builds is the same tree `yidam gather` builds over the same
//!   pins, committed as objects on the pin and shipped through the vault like a step's.
//!
//! None of the three has a `--remote`, and none mounts a git credential. The lander lands.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::{bundle, deliver, StepOutcome, StepOutput, VaultArgs, CONTRACT_VERSION, OUT_REF};
use crate::cmd::gather::{self as g, Ask, Asked, GatherReport, PeerReport};
use crate::cmd::propose::write::git;
use crate::cmd::run::exec::Scratch;
use crate::cmd::run::receipt;
use crate::report::Format;
use crate::vault::{ContentHash, Store};

/// What `survey` emits: the plan, and every peer that is not asked with its reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Survey {
    pub format_version: u32,
    pub gather: String,
    /// The pinned commit the plan was read from.
    pub input: String,
    /// The vault digest of the pin's bundle, which `gather` reads again.
    pub bundle: String,
    /// `asks.len()`, so a workflow's `when:` can skip the gather when nobody is asked.
    pub asking: usize,
    pub asks: Vec<Ask>,
    /// Peers decided without asking: refused, undeclared, missing or unaligned.
    pub peers: Vec<PeerReport>,
}

/// What `ask` emits: which peer, how it went, and the vault digest of the full record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskOutput {
    pub format_version: u32,
    pub gather: String,
    pub peer: String,
    pub outcome: g::Outcome,
    /// The vault digest of the [`Asked`] record `gather` reads.
    pub record: String,
    /// `vault` when the peer's bundle was already there, `fetched` when it came from its url,
    /// and absent when the ask was refused before a bundle was wanted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Vault,
    Fetched,
}

// ── survey ────────────────────────────────────────────────────────────────────

pub(super) fn survey(
    name: &str,
    digest: &str,
    vault: &VaultArgs,
    out: Option<&Path>,
    format: Format,
) -> Result<()> {
    let store = vault.open()?;
    let scratch = Scratch::new("survey")?;
    let (root, input) = checkout(store.as_ref(), digest, scratch.path())?;
    let record = survey_in(&root, name, &input, digest)?;
    deliver(&root, format, out, record, render_survey)
}

pub(crate) fn survey_in(root: &Path, name: &str, input: &str, digest: &str) -> Result<Survey> {
    let (peers, asks) = g::plan_at(root, name)?;
    Ok(Survey {
        format_version: CONTRACT_VERSION,
        gather: name.to_string(),
        input: input.to_string(),
        bundle: digest.to_string(),
        asking: asks.len(),
        asks,
        peers,
    })
}

fn render_survey(s: &Survey) -> String {
    let mut out = format!(
        "gather {}: asking {} peer(s) at {}\n",
        s.gather,
        s.asking,
        &s.input[..s.input.len().min(12)]
    );
    for a in &s.asks {
        let _ = writeln!(out, "  ask   {} @ {}: {}", a.peer, a.pin, a.query);
    }
    for p in &s.peers {
        let _ = writeln!(
            out,
            "  {:<9} {}: {}",
            p.outcome.as_str(),
            p.package,
            p.reason.as_deref().unwrap_or("")
        );
    }
    out
}

// ── ask ───────────────────────────────────────────────────────────────────────

/// How an asker gets a peer's bytes when the vault does not have them.
pub(crate) type Fetch<'a> = &'a dyn Fn(&str) -> Result<Vec<u8>>;

pub(super) fn ask(
    arg: &str,
    image: Option<&str>,
    vault: &VaultArgs,
    out: Option<&Path>,
    format: Format,
) -> Result<()> {
    let ask: Ask = read_json(arg, "ask")?;
    let store = vault.open()?;
    let scratch = Scratch::new("ask")?;
    let image_digest = image.and_then(receipt::image_digest);
    let record = ask_in(
        &ask,
        image_digest,
        store.as_ref(),
        scratch.path(),
        &fetch_url,
    )?;
    deliver(scratch.path(), format, out, record, render_ask)
}

#[cfg(feature = "tonpa")]
fn fetch_url(url: &str) -> Result<Vec<u8>> {
    crate::runtime::block_on(crate::cmd::tonpa::install::fetch_bytes(url))
}

#[cfg(not(feature = "tonpa"))]
fn fetch_url(url: &str) -> Result<Vec<u8>> {
    bail!("this binary was built without the `tonpa` feature and cannot fetch {url}")
}

/// Ask one peer in `scratch`, and put the record in the vault.
///
/// Errors only when the record cannot be *delivered* — the vault refused the put. Everything
/// about the peer is an outcome in the record.
pub(crate) fn ask_in(
    ask: &Ask,
    image_digest: Option<String>,
    store: &dyn Store,
    scratch: &Path,
    fetch: Fetch,
) -> Result<AskOutput> {
    let root = scratch.join("peers");
    let (asked, source) = match unpack(ask, store, &root, fetch) {
        Ok(source) => (g::answer(&root, ask, image_digest), Some(source)),
        Err(e) => (Asked::refused(ask, format!("{e:#}"), image_digest), None),
    };
    let file = scratch.join("asked.json");
    std::fs::write(&file, serde_json::to_vec_pretty(&asked)?)
        .with_context(|| format!("writing {}", file.display()))?;
    let record = bundle::put(store, &file)?;
    Ok(AskOutput {
        format_version: CONTRACT_VERSION,
        gather: ask.gather.clone(),
        peer: ask.peer.clone(),
        outcome: asked.report.outcome,
        record,
        source,
    })
}

/// Put the peer's locked `.yiz` under `root/.yidam/tonpa/<peer>/`, from the vault when it has
/// it and from its url when it does not — and then into the vault, so the next asker of this
/// pin transfers nothing.
fn unpack(ask: &Ask, store: &dyn Store, root: &Path, fetch: Fetch) -> Result<Source> {
    let Some(lock) = &ask.lock else {
        bail!("tonpa.lock has no entry for it, so there is no bundle an asker can fetch");
    };
    let hash = ContentHash::parse(&lock.sha256)
        .with_context(|| format!("tonpa.lock's sha256 for {} is not a digest", ask.peer))?;
    let dir = crate::paths::tonpa_dir(root).join(&ask.peer);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let file = dir.join("fetched.yiz");
    let has = store
        .has(&hash)
        .with_context(|| format!("asking {} for {}", store.describe(), lock.sha256))?;
    let source = if has {
        store
            .get(&hash, &file)
            .with_context(|| format!("fetching {} from {}", lock.sha256, store.describe()))?;
        Source::Vault
    } else {
        let bytes = fetch(&lock.url).with_context(|| format!("fetching {}", lock.url))?;
        std::fs::write(&file, &bytes).with_context(|| format!("writing {}", file.display()))?;
        Source::Fetched
    };
    let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
    let got = crate::deps::sha256_hex(&bytes);
    if got != lock.sha256 {
        bail!(
            "the bundle at {} hashes to {got}, and tonpa.lock pins {}",
            lock.url,
            lock.sha256
        );
    }
    if source == Source::Fetched {
        store
            .put(&hash, &file)
            .with_context(|| format!("putting {} in {}", lock.sha256, store.describe()))?;
    }
    crate::deps::extract_bundle(&bytes, &dir)?;
    std::fs::remove_file(&file).ok();
    Ok(source)
}

fn render_ask(a: &AskOutput) -> String {
    let from = match a.source {
        Some(Source::Vault) => " (bundle from the vault)",
        Some(Source::Fetched) => " (bundle fetched)",
        None => "",
    };
    format!(
        "gather {}: {} {}{from}\n  record {}\n",
        a.gather,
        a.peer,
        a.outcome.as_str(),
        a.record
    )
}

// ── gather ────────────────────────────────────────────────────────────────────

pub(super) fn gather(
    name: &str,
    digest: &str,
    asked: &str,
    vault: &VaultArgs,
    out: Option<&Path>,
    format: Format,
) -> Result<()> {
    let store = vault.open()?;
    let scratch = Scratch::new("gather")?;
    let (root, input) = checkout(store.as_ref(), digest, scratch.path())?;
    let outputs = read_asked(asked)?;
    let (record, _) = gather_in(
        &root,
        name,
        &input,
        &outputs,
        store.as_ref(),
        scratch.path(),
    )?;
    deliver(&root, format, out, record, super::step::render)
}

/// Settle the askers' records at `input` and ship what they build. The report is returned too,
/// for a caller that wants the roll call and not only the step record.
pub(crate) fn gather_in(
    root: &Path,
    name: &str,
    input: &str,
    outputs: &[AskOutput],
    store: &dyn Store,
    scratch: &Path,
) -> Result<(StepOutput, GatherReport)> {
    let mut records = BTreeMap::new();
    for o in outputs {
        if o.format_version != CONTRACT_VERSION || o.gather != name {
            continue;
        }
        // A record the vault cannot give back is a peer with no record, and `settle` says so.
        if let Ok(a) = fetch_record(store, &o.record, scratch) {
            records.insert(o.peer.clone(), a);
        }
    }
    let (report, tip) = g::gather_at(root, name, input, records)?;
    let step = format!("gather/{name}");
    let record = |outcome, sha: Option<String>, bundle: Option<String>| StepOutput {
        format_version: CONTRACT_VERSION,
        step: step.clone(),
        outcome,
        sha,
        class: "epistemic".to_string(),
        verb: "open".to_string(),
        receipt: format!(".yidam/runs/gather/{name}"),
        input: input.to_string(),
        bundle,
    };
    let Some(tip) = tip else {
        return Ok((record(StepOutcome::Unchanged, None, None), report));
    };
    git(root, None, &["update-ref", OUT_REF, &tip], None)?;
    let file = scratch.join("out.bundle");
    bundle::create(root, &file, &[&format!("{input}..{OUT_REF}")])?;
    let digest = bundle::put(store, &file)?;
    Ok((record(StepOutcome::Ran, Some(tip), Some(digest)), report))
}

fn fetch_record(store: &dyn Store, digest: &str, scratch: &Path) -> Result<Asked> {
    let file = scratch.join(format!("asked-{digest}.json"));
    bundle::fetch(store, digest, &file)?;
    let text = std::fs::read(&file)?;
    Ok(serde_json::from_slice(&text)?)
}

/// The askers' outputs, as Argo aggregates a fan-out: a JSON array whose elements are the
/// objects themselves or the JSON text of each. An element that parses as neither is a pod
/// that emitted nothing useful, and it is dropped — its peer settles as `refused`.
pub(crate) fn read_asked(arg: &str) -> Result<Vec<AskOutput>> {
    let text = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading the askers' records at {path}"))?,
        None => arg.to_string(),
    };
    let items: Vec<serde_json::Value> = serde_json::from_str(&text)
        .context("the askers' records are not a JSON array of what `ask` emits")?;
    Ok(items
        .into_iter()
        .filter_map(|v| match v {
            serde_json::Value::String(s) => serde_json::from_str(&s).ok(),
            other => serde_json::from_value(other).ok(),
        })
        .collect())
}

// ── shared ────────────────────────────────────────────────────────────────────

/// The pin's bundle, cloned into `scratch/corpus`: the root and its tip.
fn checkout(
    store: &dyn Store,
    digest: &str,
    scratch: &Path,
) -> Result<(std::path::PathBuf, String)> {
    let file = scratch.join("in.bundle");
    bundle::fetch(store, digest, &file)?;
    let (input, branch) = bundle::pinned_branch(scratch, &file)?;
    let root = scratch.join("corpus");
    bundle::clone(&file, &branch, &root)?;
    super::commits_as_the_pod(&root)?;
    Ok((root, input))
}

fn read_json<T: serde::de::DeserializeOwned>(arg: &str, what: &str) -> Result<T> {
    let text = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading the {what} at {path}"))?,
        None => arg.to_string(),
    };
    serde_json::from_str(&text)
        .with_context(|| format!("the {what} is not the JSON `survey` emits"))
}

#[cfg(test)]
mod tests;
