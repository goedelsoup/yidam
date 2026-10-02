//! `yidam catalog fetch` — follow a declared address, keep the bytes, record what was kept.
//!
//! The middle that #720 found missing. A catalog entry carries a dereferenceable address, the
//! linter checks its syntax, the vault content-addresses bytes, `ttl_days` says when it is
//! time and `refresh:` is in the commit vocabulary — and no code path connected the five.
//! This is that path: **address → bytes → cache → record → commit.**
//!
//! # Where a fetch stops
//!
//! At the record. This never authors a corpus node, and the boundary is not a scoping
//! convenience — it is RFC-0026's invariant, which the commit writer enforces mechanically:
//!
//! > A run authors **operational** commits directly. Every **epistemic** commit it produces
//! > goes to a proposal branch, and nothing merges itself.
//!
//! Turning a fetched payload into concepts is `establish:`, which is epistemic, which means a
//! person. #720 says why that line is the one most worth holding: a connector that wrote
//! nodes from JSON would be *"the single change most capable of destroying what makes this
//! corpus worth having — bijection, 2–10 sentences, a human deciding what concept a thing
//! is."*
//!
//! # And where it stops on the way out
//!
//! At the local cache. Uploading is `yidam vault push`, which already exists, already
//! consults `redistributable`, and already refuses to send what a corpus marked private. A
//! fetch that also pushed would be a second place that decision is made — and the one place
//! it would be made without a person having looked at the bytes.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::location::{self, Plan};
use super::record;
use super::superseded;
use super::transport::{self, Policy, Session};
use crate::cmd::operational::{Commit, Writer};
use crate::parse::{parse_frontmatter, ArtifactOrigin, CatalogArtifact, CatalogLocation};
use crate::paths::{repo_root, yidam_catalog_dir};
use crate::sources::resolve::{Enabled, WAYBACK};
use crate::vault::{Cache, ContentHash, Route};
use crate::walk::walk_md_files;

pub struct FetchOptions {
    /// Restrict to one entry, by file stem or `name:`. Absent means every entry.
    pub entry: Option<String>,
    /// Restrict to one location, by its index in the entry's own `location:` list.
    ///
    /// The same index `ArtifactOrigin::Location` records, so `--location 1` and `from: 1`
    /// name the same thing. An entry commonly lists a human-facing page and a machine
    /// endpoint; without this a fetch would take the page as well.
    pub location: Option<usize>,
    /// `--bind name=value`, filling a `url_template`'s slots.
    pub bind: Vec<(String, String)>,
    /// Resolve and report; fetch nothing, write nothing, commit nothing.
    pub dry_run: bool,
    /// Look each identifier whose pack says `archive = "wayback"` up in the Wayback Machine,
    /// and add the nearest capture to the entry as a `wayback:` location.
    ///
    /// A read of the availability API. It never asks the archive to capture anything.
    pub archive: bool,
    pub format: crate::report::Format,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Obtained {
    location: usize,
    declared: String,
    /// The URL or path actually followed, after binding. Equal to `declared` for a `url`.
    followed: String,
    sha256: String,
    bytes: u64,
    media_type: Option<String>,
    /// Whether the cache already held these bytes — a re-fetch that found no change.
    cached: bool,
    /// Where `vault push` would send it, as prose. Never a credential.
    route: String,
    /// `<pack>@<version>` that resolved an identifier location. Absent for every other kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pack: Option<String>,
}

/// A location whose publisher answered with something other than success (RFC-0048 §5).
///
/// A finding on the entry, and not retried. A 403 is the publisher's answer, and nothing here
/// steps around it.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Declined {
    location: usize,
    declared: String,
    /// The URL asked, with no credential in it.
    followed: String,
    status: u16,
    why: String,
}

