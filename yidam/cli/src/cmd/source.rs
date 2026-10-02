//! `yidam source` — the source packs a corpus resolves identifiers through (RFC-0048 §4).
//!
//! Four subcommands over one [`crate::sources::load`]: `check` (#1315), and `list`, `search`
//! and `add` (#1316).
//!
//! # `check` is offline, and is the gate for an authored pack
//!
//! A corpus that writes a pack for its own publisher has nothing else holding it to the format:
//! no crate compiles it and no template test reads it. So `check` exits nonzero on any error,
//! and a corpus runs it in CI. It reads only the repository: the manifests, the fixtures they
//! name, and `prelude_sources`. It never makes a request, so a pack is checked the same way on
//! a laptop with no network as in CI.
//!
//! # `search` and `list` write nothing; `add` writes a catalog entry and nothing else
//!
//! `add` writes a draft `.yidam/catalog/<slug>.md` as a `catalog:` commit, which is
//! operational: a source anchor added, and nothing said about what it holds. It never writes a
//! node and never writes `obtained: true`. Saying a source was read stays a person's commit, or
//! a `catalog-fetch` that recorded the bytes, which `--fetch` runs as a second commit.
//!
//! # `--offline`
//!
//! `search` and `add` ask a publisher, and both take `--offline`, which answers from the pack's
//! recorded fixtures and asks nothing. It is how a pack author previews what a pack will draft,
//! and how the tests of this command run without a network.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use serde::Serialize;

use crate::cmd::catalog::{self, transport};
use crate::cmd::operational::{Commit, Writer};
use crate::report::Format;
use crate::sources::manifest::{self, Manifest};
use crate::sources::resolve::{self, Answered, Asker, Needs, Resolved};
use crate::sources::search::{self, Candidate};
use crate::sources::transform;
use crate::sources::{self, Origin, Pack, Severity};

