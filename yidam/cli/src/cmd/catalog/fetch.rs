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

use super::commit::{self, Commit};
use super::location::{self, Plan};
use super::record;
use super::transport;
use crate::parse::{parse_frontmatter, ArtifactOrigin, CatalogArtifact, CatalogLocation};
use crate::paths::{repo_root, yidam_catalog_dir};
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
    pub format: crate::report::Format,
}

#[derive(Debug, Clone, serde::Serialize)]
struct Obtained {
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
}

#[derive(Debug, Clone, serde::Serialize)]
struct Skipped {
    location: usize,
    declared: String,
    why: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EntryOutcome {
    entry: String,
    obtained: Vec<Obtained>,
    skipped: Vec<Skipped>,
    /// Absent when nothing changed, which is the ordinary result of a re-run.
    commit: Option<Commit>,
}

/// `fetched`, and deliberately not `entries`.
///
/// The report contract already declares `entries` — as `log`'s commits, with a required
/// `hash`/`verb`/`subject` shape — and a second meaning under one key is how a consumer comes
/// to decode one report as another. `report.schema.json` refused this before it shipped,
/// which is the gate doing exactly what it is for.
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
fn select(catalog: &Path, filter: Option<&str>) -> Result<Vec<PathBuf>> {
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
fn staging(cache: &Cache, n: usize) -> PathBuf {
    cache
        .root()
        .join("staging")
        .join(format!("{}-{n}", std::process::id()))
}

/// Follow one location and file the bytes under their digest.
fn obtain(
    plan: &Plan,
    index: usize,
    cache: &Cache,
    n: usize,
    vaults: &crate::vault::Vaults,
) -> Result<Obtained> {
    let staged = staging(cache, n);
    let fetched = match plan {
        Plan::File { path, .. } => transport::read_local(path, &staged),
        Plan::Url { url, .. } => transport::get(url, &staged),
    }?;

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

    let (declared, followed) = match plan {
        Plan::File { path, declared } => (declared.clone(), path.display().to_string()),
        Plan::Url { url, declared } => (declared.clone(), url.clone()),
    };
    // Routed but not recorded — see `artifact_for`. Reported so a person can see where
    // `vault push` would send it before they run it.
    let route = match vaults.route(crate::vault::CATALOG_KIND, None) {
        Route::To(name, cfg) => format!("{name} ({})", cfg.url),
        Route::Local => "the local cache only".to_string(),
        Route::Unroutable(why) => format!("nowhere yet — {why}"),
    };

    Ok(Obtained {
        location: index,
        declared,
        followed,
        sha256: hash.as_str().to_string(),
        bytes,
        media_type: fetched.media_type,
        cached,
        route,
    })
}

/// The record that lands in the entry's frontmatter.
///
/// **`vault:` is deliberately absent, and `redistributable:` too.**
///
/// `vault:` on a record is an override of what the config's `holds` already decides, and the
/// config is the place that decision belongs: a corpus reorganising its storage edits one
/// file, not every record it has ever written. Stamping the current route into each record
/// would freeze a routing answer at fetch time and make the config's own routing dead.
///
/// `redistributable:` is a licensing fact about the source — whether these bytes may leave
/// this machine at all — and nothing a fetch observes can establish it. Its own field note
/// says a route is edited casually and a licence is not something that edit may undo. A
/// machine writing a default here would be a machine asserting a licence, in a committed
/// file, on the strength of an HTTP 200.
fn artifact_for(o: &Obtained) -> CatalogArtifact {
    CatalogArtifact {
        sha256: Some(o.sha256.clone()),
        bytes: Some(o.bytes),
        media_type: o.media_type.clone(),
        retrieved: Some(today_iso()),
        from: Some(ArtifactOrigin::Location(o.location)),
        vault: None,
        redistributable: None,
    }
}

/// Which locations to follow, and why the others were passed over.
fn plan_locations(
    locations: &[CatalogLocation],
    root: &Path,
    opts: &FetchOptions,
) -> (Vec<(usize, Plan)>, Vec<Skipped>) {
    let mut plans = Vec::new();
    let mut skipped = Vec::new();
    for (i, loc) in locations.iter().enumerate() {
        if opts.location.is_some_and(|want| want != i) {
            continue;
        }
        match location::resolve(loc, root, &opts.bind) {
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
fn message(entry: &str, obtained: &[Obtained]) -> (String, String) {
    let subject = match obtained.len() {
        1 => format!("refresh: {entry} from {}", obtained[0].declared),
        n => format!("refresh: {entry} from {n} of its locations"),
    };
    let mut body = String::new();
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
    }
    (subject, body)
}

pub fn fetch(opts: &FetchOptions) -> Result<()> {
    let root = repo_root()?;
    let catalog = yidam_catalog_dir(&root);
    let entries = select(&catalog, opts.entry.as_deref())?;
    let vaults = crate::vault::resolve(&crate::config::load_yidam_config(&root)?.vault)?;
    let cache = Cache::resolve(|k| std::env::var(k).ok())?;

    let mut out: Vec<EntryOutcome> = Vec::new();
    let mut n = 0usize;
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

        let (plans, skipped) = plan_locations(&locations, &root, opts);
        if plans.is_empty() {
            if !skipped.is_empty() {
                out.push(EntryOutcome {
                    entry: name,
                    obtained: vec![],
                    skipped,
                    commit: None,
                });
            }
            continue;
        }

        // Checked before a byte moves, so the refusal names a person's uncommitted work
        // rather than the edit this command is about to make.
        if !opts.dry_run {
            commit::require_clean(&root, &[rel.clone()])?;
        }

        let mut obtained = Vec::new();
        for (i, plan) in &plans {
            if opts.dry_run {
                let (declared, followed) = match plan {
                    Plan::File { path, declared } => (declared.clone(), path.display().to_string()),
                    Plan::Url { url, declared } => (declared.clone(), url.clone()),
                };
                obtained.push(Obtained {
                    location: *i,
                    declared,
                    followed,
                    sha256: String::new(),
                    bytes: 0,
                    media_type: None,
                    cached: false,
                    route: String::new(),
                });
                continue;
            }
            n += 1;
            obtained.push(obtain(plan, *i, &cache, n, &vaults)?);
        }

        let mut written = None;
        if !opts.dry_run {
            let records: Vec<CatalogArtifact> = obtained.iter().map(artifact_for).collect();
            let updated = record::append_artifacts(&text, &records)
                .with_context(|| format!("recording what {rel} obtained"))?;
            if updated != text {
                std::fs::write(path, &updated)
                    .with_context(|| format!("writing {}", path.display()))?;
                let (subject, body) = message(&name, &obtained);
                written = commit::author(&root, &subject, &body, &[rel.clone()])?;
            }
        }

        out.push(EntryOutcome {
            entry: name,
            obtained,
            skipped,
            commit: written,
        });
    }

    if opts.format.is_json() {
        return crate::report::emit(&root, FetchReport { fetched: out });
    }
    print!("{}", render(&out, opts.dry_run));
    Ok(())
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
        match &e.commit {
            Some(c) => {
                let _ = writeln!(s, "  {} {}", c.sha, c.subject);
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
            format: crate::report::Format::Text,
        }
    }

    /// The streamflow entry's shape: a human-facing page and a machine endpoint. Without a
    /// binding the template is skipped and reported; the plain URL is still followed.
    #[test]
    fn an_unbindable_template_is_reported_and_its_sibling_is_still_planned() {
        let locs = vec![
            loc("url", "https://waterdata.usgs.gov/nwis"),
            loc("url_template", "https://x/?sites={site}"),
        ];
        let (plans, skipped) = plan_locations(&locs, Path::new("/repo"), &opts());
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
        let (plans, skipped) = plan_locations(&locs, Path::new("/repo"), &opts());
        assert_eq!(plans.len(), 1);
        assert!(skipped.is_empty(), "{skipped:?}");
    }

    /// But an entry whose only address is a physical one produces no plans, which is how the
    /// caller knows to say so.
    #[test]
    fn an_entry_with_only_an_address_plans_nothing() {
        let locs = vec![loc("address", "County Recorder, 14 Court St")];
        let (plans, skipped) = plan_locations(&locs, Path::new("/repo"), &opts());
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
        let (plans, skipped) = plan_locations(&locs, Path::new("/repo"), &o);
        assert!(plans.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].why.contains("place rather than an endpoint"));
    }

    #[test]
    fn a_location_index_restricts_the_run_to_that_one() {
        let locs = vec![loc("url", "https://a/"), loc("url", "https://b/")];
        let mut o = opts();
        o.location = Some(1);
        let (plans, _) = plan_locations(&locs, Path::new("/repo"), &o);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].0, 1);
    }