/// A Wayback capture `--archive` looked up for one location.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Archived {
    location: usize,
    /// The address looked up.
    followed: String,
    /// The capture's date, `YYYY-MM-DD`. Absent when the archive holds no successful capture.
    captured: Option<String>,
    /// The `wayback:` location added to the entry. Absent with no capture, and on a dry run.
    added: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Skipped {
    location: usize,
    declared: String,
    why: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct EntryOutcome {
    pub(crate) entry: String,
    obtained: Vec<Obtained>,
    skipped: Vec<Skipped>,
    /// Locations the publisher refused. Any one fails the run.
    refused: Vec<Declined>,
    /// What `--archive` found. Empty without it.
    archived: Vec<Archived>,
    /// Absent when nothing changed, which is the ordinary result of a re-run.
    pub(crate) commit: Option<Commit>,
    /// Nodes citing this entry that were read against an earlier version of it, when this run
    /// recorded a new one (#1200). Repo-relative and sorted; empty on every other run.
    ///
    /// Every node citing the entry, and not a subset, because the version did not exist until
    /// this run: nothing can have been written against it yet. `yidam due` keeps the list after
    /// this report is gone, and drops a node once a commit touches it.
    superseded: Vec<String>,
}

/// `fetched`, and deliberately not `entries`.
///
/// The report contract already declares `entries` — as `log`'s commits, with a required
/// `hash`/`verb`/`subject` shape — and a second meaning under one key is how a consumer comes
/// to decode one report as another. `report.schema.json` refused this before it shipped,
/// which is the gate doing exactly what it is for.
impl EntryOutcome {
    /// Whether a publisher refused any location of this entry.
    pub(crate) fn was_refused(&self) -> bool {
        !self.refused.is_empty()
    }

    fn of(entry: String, skipped: Vec<Skipped>) -> Self {
        Self {
            entry,
            obtained: vec![],
            skipped,
            refused: vec![],
            archived: vec![],
            commit: None,
            superseded: vec![],
        }
    }
}

#[derive(serde::Serialize)]
struct FetchReport {
    fetched: Vec<EntryOutcome>,
}

/// Today as `YYYY-MM-DD`, for the `retrieved:` a record carries.
fn today_iso() -> String {
    let (y, m, d) = crate::dates::civil_from_days(crate::dates::today_days());
    format!("{y:04}-{m:02}-{d:02}")
}

/// The entries this run is about.
pub(super) fn select(catalog: &Path, filter: Option<&str>) -> Result<Vec<PathBuf>> {
    let all = walk_md_files(catalog);
    let Some(want) = filter else {
        // README.md is the REGEN target `catalog-audit` writes, not a source. Excluded here
        // rather than in `walk_md_files`, which several other readers depend on including it.
        return Ok(all
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|n| n != "README.md"))
            .collect());
    };
    let want = want.trim().trim_end_matches(".md");
    let hit: Vec<PathBuf> = all
        .into_iter()
        .filter(|p| {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            if stem == want {
                return true;
            }
            let text = std::fs::read_to_string(p).unwrap_or_default();
            parse_frontmatter(&text).name.as_deref() == Some(want)
        })
        .collect();
    if hit.is_empty() {
        anyhow::bail!(
            "no catalog entry named `{want}`. `yidam catalog-audit` lists what this corpus \
             holds."
        );
    }
    Ok(hit)
}

/// Stage bytes somewhere they can be hashed before they are given a name.
///
/// In the cache's own directory rather than a system temp: the file is about to be renamed
/// into place under its digest, and `rename` is atomic only within a filesystem. This is the
/// same reasoning `Cache::put_file` states about its `.part` files, and staging across a
/// filesystem boundary would silently undo it.
pub(super) fn staging(cache: &Cache, n: usize) -> PathBuf {
    cache
        .root()
        .join("staging")
        .join(format!("{}-{n}", std::process::id()))
}

/// Where one run's requests go and what they carry.
struct Network<'a> {
    session: Session,
    /// What a plain `url` or `url_template` sends: the contact, and nothing a pack declares.
    plain: Policy,
    vaults: &'a crate::vault::Vaults,
}

/// Follow one location and file the bytes under their digest, or the publisher's refusal.
fn obtain(
    plan: &Plan,
    index: usize,
    cache: &Cache,
    n: usize,
    net: &mut Network,
) -> Result<std::result::Result<Obtained, Declined>> {
    let staged = staging(cache, n);
    let answer = match plan {
        Plan::File { path, .. } => Ok(transport::read_local(path, &staged)?),
        Plan::Url { url, .. } => net.session.get(url, &net.plain, &staged)?,
        Plan::Identifier { url, policy, .. } => net.session.get(url, policy, &staged)?,
    };
    let fetched = match answer {
        Ok(f) => f,
        Err(r) => {
            return Ok(Err(Declined {
                location: index,
                declared: plan.declared().to_string(),
                followed: plan.followed(),
                status: r.status,
                why: if r.reason.is_empty() {
                    format!("HTTP {}", r.status)
                } else {
                    format!("HTTP {} {}", r.status, r.reason)
                },
            }))
        }
    };

    let hash =
        ContentHash::of_file(&staged).with_context(|| format!("hashing {}", staged.display()))?;
    let bytes = std::fs::metadata(&staged)
        .with_context(|| format!("sizing {}", staged.display()))?
        .len();
    let cached = cache.contains(&hash);
    cache.put_file(&staged, &hash)?;
    // Best-effort: a staging file left behind is noise, not corruption, and failing the fetch
    // over it would discard bytes already safely filed under their digest.
    let _ = std::fs::remove_file(&staged);

    // Routed but not recorded — see `artifact_for`. Reported so a person can see where
    // `vault push` would send it before they run it.
    let route = match net.vaults.route(crate::vault::CATALOG_KIND, None) {
        Route::To(name, cfg) => format!("{name} ({})", cfg.url),
        Route::Local => "the local cache only".to_string(),
        Route::Unroutable(why) => format!("nowhere yet — {why}"),
    };

    Ok(Ok(Obtained {
        location: index,
        declared: plan.declared().to_string(),
        followed: plan.followed(),
        sha256: hash.as_str().to_string(),
        bytes,
        media_type: fetched.media_type,
        cached,
        route,
        pack: pack_of(plan),
    }))
}

fn pack_of(plan: &Plan) -> Option<String> {
    match plan {
        Plan::Identifier { pack, .. } => Some(pack.clone()),
        _ => None,
    }
}

/// `20240102030405` as `2024-01-02`.
fn capture_date(ts: &str) -> String {
    match (ts.get(..4), ts.get(4..6), ts.get(6..8)) {
        (Some(y), Some(m), Some(d)) => format!("{y}-{m}-{d}"),
        _ => ts.to_string(),
    }
}