#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    /// Check every pack and every `prelude_sources` pin, offline; exit nonzero on an error
    ///
    /// Each manifest parses, each pattern compiles, each resolve template binds, and each
    /// fixture is claimed by a scheme. Each vendored pack is the one its pin asked for. Nothing
    /// is fetched.
    Check {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// The enabled packs, their schemes, and what each needs of the environment
    ///
    /// A pack's transport may require a contact (`YIDAM_CONTACT`) or name auth variables. Each
    /// is listed with whether this environment sets it, so a search that would be refused is
    /// visible before it is run. Values are never printed.
    List {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Ask a pack's search endpoint, and print the identifiers it answers with
    ///
    /// The pack's `[search]` table names the endpoint, and where in its answer each result's
    /// identifier and title are. Writes nothing.
    Search {
        /// The pack, by name.
        pack: String,
        /// What to search for.
        query: String,
        /// The most candidates to print, and the `{limit}` the endpoint is asked for.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Answer from the pack's recorded `[search] fixtures`, and ask nothing.
        #[arg(long)]
        offline: bool,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Write a draft catalog entry for each identifier, as a `catalog:` commit
    ///
    /// Each identifier is `scheme:local-id`. It is resolved through its pack, its `then` chain
    /// is followed, and the scheme's `describe` fills what it can where the build runs
    /// transforms. The draft is written `obtained: false`, with every identifier as a location,
    /// the pack's `ttl_days`, and the pack's `entry.md` as its body. Never a node.
    Add {
        /// One or more `scheme:local-id`.
        #[arg(required = true)]
        identifiers: Vec<String>,
        /// Then run `catalog-fetch` on each new entry, as a second commit.
        #[arg(long, conflicts_with = "dry_run")]
        fetch: bool,
        /// Answer each describe from the pack's recorded `[fixtures]`, and ask nothing.
        #[arg(long)]
        offline: bool,
        /// Resolve and report; write nothing, commit nothing.
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

pub fn run(root: Option<&Path>, sub: SourceCommand) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    crate::paths::require_yidam_repo(&root)?;
    match sub {
        SourceCommand::Check { format } => check(&root, format),
        SourceCommand::List { format } => list(&root, format),
        SourceCommand::Search {
            pack,
            query,
            limit,
            offline,
            format,
        } => search_cmd(&root, &pack, &query, limit, offline, format),
        SourceCommand::Add {
            identifiers,
            fetch,
            offline,
            dry_run,
            format,
        } => add(&root, &identifiers, fetch, offline, dry_run, format),
    }
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

// ── check ────────────────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct CheckReport {
    passed: bool,
    packs: Vec<sources::Summary>,
    /// Not `findings`, which the contract gives `check-diff`'s shape.
    pack_findings: Vec<sources::Finding>,
}

fn check(root: &Path, format: Format) -> Result<()> {
    let checked = sources::check(root)?;
    let passed = checked.ok();
    let report = CheckReport {
        passed,
        packs: checked.packs,
        pack_findings: checked.findings,
    };
    crate::report::gate(root, format, report, passed, render_check)
}

fn render_check(report: &CheckReport) {
    if report.packs.is_empty() && report.pack_findings.is_empty() {
        println!(
            "No source packs: nothing in {} or {}, and nothing pinned in prelude_sources.",
            sources::AUTHORED,
            sources::VENDORED
        );
        return;
    }
    if !report.packs.is_empty() {
        println!("Packs");
        for p in &report.packs {
            let origin = match p.origin {
                Origin::Authored => "authored",
                Origin::Vendored if p.shadowed => "shadowed",
                Origin::Vendored => "vendored",
            };
            let schemes = if p.schemes.is_empty() {
                "-".to_string()
            } else {
                p.schemes.join(", ")
            };
            println!(
                "  {:<20} {:<9} {:<9} {schemes} ({} fixture{})",
                p.name,
                p.version.as_deref().unwrap_or("?"),
                origin,
                p.fixtures,
                if p.fixtures == 1 { "" } else { "s" },
            );
        }
    }
    if !report.pack_findings.is_empty() {
        println!();
        for f in &report.pack_findings {
            let tag = match f.severity {
                Severity::Error => "error",
                Severity::Info => "info ",
            };
            println!("  {tag}  {}: {}", f.path, f.message);
        }
    }
    let errors = report
        .pack_findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    println!();
    if report.passed {
        println!("Every pack checks.");
    } else {
        println!("{errors} error(s).");
    }
}

// ── list ─────────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListReport {
    /// Whether this build runs a pack's `describe` and `extract`.
    transforms_run: bool,
    source_packs: Vec<Listed>,
    /// Pack directories whose manifest does not parse, which declare nothing. `source check`
    /// says why.
    unreadable_packs: Vec<String>,
}

#[derive(Serialize)]
struct Listed {
    name: String,
    version: String,
    origin: Origin,
    path: String,
    description: Option<String>,
    schemes: Vec<ListedScheme>,
    /// The scheme `[search]` answers in, when the pack declares one.
    search: Option<String>,
    needs: Needs,
    /// Whether this environment meets every need, so a request to the publisher would be made.
    ready: bool,
}

#[derive(Serialize)]
struct ListedScheme {
    scheme: String,
    #[serde(rename = "type")]
    kind: String,
    describe: bool,
    extract: bool,
    /// The schemes its `then` leads to.
    then: Vec<String>,
}

fn list(root: &Path, format: Format) -> Result<()> {
    let packs = sources::load(root)?;
    let enabled = transform::enabled(&packs);
    let report = ListReport {
        transforms_run: transform::AVAILABLE,
        source_packs: enabled
            .iter()
            .map(|(p, m)| {
                let needs = Needs::of(m, &env);
                Listed {
                    name: p.name.clone(),
                    version: m.pack.version.clone(),
                    origin: p.origin,
                    path: p.dir.to_string_lossy().replace('\\', "/"),
                    description: m.pack.description.clone(),
                    schemes: m
                        .scheme
                        .iter()
                        .map(|(name, s)| ListedScheme {
                            scheme: name.clone(),
                            kind: s.kind.clone(),
                            describe: s.describe.is_some(),
                            extract: s.extract.is_some(),
                            then: s.then.iter().map(|t| t.scheme.clone()).collect(),
                        })
                        .collect(),
                    search: m.search.as_ref().map(|s| s.scheme.clone()),
                    ready: needs.satisfied(),
                    needs,
                }
            })
            .collect(),
        unreadable_packs: packs
            .iter()
            .filter(|p| p.manifest.is_err())
            .map(|p| p.dir.to_string_lossy().replace('\\', "/"))
            .collect(),
    };
    crate::report::finish(root, format, report, render_list)
}

fn render_list(r: &ListReport) {
    if r.source_packs.is_empty() {
        println!(
            "No enabled source packs: nothing in {} or {}.",
            sources::AUTHORED,
            sources::VENDORED
        );
    }
    for p in &r.source_packs {
        let origin = match p.origin {
            Origin::Authored => "authored",
            Origin::Vendored => "vendored",
        };
        println!("{} {} ({origin})", p.name, p.version);
        if let Some(d) = &p.description {
            println!("  {d}");
        }
        for s in &p.schemes {
            let mut notes = Vec::new();
            if s.describe {
                notes.push("describe".to_string());
            }
            if s.extract {
                notes.push("extract".to_string());
            }
            if !s.then.is_empty() {
                notes.push(format!("then {}", s.then.join(", ")));
            }
            println!("  {:<14} {:<9} {}", s.scheme, s.kind, notes.join(" · "));
        }
        if let Some(s) = &p.search {
            println!("  search         answers `{s}:` identifiers");
        }
        let n = &p.needs;
        println!(
            "  contact        {}{}",
            n.contact,
            if n.contact_set {
                format!(" — {} is set", resolve::CONTACT)
            } else if n.contact == "required" {
                format!(" — {} is not set", resolve::CONTACT)
            } else {
                String::new()
            }
        );
        for a in &n.auth {
            println!(
                "  auth           {} {}",
                a.var,
                if a.set { "is set" } else { "is not set" }
            );
        }
        if let Some(b) = &n.blocked {
            println!("  blocked        {b}");
        }
        if !p.ready {
            println!("  not ready: a request to this publisher would be refused here");
        }
        println!();
    }
    if !r.unreadable_packs.is_empty() {
        println!(
            "Not enabled, because the manifest does not parse: {}. `yidam source check` says why.",
            r.unreadable_packs.join(", ")
        );
    }
    if !r.transforms_run {
        println!("Note: {}.", transform::UNAVAILABLE);
    }
}

// ── search ───────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct SearchReport {
    search: Searched,
}

#[derive(Serialize)]
struct Searched {
    pack: String,
    query: String,
    /// The scheme every candidate's identifier is in.
    scheme: String,
    /// The address asked, with the query bound. Asked only when `answered` is `network`.
    url: String,
    answered: Answered,
    candidates: Vec<Candidate>,
}

fn search_cmd(
    root: &Path,
    pack: &str,
    query: &str,
    limit: usize,
    offline: bool,
    format: Format,
) -> Result<()> {
    let packs = sources::load(root)?;
    let enabled = transform::enabled(&packs);
    let Some((p, m)) = enabled.iter().find(|(p, _)| p.name == pack) else {
        let names: Vec<&str> = enabled.iter().map(|(p, _)| p.name.as_str()).collect();
        bail!(
            "no enabled source pack is named `{pack}`{}",
            if names.is_empty() {
                " — this repository has none; `yidam source list` reads the same set".to_string()
            } else {
                format!("; the enabled packs are {}", names.join(", "))
            }
        );
    };
    let Some(s) = &m.search else {
        bail!(
            "{} declares no [search], so it has no endpoint to ask; `source add` takes an \
             identifier you already have",
            p.name
        );
    };
    let url = search_url(s, query, limit);
    let bytes = if offline {
        let Some(file) = s.fixtures.get(query) else {
            bail!(
                "offline, and {} records no [search] fixture for `{query}`{}",
                p.name,
                if s.fixtures.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; it records {}",
                        s.fixtures
                            .keys()
                            .map(|q| format!("`{q}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            );
        };
        let path = root.join(&p.dir).join("fixtures").join(file);
        std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?
    } else {
        if let Some(why) = Needs::of(m, &env).unmet() {
            bail!(
                "{} was not asked: {why}. `--offline` answers from its fixtures",
                p.name
            );
        }
        transport::read(&url, env(resolve::CONTACT).as_deref())?
    };
    let mut candidates = search::candidates(s, &bytes).map_err(anyhow::Error::msg)?;
    candidates.truncate(limit);
    let report = SearchReport {
        search: Searched {
            pack: p.name.clone(),
            query: query.to_string(),
            scheme: s.scheme.clone(),
            url,
            answered: if offline {
                Answered::Fixture
            } else {
                Answered::Network
            },
            candidates,
        },
    };
    crate::report::finish(root, format, report, render_search)
}

/// The search template with the query and the limit bound, the query percent-encoded.
fn search_url(s: &manifest::Search, query: &str, limit: usize) -> String {
    s.template
        .replace("{query}", &percent_encode(query))
        .replace("{limit}", &limit.to_string())
}

/// Every byte but RFC 3986's unreserved set, as `%XX`.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn render_search(r: &SearchReport) {
    let s = &r.search;
    if s.candidates.is_empty() {
        println!("{} answered nothing for `{}`.", s.pack, s.query);
    }
    for c in &s.candidates {
        match &c.title {
            Some(t) => println!("{}  {t}", c.identifier),
            None => println!("{}", c.identifier),
        }
    }
    if s.answered == Answered::Fixture {
        println!(
            "\n(answered from {}'s recorded fixture; nothing was asked)",
            s.pack
        );
    }
}

// ── add ──────────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AddReport {
    drafts: Vec<Drafted>,
    /// What `--fetch` did with each new entry: `catalog-fetch`'s own outcome.
    fetched: Vec<catalog::EntryOutcome>,
}

#[derive(Serialize)]
struct Drafted {
    /// The entry's file stem, and its `name:`.
    entry: String,
    /// `.yidam/catalog/<entry>.md`.
    path: String,
    /// Whether the file was written. False on `--dry-run`.
    written: bool,
    commit: Option<Commit>,
    #[serde(flatten)]
    resolved: Resolved,
}

/// Answers each describe by asking the publisher, a pack's `min_interval` apart.
struct Network {
    last: HashMap<String, Instant>,
}

impl Asker for Network {
    fn answered(&self) -> Answered {
        Answered::Network
    }
    fn ask(&mut self, pack: &Pack, m: &Manifest, _: &str, url: &str) -> Result<Vec<u8>, String> {
        if let Some(why) = Needs::of(m, &env).unmet() {
            return Err(format!("{} was not asked: {why}", pack.name));
        }
        let gap = m
            .transport
            .min_interval
            .as_deref()
            .and_then(manifest::parse_interval);
        if let (Some(gap), Some(at)) = (gap, self.last.get(&pack.name)) {
            if let Some(wait) = gap.checked_sub(at.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        let got =
            transport::read(url, env(resolve::CONTACT).as_deref()).map_err(|e| format!("{e:#}"));
        self.last.insert(pack.name.clone(), Instant::now());
        got
    }
}

fn add(
    root: &Path,
    identifiers: &[String],
    fetch: bool,
    offline: bool,
    dry_run: bool,
    format: Format,
) -> Result<()> {
    let packs = sources::load(root)?;
    let catalog_dir = crate::paths::yidam_catalog_dir(root);
    let held = catalogued(&catalog_dir);

    let mut fixtures = resolve::Fixtures { root };
    let mut network = Network {
        last: HashMap::new(),
    };
    let asker: &mut dyn Asker = if offline { &mut fixtures } else { &mut network };

    // Every identifier resolves, or nothing is written: a run that wrote two of three entries
    // and then refused would leave a person to work out which.
    let mut resolved = Vec::new();
    let mut refused = Vec::new();
    let mut taken: Vec<String> = Vec::new();
    for raw in identifiers {
        match resolve::resolve(root, &packs, raw, asker) {
            Err(why) => refused.push(why),
            Ok(r) => {
                if let Some((id, at)) = r
                    .locations
                    .iter()
                    .find_map(|l| held.get(&l.identifier).map(|at| (&l.identifier, at)))
                {
                    refused.push(format!(
                        "`{id}` is already catalogued in {at}; a second entry for one source \
                         would split what is said about it"
                    ));
                    continue;
                }
                let entry = free_slug(&catalog_dir, &slug_for(&r), &taken);
                taken.push(entry.clone());
                resolved.push((entry, r));
            }
        }
    }
    if !refused.is_empty() {
        bail!(
            "nothing was written:\n{}",
            refused
                .iter()
                .map(|r| format!("  {r}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    let mut writer = Writer::WorkingTree;
    let mut drafts = Vec::new();
    for (entry, r) in resolved {
        let rel = format!(".yidam/catalog/{entry}.md");
        let mut commit = None;
        if !dry_run {
            writer.require_clean(root, &[rel.clone()])?;
            let (subject, body) = message(&entry, &r);
            commit = writer.commit(
                root,
                catalog::WHO,
                &subject,
                &body,
                &[(rel.clone(), entry_text(&entry, &r))],
            )?;
        }
        drafts.push(Drafted {
            entry,
            path: rel,
            written: !dry_run,
            commit,
            resolved: r,
        });
    }

    let mut fetched = Vec::new();
    if fetch {
        for d in &drafts {
            let opts = catalog::FetchOptions {
                entry: Some(d.entry.clone()),
                location: None,
                bind: Vec::new(),
                dry_run: false,
                format,
            };
            fetched.extend(catalog::fetch_in(root, &opts, &mut writer)?);
        }
    }

    let report = AddReport { drafts, fetched };
    crate::report::finish(root, format, report, |r| render_add(r, dry_run, fetch))
}

/// Every identifier the catalog already records, and the entry it is in.
fn catalogued(dir: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for path in crate::walk::walk_md_files(dir) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        for l in crate::parse::parse_frontmatter(&text)
            .location
            .unwrap_or_default()
        {
            if l.kind.as_deref() == Some("identifier") {
                if let Some(v) = l.value {
                    out.entry(v.trim().to_string())
                        .or_insert_with(|| format!(".yidam/catalog/{name}"));
                }
            }
        }
    }
    out
}

/// The entry's stem: the describe's name where there is one, else the identifier.
fn slug_for(r: &Resolved) -> String {
    let from = r
        .draft
        .name
        .as_deref()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(&r.identifier);
    let mut out = String::new();
    for c in from.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    // Sixty characters at a word boundary: a title is a sentence, and a file name is not.
    let mut out = out.trim_matches('-').to_string();
    if out.len() > 60 {
        let cut = out[..60].rfind('-').unwrap_or(60);
        out.truncate(cut);
    }
    if out.is_empty() {
        "source".into()
    } else {
        out
    }
}

/// `base`, or `base-2`, `base-3`… — the first that is neither on disk nor taken by this run.
fn free_slug(dir: &Path, base: &str, taken: &[String]) -> String {
    let free = |s: &str| !dir.join(format!("{s}.md")).exists() && !taken.iter().any(|t| t == s);
    if free(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|s| free(s))
        .unwrap_or_else(|| base.to_string())
}

/// A YAML scalar, quoted when YAML would read it as something else.
fn scalar(s: &str) -> String {
    serde_yaml::to_string(s)
        .map(|y| y.trim_end().to_string())
        .unwrap_or_else(|_| format!("{s:?}"))
}

/// The draft entry: frontmatter from the resolution, body from the pack.
fn entry_text(entry: &str, r: &Resolved) -> String {
    let d = &r.draft;
    let mut out = String::from("---\n");
    let _ = writeln!(out, "name: {}", scalar(entry));
    if let Some(desc) = d.description.as_deref().filter(|s| !s.trim().is_empty()) {
        let _ = writeln!(out, "description: {}", scalar(desc.trim()));
    }
    if let Some(kind) = &d.kind {
        let _ = writeln!(out, "type: {}", scalar(kind));
    }
    out.push_str("obtained: false\n");
    if let Some(date) = d.date.as_deref().filter(|s| !s.trim().is_empty()) {
        let _ = writeln!(out, "date: {}", scalar(date.trim()));
    }
    if let Some(ttl) = r.ttl_days {
        let _ = writeln!(out, "ttl_days: {ttl}");
    }
    out.push_str("location:\n");
    for l in &r.locations {
        out.push_str("  - kind: identifier\n");
        let _ = writeln!(out, "    value: {}", scalar(&l.identifier));
        let _ = writeln!(out, "    description: {}", scalar(&l.description()));
    }
    out.push_str("---\n\n");
    let title = d
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&r.identifier);
    let _ = writeln!(out, "# {title}\n");
    // The pack's own H1 names the pack, not the source, so the entry's title replaces it.
    let body = r.body.trim_start();
    let body = match body.strip_prefix("# ") {
        Some(rest) => rest.split_once('\n').map_or("", |(_, b)| b).trim_start(),
        None => body,
    };
    out.push_str(body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The `catalog:` commit: what was added, from where, and what the draft could not fill.
fn message(entry: &str, r: &Resolved) -> (String, String) {
    let subject = format!("catalog: {entry} from {}", r.identifier);
    let mut body = format!(
        "A draft entry from the {} pack, written `obtained: false`: nothing has been read.\n",
        r.pack
    );
    if r.locations.len() > 1 {
        body.push_str("\nLocations:\n");
        for l in &r.locations {
            let _ = writeln!(body, "- {} ({})", l.identifier, l.description());
        }
    }
    match &r.draft.by {
        Some(by) => {
            let _ = writeln!(body, "\nDescribed by {by}.");
        }
        None => {
            if let Some(why) = &r.draft.why {
                let _ = writeln!(body, "\nNot described: {why}.");
            }
        }
    }
    if !r.draft.unfilled.is_empty() {
        let _ = writeln!(body, "Unfilled: {}.", r.draft.unfilled.join(", "));
    }
    (subject, body)
}

fn render_add(r: &AddReport, dry_run: bool, fetch: bool) {
    for d in &r.drafts {
        let verb = if dry_run { "would write" } else { "wrote" };
        println!("{} — {verb} {}", d.resolved.identifier, d.path);
        for l in &d.resolved.locations {
            println!("  {}  {}", l.identifier, l.url);
        }
        match &d.resolved.draft.by {
            Some(by) => println!("  described by {by}"),
            None => {
                if let Some(why) = &d.resolved.draft.why {
                    println!("  not described: {why}");
                }
            }
        }
        if !d.resolved.draft.unfilled.is_empty() {
            println!("  unfilled: {}", d.resolved.draft.unfilled.join(", "));
        }
        for u in &d.resolved.unfollowed {
            println!("  not followed: {u}");
        }
        if let Some(c) = &d.commit {
            println!("  {} {}", &c.sha[..c.sha.len().min(7)], c.subject);
        }
    }
    if fetch {
        println!();
        print!("{}", catalog::render_fetched(&r.fetched, false));
    }
}