    /// The record is what a person reviews in the `refresh:` commit, so what it omits matters
    /// as much as what it carries.
    #[test]
    fn a_record_asserts_no_licence_and_freezes_no_route() {
        let o = Obtained {
            location: 0,
            declared: "d".into(),
            followed: "f".into(),
            sha256: "aa".into(),
            bytes: 12,
            media_type: Some("application/json".into()),
            cached: false,
            route: "sources (s3://x)".into(),
        };
        let a = artifact_for(&o);
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

    #[test]
    fn the_subject_names_the_entry_and_the_body_names_every_digest() {
        let o = |sha: &str, at: usize| Obtained {
            location: at,
            declared: format!("https://x/{at}"),
            followed: format!("https://x/{at}"),
            sha256: sha.into(),
            bytes: 3,
            media_type: None,
            cached: false,
            route: String::new(),
        };
        let (subject, body) = message("usgs-nwis", &[o("aa", 0)]);
        assert_eq!(subject, "refresh: usgs-nwis from https://x/0");
        assert!(body.contains("sha256:aa (3 bytes) from location 0"));

        let (subject, body) = message("usgs-nwis", &[o("aa", 0), o("bb", 1)]);
        assert_eq!(subject, "refresh: usgs-nwis from 2 of its locations");
        assert!(body.contains("sha256:aa") && body.contains("sha256:bb"));
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
        };
        for obtained in [vec![o.clone()], vec![o.clone(), o.clone()]] {
            let (subject, _) = message("e", &obtained);
            assert_eq!(
                yidam_core::git::classify_commit("", &subject).kind,
                yidam_core::git::CommitKind::Operational,
                "{subject}"
            );
        }
    }
}