/// The location `--archive` adds for a capture of location `index`.
fn archived_location(index: usize, url: &str, ts: &str) -> CatalogLocation {
    CatalogLocation {
        kind: Some("identifier".into()),
        value: Some(format!("{WAYBACK}:{ts}/{url}")),
        description: Some(format!(
            "Location {index} as the Wayback Machine captured it on {}.",
            capture_date(ts)
        )),
    }
}

/// The record an entry already holds for this same location, if any — the one whose
/// decisions a new capture continues.
///
/// **The last such record wins, whatever it says.** The list is chronological, because
/// [`record::append_artifacts`] appends, so the last entry naming this origin is the most
/// recent statement about it. An operator who wrote `redistributable: true` for one edition
/// and `false` for the next has changed their answer, and a third edition must carry the
/// second one; a rule that reached for any `true` in the history would be a ratchet that
/// only ever opens.
///
/// **Keyed on the origin, and only the origin.** A licence and a routing override are facts
/// about the publisher at one address. An entry commonly lists a human-facing page beside a
/// machine endpoint, and carrying the endpoint's permissions onto the page would assert one
/// publisher's terms about another's bytes.
///
/// **A record with no `from:` is no origin, and matches nothing.** Every record
/// [`artifact_for`] writes carries one, so this only skips records written by hand or by a
/// bulk importer — for which the origin genuinely is not known, and guessing it would be
/// lending one publisher's permissions to whatever location a later fetch happens to follow.
/// Measured: a derived corpus holding 35 vault-imported entries and 19 records in one of them
/// has `from:` on none of them, and declares no `location:` either, so a fetch skips all 35
/// before this is reached.
///
/// A record that says nothing is still the most recent record, and nothing is what it
/// carries. Reaching past it to an older one that did speak would be guessing: a silent
/// record written by a person who no longer asserts the field and a silent record written by
/// a machine that never could are the same four lines in the file, and this cannot tell them
/// apart. Entries already carrying a machine-written silence are repaired by an operator
/// editing the latest record once; from there it carries.
fn prior_for<'a>(
    held: &'a [CatalogArtifact],
    origin: &ArtifactOrigin,
) -> Option<&'a CatalogArtifact> {
    held.iter()
        .filter(|a| a.from.as_ref() == Some(origin))
        .next_back()
}

/// The record that lands in the entry's frontmatter.
///
/// **`vault:` and `redistributable:` are never this command's answer — but they are not
/// dropped either.** On a first capture both are absent, for the reasons below. On a
/// re-capture of a location an entry already holds a record for, both are carried forward
/// from that record: they are the operator's own statement about that source, and this
/// command reproduces it rather than either inventing or discarding it.
///
/// `vault:` on a record is an override of what the config's `holds` already decides, and the
/// config is the place that decision belongs: a corpus reorganising its storage edits one
/// file, not every record it has ever written. Stamping the *current route* into each record
/// would freeze a routing answer at fetch time and make the config's own routing dead — and
/// carrying a prior record's override forward is not that. `vault: none` is a decision to
/// keep particular bytes in the local cache and nowhere else, which RFC-0023 relies on being
/// durable where it says `gc` must warn about *"the artifact recorded `vault: none` — the
/// local cache and nowhere else, by decision — for which the cache is the only copy there
/// will ever be."* Letting that lapse at the next edition would
/// route to a store the operator held the previous edition back from, which is the failing
/// direction that loses bytes to a third party rather than merely inconveniencing someone.
///
/// `redistributable:` is a licensing fact about the source — whether these bytes may leave
/// this machine at all — and nothing a fetch observes can establish it. Its own field note
/// says a route is edited casually and a licence is not something that edit may undo. A
/// machine writing a *default* here would be a machine asserting a licence, in a committed
/// file, on the strength of an HTTP 200. Carrying a prior record's value is a different act:
/// the assertion is still the operator's, about the same publisher, and #1074 is what its
/// absence cost — an entry whose earlier records were cleared for redistribution had every
/// re-capture silently refused by `vault push`, one licensing decision re-entered by hand per
/// edition. What a carried value is not is unreviewed: [`message`] names every field it
/// carried and which digest it came from, so the `refresh:` commit a person reads says that a
/// licence was continued and not established.
fn artifact_for(o: &Obtained, prior: Option<&CatalogArtifact>) -> CatalogArtifact {
    CatalogArtifact {
        sha256: Some(o.sha256.clone()),
        bytes: Some(o.bytes),
        media_type: o.media_type.clone(),
        retrieved: Some(today_iso()),
        from: Some(ArtifactOrigin::Location(o.location)),
        vault: prior.and_then(|p| p.vault.clone()),
        redistributable: prior.and_then(|p| p.redistributable),
        // A reading is of these bytes, and a prior record's were of others.
        text: None,
        readings: None,
    }
}

/// Which locations to follow, and why the others were passed over.
fn plan_locations(
    locations: &[CatalogLocation],
    ctx: &location::Context,
    opts: &FetchOptions,
) -> (Vec<(usize, Plan)>, Vec<Skipped>) {
    let mut plans = Vec::new();
    let mut skipped = Vec::new();
    for (i, loc) in locations.iter().enumerate() {
        if opts.location.is_some_and(|want| want != i) {
            continue;
        }
        match location::resolve(loc, ctx) {
            Ok(plan) => plans.push((i, plan)),
            Err(u) => {
                // A benign outcome is reported only when it was asked for by index. An entry
                // listing a records office beside a URL should fetch the URL and say nothing
                // about the office; an entry whose only address is the office should say so,
                // and it will, because `plans` comes back empty.
                if !u.is_benign() || opts.location.is_some() {
                    skipped.push(Skipped {
                        location: i,
                        declared: declared_of(loc),
                        why: u.message(),
                    });
                }
            }
        }
    }
    (plans, skipped)
}

fn declared_of(loc: &CatalogLocation) -> String {
    loc.value.clone().unwrap_or_default()
}

/// The subject line the run authors, and the body under it.
///
/// The body names every digest and where it came from, because the subject cannot and because
/// this is the record RFC-0026 asks a run to leave: *what ran, against what, producing which
/// bytes.* A commit saying only `refresh: usgs-nwis` is the thing that RFC opens by objecting
/// to — a person's account of what a tool did, checkable against nothing.
///
/// **And it names every field carried forward, with the digest it came from.** A licence in a
/// committed file is the one value here a person must be able to see arrive. [`artifact_for`]
/// reproduces the operator's own assertion rather than establishing one, and this line is what
/// makes that reviewable: the diff alone shows `redistributable: true` on a new record and
/// cannot show whose claim it is or which record it continues.
fn message(
    entry: &str,
    obtained: &[Obtained],
    held: &[CatalogArtifact],
    records: &[CatalogArtifact],
    archived: &[Archived],
) -> (String, String) {
    let added = archived.iter().filter(|a| a.added.is_some()).count();
    let subject = match obtained.len() {
        0 => format!("refresh: {entry} with {added} archived capture(s)"),
        1 => format!("refresh: {entry} from {}", obtained[0].declared),
        n => format!("refresh: {entry} from {n} of its locations"),
    };
    let mut body = String::new();
    for a in archived {
        use std::fmt::Write;
        if let (Some(v), Some(d)) = (&a.added, &a.captured) {
            let _ = writeln!(
                body,
                "location {} as the Wayback Machine captured it on {d} — {v}",
                a.location
            );
        }
    }
    for o in obtained {
        use std::fmt::Write;
        let _ = writeln!(
            body,
            "sha256:{} ({} bytes{}) from location {} — {}",
            o.sha256,
            o.bytes,
            o.media_type
                .as_deref()
                .map(|m| format!(", {m}"))
                .unwrap_or_default(),
            o.location,
            o.followed
        );
        // Matched by digest rather than by position: `append_artifacts` drops a record whose
        // digest the entry already holds, so `records` and `obtained` are not index-aligned
        // on a run that refreshed one location and found another unchanged.
        let Some(r) = records
            .iter()
            .find(|r| r.sha256.as_deref() == Some(&o.sha256))
        else {
            continue;
        };
        let origin = ArtifactOrigin::Location(o.location);
        let Some(p) = prior_for(held, &origin) else {
            continue;
        };
        let mut carried: Vec<String> = Vec::new();
        if let Some(v) = &r.vault {
            carried.push(format!("vault: {v}"));
        }
        if let Some(d) = r.redistributable {
            carried.push(format!("redistributable: {d}"));
        }
        if carried.is_empty() {
            continue;
        }
        let _ = writeln!(
            body,
            "  carried forward from sha256:{}: {}",
            p.sha256.as_deref().unwrap_or("(no digest)"),
            carried.join(", ")
        );
    }
    (subject, body)
}

pub fn fetch(opts: &FetchOptions) -> Result<()> {
    let root = repo_root()?;
    let out = fetch_in(&root, opts, &mut Writer::WorkingTree)?;
    // A refusal is a finding: recorded, not retried, and the run says so in its exit code.
    let passed = !out.iter().any(EntryOutcome::was_refused);
    crate::report::gate(
        &root,
        opts.format,
        FetchReport { fetched: out },
        passed,
        |r| print!("{}", render(&r.fetched, opts.dry_run)),
    )
}

/// [`fetch`] against `root`, committing through `writer` — the working tree on a laptop, a
/// detached index in a pod (RFC-0026, the 2026-09-30 amendment).
pub(crate) fn fetch_in(
    root: &Path,
    opts: &FetchOptions,
    writer: &mut Writer,
) -> Result<Vec<EntryOutcome>> {
    let root = root.to_path_buf();
    let catalog = yidam_catalog_dir(&root);
    let entries = select(&catalog, opts.entry.as_deref())?;
    let vaults = crate::vault::resolve(&crate::config::load_yidam_config(&root)?.vault)?;
    let cache = Cache::resolve(|k| std::env::var(k).ok())?;
    let env = |k: &str| std::env::var(k).ok();
    let contact = transport::contact(&env).map_err(anyhow::Error::msg)?;
    let packs = Enabled::load(&root)?;
    // The archive's own spacing and contact come from the pack that declares `wayback:`, so a
    // run with `--archive` and no such pack is refused before it asks anything.
    let archive_policy = if opts.archive {
        let Some((_, t)) = packs.transport_of(WAYBACK) else {
            anyhow::bail!(
                "--archive adds `{WAYBACK}:` locations, and no enabled source pack declares the \
                 `{WAYBACK}` scheme. Pin one in prelude_sources, or write one in {}",
                crate::sources::AUTHORED
            );
        };
        Some(Policy {
            contact: contact.clone(),
            credentials: vec![],
            min_interval: t
                .min_interval
                .as_deref()
                .and_then(crate::sources::manifest::parse_interval),
        })
    } else {
        None
    };
    let ctx = location::Context {
        root: &root,
        bindings: &opts.bind,
        packs: &packs,
        contact: contact.as_deref(),
        env: &env,
    };
    let mut net = Network {
        session: Session::new(),
        plain: Policy {
            contact: contact.clone(),
            ..Policy::default()
        },
        vaults: &vaults,
    };

    let mut out: Vec<EntryOutcome> = Vec::new();
    let mut n = 0usize;
    // Read once, and only if some entry records a new version: a corpus walk to answer a
    // question no entry in this run is asking would be the ordinary run's whole cost.
    let mut citing: Option<std::collections::HashMap<PathBuf, Vec<String>>> = None;
    for path in &entries {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let locations = parse_frontmatter(&text).location.unwrap_or_default();
        if locations.is_empty() {
            continue;
        }

        let (plans, skipped) = plan_locations(&locations, &ctx, opts);
        if plans.is_empty() {
            if !skipped.is_empty() {
                out.push(EntryOutcome::of(name, skipped));
            }
            continue;
        }

        // Checked before a byte moves, so the refusal names a person's uncommitted work
        // rather than the edit this command is about to make.
        if !opts.dry_run {
            writer.require_clean(&root, &[rel.clone()])?;
        }

        let mut obtained = Vec::new();
        let mut refused = Vec::new();
        let mut archived = Vec::new();
        let mut captures: Vec<CatalogLocation> = Vec::new();
        for (i, plan) in &plans {
            if opts.dry_run {
                obtained.push(Obtained {
                    location: *i,
                    declared: plan.declared().to_string(),
                    followed: plan.followed(),
                    sha256: String::new(),
                    bytes: 0,
                    media_type: None,
                    cached: false,
                    route: String::new(),
                    pack: pack_of(plan),
                });
                continue;
            }
            n += 1;
            match obtain(plan, *i, &cache, n, &mut net)? {
                Ok(o) => obtained.push(o),
                Err(d) => refused.push(d),
            }
            // Looked up whatever the publisher answered: a capture is most use where the
            // source now refuses.
            if let (
                Some(policy),
                Plan::Identifier {
                    url, archive: true, ..
                },
            ) = (&archive_policy, plan)
            {
                n += 1;
                let scratch = staging(&cache, n);
                match net
                    .session
                    .closest_capture(transport::AVAILABILITY, url, policy, &scratch)?
                {
                    Ok(capture) => {
                        let added = capture.as_deref().map(|ts| archived_location(*i, url, ts));
                        archived.push(Archived {
                            location: *i,
                            followed: url.clone(),
                            captured: capture.as_deref().map(capture_date),
                            added: added.as_ref().and_then(|l| l.value.clone()),
                        });
                        captures.extend(added);
                    }
                    Err(r) => refused.push(Declined {
                        location: *i,
                        declared: plan.declared().to_string(),
                        followed: transport::availability_url(transport::AVAILABILITY, url),
                        status: r.status,
                        why: format!(
                            "the Wayback availability API answered HTTP {} {}",
                            r.status, r.reason
                        ),
                    }),
                }
            }
        }

        let mut written = None;
        let mut owed: Vec<String> = Vec::new();
        if !opts.dry_run {
            // Read from `text` — the entry as it was before this run — so a carry consults
            // the operator's records and never one this same run appended.
            let held = parse_frontmatter(&text).artifacts.unwrap_or_default();
            let records: Vec<CatalogArtifact> = obtained
                .iter()
                .map(|o| artifact_for(o, prior_for(&held, &ArtifactOrigin::Location(o.location))))
                .collect();
            let updated = record::append_artifacts(&text, &records)
                .with_context(|| format!("recording what {rel} obtained"))?;
            let updated = record::append_locations(&updated, &captures)
                .with_context(|| format!("adding archived captures to {rel}"))?;
            if updated != text {
                if records.iter().any(|r| superseded::supersedes(&held, r)) {
                    owed = citing
                        .get_or_insert_with(|| superseded::citing_nodes(&root))
                        .get(&crate::corpus::normalize(path))
                        .cloned()
                        .unwrap_or_default();
                    owed.sort();
                }
                let (subject, mut body) = message(&name, &obtained, &held, &records, &archived);
                body.push_str(&owed_lines(&owed));
                written = writer.commit(
                    &root,
                    super::WHO,
                    &subject,
                    &body,
                    &[(rel.clone(), updated)],
                )?;
            }
        }

        out.push(EntryOutcome {
            entry: name,
            obtained,
            skipped,
            refused,
            archived,
            commit: written,
            superseded: owed,
        });
    }
    Ok(out)
}

/// The paragraph a `refresh:` commit carries when it records a new version of a cited source.
///
/// In the commit and not only in the report, because the commit is what outlives the run: a
/// person reading `git log` for why a node was re-read finds the list where the version
/// arrived. Empty when nothing is owed, so an ordinary refresh reads as it always did.
fn owed_lines(owed: &[String]) -> String {
    use std::fmt::Write as _;
    if owed.is_empty() {
        return String::new();
    }
    let mut s = format!(
        "\nA new version of a source {} node(s) cite, each read against an earlier one:\n",
        owed.len()
    );
    for n in owed {
        let _ = writeln!(s, "  - {n}");
    }
    s
}

/// Indent a message's continuation lines to sit under the line that introduced it.
///
/// `Route::Unroutable` and [`Unfollowable::message`] are both written for someone reading a
/// terminal, and both run to more than one line when they have a repair to suggest. Printed
/// raw under an indented heading their second line starts at column zero, which reads as a
/// new entry rather than as the rest of a sentence.
fn hang(message: &str, indent: &str) -> String {
    message
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                l.to_string()
            } else {
                format!("{indent}{}", l.trim_start())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render(entries: &[EntryOutcome], dry_run: bool) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    if entries.is_empty() {
        return "No catalog entry declares an address anything can follow.\n".to_string();
    }
    for e in entries {
        let _ = writeln!(s, "{}", e.entry);
        for o in &e.obtained {
            if dry_run {
                let _ = writeln!(s, "  would fetch location {} — {}", o.location, o.followed);
                continue;
            }
            let _ = writeln!(
                s,
                "  location {} — {}\n    sha256:{} ({} bytes{}){}\n    push route: {}",
                o.location,
                o.followed,
                o.sha256,
                o.bytes,
                o.media_type
                    .as_deref()
                    .map(|m| format!(", {m}"))
                    .unwrap_or_default(),
                if o.cached { " — already held" } else { "" },
                hang(&o.route, "      ")
            );
        }
        for k in &e.skipped {
            let _ = writeln!(
                s,
                "  location {} skipped — {}",
                k.location,
                hang(&k.why, "    ")
            );
        }
        for r in &e.refused {
            let _ = writeln!(
                s,
                "  location {} refused — {}: {}\n    recorded and not retried",
                r.location, r.followed, r.why
            );
        }
        for a in &e.archived {
            let _ = match (&a.captured, &a.added) {
                (Some(d), Some(v)) => {
                    writeln!(s, "  location {} captured {d} — added {v}", a.location)
                }
                _ => writeln!(
                    s,
                    "  location {} — the Wayback Machine holds no successful capture of {}",
                    a.location, a.followed
                ),
            };
        }
        match &e.commit {
            Some(c) => {
                let _ = writeln!(s, "  {} {}", c.sha, c.subject);
                if !e.superseded.is_empty() {
                    let _ = writeln!(
                        s,
                        "  a new version — {} node(s) cite an earlier one, and `yidam due` \
                         holds them until each is re-read:",
                        e.superseded.len()
                    );
                    for n in &e.superseded {
                        let _ = writeln!(s, "    {n}");
                    }
                }
            }
            None if !dry_run && !e.obtained.is_empty() => {
                let _ = writeln!(s, "  nothing new to record");
            }
            None => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(kind: &str, value: &str) -> CatalogLocation {
        CatalogLocation {
            kind: Some(kind.into()),
            value: Some(value.into()),
            description: None,
        }
    }

    fn opts() -> FetchOptions {
        FetchOptions {
            entry: None,
            location: None,
            bind: vec![],
            dry_run: false,
            archive: false,
            format: crate::report::Format::Text,
        }
    }

    /// Plan against `/repo` with no packs and an empty environment.
    fn plan(locs: &[CatalogLocation], o: &FetchOptions) -> (Vec<(usize, Plan)>, Vec<Skipped>) {
        let ctx = location::Context {
            root: Path::new("/repo"),
            bindings: &o.bind,
            packs: &Enabled::default(),
            contact: None,
            env: &|_| None,
        };
        plan_locations(locs, &ctx, o)
    }

    /// The streamflow entry's shape: a human-facing page and a machine endpoint. Without a
    /// binding the template is skipped and reported; the plain URL is still followed.
    #[test]
    fn an_unbindable_template_is_reported_and_its_sibling_is_still_planned() {
        let locs = vec![
            loc("url", "https://waterdata.usgs.gov/nwis"),
            loc("url_template", "https://x/?sites={site}"),
        ];
        let (plans, skipped) = plan(&locs, &opts());
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].0, 0);
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].why.contains("--bind site="), "{skipped:?}");
    }

    /// An address is passed over in silence when it sits beside something followable.
    #[test]
    fn an_address_beside_a_url_is_not_reported_as_a_problem() {
        let locs = vec![
            loc("address", "County Recorder, 14 Court St"),
            loc("url", "https://x/y"),
        ];
        let (plans, skipped) = plan(&locs, &opts());
        assert_eq!(plans.len(), 1);
        assert!(skipped.is_empty(), "{skipped:?}");
    }

    /// But an entry whose only address is a physical one produces no plans, which is how the
    /// caller knows to say so.
    #[test]
    fn an_entry_with_only_an_address_plans_nothing() {
        let locs = vec![loc("address", "County Recorder, 14 Court St")];
        let (plans, skipped) = plan(&locs, &opts());
        assert!(plans.is_empty());
        assert!(skipped.is_empty());
    }

    /// Asking for one location by index means being told why that one cannot be followed,
    /// even when the reason is benign — a silent no-op to an explicit request is worse than a
    /// refusal.
    #[test]
    fn naming_a_benign_location_explicitly_reports_it() {
        let locs = vec![loc("address", "County Recorder")];
        let mut o = opts();
        o.location = Some(0);
        let (plans, skipped) = plan(&locs, &o);
        assert!(plans.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].why.contains("place rather than an endpoint"));
    }

    #[test]
    fn a_location_index_restricts_the_run_to_that_one() {
        let locs = vec![loc("url", "https://a/"), loc("url", "https://b/")];
        let mut o = opts();
        o.location = Some(1);
        let (plans, _) = plan(&locs, &o);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].0, 1);
    }

    fn obtained(sha: &str, at: usize) -> Obtained {
        Obtained {
            location: at,
            declared: "d".into(),
            followed: "f".into(),
            sha256: sha.into(),
            bytes: 12,
            media_type: Some("application/json".into()),
            cached: false,
            route: "sources (s3://x)".into(),
            pack: None,
        }
    }

    /// A record carrying a licence and a route, as an operator would have written it.
    fn decided(sha: &str, at: usize, vault: Option<&str>, r: Option<bool>) -> CatalogArtifact {
        CatalogArtifact {
            sha256: Some(sha.into()),
            bytes: Some(1),
            media_type: None,
            retrieved: Some("2026-01-01".into()),
            from: Some(ArtifactOrigin::Location(at)),
            vault: vault.map(str::to_string),
            redistributable: r,
            text: None,
            readings: None,
        }
    }

    /// The record is what a person reviews in the `refresh:` commit, so what it omits matters
    /// as much as what it carries. On a first capture it omits both decisions.
    #[test]
    fn a_first_capture_asserts_no_licence_and_freezes_no_route() {
        let a = artifact_for(&obtained("aa", 0), None);
        assert_eq!(a.sha256.as_deref(), Some("aa"));
        assert_eq!(a.bytes, Some(12));
        assert!(matches!(a.from, Some(ArtifactOrigin::Location(0))));
        assert!(
            a.redistributable.is_none(),
            "a licence is not something an HTTP 200 establishes"
        );
        assert!(
            a.vault.is_none(),
            "routing is the config's decision, not a value frozen per record"
        );
    }

    /// #1074. An operator cleared a source for redistribution and held its bytes local; the
    /// next edition of the same source keeps both, or they re-enter them once per edition.
    #[test]
    fn a_re_capture_carries_the_decisions_the_entry_already_made() {
        let held = vec![decided("aa", 0, Some("none"), Some(true))];
        let a = artifact_for(
            &obtained("bb", 0),
            prior_for(&held, &ArtifactOrigin::Location(0)),
        );
        assert_eq!(a.redistributable, Some(true));
        assert_eq!(a.vault.as_deref(), Some("none"));
        assert_eq!(
            a.sha256.as_deref(),
            Some("bb"),
            "the new bytes, not the old"
        );
    }

    /// `false` is carried exactly as `true` is. A rule that forwarded only permission would be
    /// a ratchet that never closes, and `redistributable: false` is the value that has to hold.
    #[test]
    fn a_refusal_is_carried_as_readily_as_a_permission() {
        let held = vec![decided("aa", 0, None, Some(false))];
        let a = artifact_for(
            &obtained("bb", 0),
            prior_for(&held, &ArtifactOrigin::Location(0)),
        );
        assert_eq!(a.redistributable, Some(false));
    }

    /// The operator's latest answer, not their first. A publisher that tightened its terms
    /// between editions was recorded once, and every edition after it inherits that.
    #[test]
    fn the_most_recent_record_for_the_location_is_the_one_carried() {
        let held = vec![
            decided("aa", 0, Some("sources"), Some(true)),
            decided("bb", 0, Some("none"), Some(false)),
        ];
        let a = artifact_for(
            &obtained("cc", 0),
            prior_for(&held, &ArtifactOrigin::Location(0)),
        );
        assert_eq!(a.redistributable, Some(false));
        assert_eq!(a.vault.as_deref(), Some("none"));
    }

    /// A permission belongs to one publisher at one address. An entry listing a cleared
    /// endpoint beside an uncleared page must not lend the first's licence to the second.
    #[test]
    fn a_decision_does_not_cross_between_locations() {
        let held = vec![decided("aa", 0, Some("none"), Some(true))];
        assert!(prior_for(&held, &ArtifactOrigin::Location(1)).is_none());
        let a = artifact_for(
            &obtained("bb", 1),
            prior_for(&held, &ArtifactOrigin::Location(1)),
        );
        assert!(a.redistributable.is_none(), "location 1 was never cleared");
        assert!(a.vault.is_none());
    }

    /// A record naming a literal URL is a different origin from the index that happens to
    /// resolve to it, because nothing here can tell whether it still does.
    #[test]
    fn a_url_origin_is_not_the_same_source_as_a_location_index() {
        let mut held = vec![decided("aa", 0, None, Some(true))];
        held[0].from = Some(ArtifactOrigin::Url("https://x/y".into()));
        assert!(prior_for(&held, &ArtifactOrigin::Location(0)).is_none());
    }

    /// A silent record is still the most recent one. Reaching past it to an older record that
    /// did speak would be this command guessing which silences were a person's.
    #[test]
    fn a_silent_latest_record_carries_nothing_forward() {
        let held = vec![
            decided("aa", 0, Some("none"), Some(true)),
            decided("bb", 0, None, None),
        ];
        let a = artifact_for(
            &obtained("cc", 0),
            prior_for(&held, &ArtifactOrigin::Location(0)),
        );
        assert!(a.redistributable.is_none());
        assert!(a.vault.is_none());
    }

    /// A two-line repair suggestion has to read as one message, not as two entries.
    #[test]
    fn a_multi_line_message_hangs_under_its_first_line() {
        assert_eq!(
            hang(
                "no vault holds `catalog`.\n  Add it to a vault's `holds`",
                "    "
            ),
            "no vault holds `catalog`.\n    Add it to a vault's `holds`"
        );
        assert_eq!(hang("one line", "    "), "one line");
    }

    fn at_location(sha: &str, at: usize) -> Obtained {
        Obtained {
            location: at,
            declared: format!("https://x/{at}"),
            followed: format!("https://x/{at}"),
            sha256: sha.into(),
            bytes: 3,
            media_type: None,
            cached: false,
            route: String::new(),
            pack: None,
        }
    }

    #[test]
    fn the_subject_names_the_entry_and_the_body_names_every_digest() {
        let o = at_location;
        let (subject, body) = message("usgs-nwis", &[o("aa", 0)], &[], &[], &[]);
        assert_eq!(subject, "refresh: usgs-nwis from https://x/0");
        assert!(body.contains("sha256:aa (3 bytes) from location 0"));

        let (subject, body) = message("usgs-nwis", &[o("aa", 0), o("bb", 1)], &[], &[], &[]);
        assert_eq!(subject, "refresh: usgs-nwis from 2 of its locations");
        assert!(body.contains("sha256:aa") && body.contains("sha256:bb"));
    }

    /// A carried licence is reviewable only if the commit says it was carried and from where.
    /// The diff shows `redistributable: true` on a new record and cannot show whose claim it is.
    #[test]
    fn the_body_names_what_it_carried_and_which_record_it_came_from() {
        let held = vec![decided("aa", 0, Some("none"), Some(true))];
        let o = at_location("bb", 0);
        let records = vec![artifact_for(
            &o,
            prior_for(&held, &ArtifactOrigin::Location(0)),
        )];
        let (_, body) = message("usgs-nwis", &[o], &held, &records, &[]);
        assert!(
            body.contains("carried forward from sha256:aa: vault: none, redistributable: true"),
            "{body}"
        );
    }

    /// A first capture has nothing to disclose, and a line saying so would make every
    /// ordinary `refresh:` commit longer for no reader.
    #[test]
    fn a_first_capture_adds_no_carry_line() {
        let o = at_location("bb", 0);
        let records = vec![artifact_for(&o, None)];
        let (_, body) = message("usgs-nwis", &[o], &[], &records, &[]);
        assert!(!body.contains("carried forward"), "{body}");
    }

    /// A prior record that decided nothing is not a carry. The line is about a decision
    /// travelling, not about a previous record existing.
    #[test]
    fn a_prior_record_with_no_decisions_adds_no_carry_line() {
        let held = vec![decided("aa", 0, None, None)];
        let o = at_location("bb", 0);
        let records = vec![artifact_for(
            &o,
            prior_for(&held, &ArtifactOrigin::Location(0)),
        )];
        let (_, body) = message("usgs-nwis", &[o], &held, &records, &[]);
        assert!(!body.contains("carried forward"), "{body}");
    }

    /// `append_artifacts` drops a record whose digest the entry already holds, so a run that
    /// refreshed location 1 and found location 0 unchanged hands `message` a `records` list
    /// shorter than `obtained`. Matched by digest, the carry line still lands on the right one.
    #[test]
    fn a_carry_line_follows_the_digest_and_not_the_position() {
        let held = vec![
            decided("aa", 0, None, Some(true)),
            decided("cc", 1, None, Some(false)),
        ];
        let unchanged = at_location("aa", 0);
        let fresh = at_location("dd", 1);
        // Only location 1 produced a new record; location 0's digest was already held.
        let records = vec![artifact_for(
            &fresh,
            prior_for(&held, &ArtifactOrigin::Location(1)),
        )];
        let (_, body) = message("usgs-nwis", &[unchanged, fresh], &held, &records, &[]);
        assert!(
            body.contains("carried forward from sha256:cc: redistributable: false"),
            "{body}"
        );
        assert_eq!(body.matches("carried forward").count(), 1, "{body}");
    }

    /// The subject has to survive the invariant check the commit writer applies, or the
    /// command would fail at the last step of a fetch that already moved bytes.
    #[test]
    fn every_subject_this_writes_is_operational() {
        let o = Obtained {
            location: 0,
            declared: "d".into(),
            followed: "f".into(),
            sha256: "aa".into(),
            bytes: 1,
            media_type: None,
            cached: false,
            route: String::new(),
            pack: None,
        };
        for obtained in [vec![o.clone()], vec![o.clone(), o.clone()]] {
            let (subject, _) = message("e", &obtained, &[], &[], &[]);
            assert_eq!(
                yidam_core::git::classify_commit("", &subject).kind,
                yidam_core::git::CommitKind::Operational,
                "{subject}"
            );
        }
    }
}
