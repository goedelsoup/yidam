//! Source packs (RFC-0048): a directory that names a family of sources by identifier.
//!
//! ```text
//! <pack>/
//!   pack.toml          # schemes, resolution, transport, entry defaults, fixtures
//!   entry.md           # the body a new catalog entry starts from
//!   transforms/*.glu   # optional; pure, under the closed prelude (#1318)
//!   fixtures/          # recorded responses; nothing here touches the network
//! ```
//!
//! # Where packs are read from
//!
//! A corpus reads its own packs in `.yidam/sources/` before the vendored ones in
//! `.yidam/.vendor/sources/`. So a corpus can carry a pack for a publisher the template will
//! never ship, and an authored pack with a vendored pack's name is the one that counts. The
//! vendored copy it shadows stays on disk under its pin, and is reported as shadowed.
//!
//! A vendored pack arrives only through `yidam-vendor-update`, which copies each pin in
//! `prelude_sources` ([`pin`]) wholesale and writes `[vendored]` onto the copy. Vendoring is the
//! only step that reaches another repository, and it lands as a reviewed commit.
//!
//! # What [`check`] holds a pack to
//!
//! It runs offline and reads nothing outside the repository. It checks five things:
//!
//! - the manifest parses
//! - patterns compile
//! - templates bind
//! - fixtures are claimed by a scheme
//! - every vendored pack is the one its pin asked for
//! - a `[search]` names a scheme of its own, binds only `{query}` and `{limit}`, and every
//!   one of its fixtures answers with candidates that scheme's pattern admits (#1316)
//! - every transform is admitted by the closed prelude, runs over each of its scheme's
//!   fixtures, and yields every identifier a `then.from` reads from it (#1318)
//!
//! The last needs the gluon engine, which is behind `source-transforms`. A build without it
//! says so once per pack that declares a transform, as an info finding, and checks the rest.

pub mod manifest;
#[cfg(feature = "source-transforms")]
pub mod parsed;
pub mod pin;
pub mod resolve;
pub mod search;
pub mod transform;
pub mod version;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use manifest::Manifest;
use pin::Pin;
use version::Version;

/// Packs this corpus wrote. Tracked, linted, and read first.
pub const AUTHORED: &str = ".yidam/sources";
/// Packs `yidam-vendor-update` copied in from a pin.
pub const VENDORED: &str = ".yidam/.vendor/sources";
/// Where `prelude_sources` is declared.
pub const PROPOSALS: &str = ".yidam/decisions/proposals.yml";

/// Where a pack came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Authored,
    Vendored,
}

/// One pack directory, and its manifest or why it has none.
#[derive(Debug)]
pub struct Pack {
    /// The directory's name.
    pub name: String,
    /// Relative to the repository root.
    pub dir: PathBuf,
    pub origin: Origin,
    pub manifest: Result<Manifest, String>,
}

/// Every pack directory, authored ones first, each set in name order.
///
/// A file beside the pack directories (a README) is not a pack and is skipped.
pub fn load(root: &Path) -> Result<Vec<Pack>> {
    let mut packs = Vec::new();
    for (base, origin) in [(AUTHORED, Origin::Authored), (VENDORED, Origin::Vendored)] {
        let abs = root.join(base);
        if !abs.is_dir() {
            continue;
        }
        let mut names: Vec<String> = std::fs::read_dir(&abs)
            .with_context(|| format!("reading {base}"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        for name in names {
            let dir = Path::new(base).join(&name);
            let manifest = std::fs::read_to_string(root.join(&dir).join("pack.toml"))
                .map_err(|e| format!("pack.toml: {e}"))
                .and_then(|text| toml::from_str(&text).map_err(|e| format!("pack.toml: {e}")));
            packs.push(Pack {
                name,
                dir,
                origin,
                manifest,
            });
        }
    }
    Ok(packs)
}

/// Whether a finding fails the check. Two of the contract's three severity words: nothing
/// here warns without failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: Severity,
    /// The pack, or the pin, the finding is about.
    pub pack: String,
    /// The file to open, relative to the repository root.
    pub path: String,
    pub message: String,
}

/// One pack as `source check` read it.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub name: String,
    /// `None` when the manifest did not parse.
    pub version: Option<String>,
    pub origin: Origin,
    pub path: String,
    pub schemes: Vec<String>,
    pub fixtures: usize,
    /// A vendored pack an authored pack of the same name replaces. It is not enabled.
    pub shadowed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub packs: Vec<Summary>,
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.findings.iter().all(|f| f.severity != Severity::Error)
    }
}

/// Hold every pack, and every pin, to the rules in the module note.
pub fn check(root: &Path) -> Result<Report> {
    let packs = load(root)?;
    let mut findings = Vec::new();
    let authored: BTreeSet<&str> = packs
        .iter()
        .filter(|p| p.origin == Origin::Authored)
        .map(|p| p.name.as_str())
        .collect();

    // scheme -> the packs that declare it, among the enabled ones.
    let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut summaries = Vec::new();
    for pack in &packs {
        let shadowed = pack.origin == Origin::Vendored && authored.contains(pack.name.as_str());
        let mut out = Findings {
            pack: &pack.name,
            dir: &pack.dir,
            findings: &mut findings,
        };
        let manifest = match &pack.manifest {
            Ok(m) => m,
            Err(e) => {
                out.error("pack.toml", e.clone());
                summaries.push(summary(pack, None, shadowed));
                continue;
            }
        };
        check_pack(root, pack, manifest, &mut out);
        check_transforms(root, pack, manifest, &mut out);
        if shadowed {
            out.info(
                "pack.toml",
                format!(
                    "shadowed by {AUTHORED}/{}, which is read first; this copy is not enabled",
                    pack.name
                ),
            );
        } else {
            for scheme in manifest.scheme.keys() {
                owners
                    .entry(scheme.clone())
                    .or_default()
                    .push(pack.name.clone());
            }
        }
        summaries.push(summary(pack, Some(manifest), shadowed));
    }

    for (scheme, by) in owners.iter().filter(|(_, by)| by.len() > 1) {
        for name in by {
            let dir = packs
                .iter()
                .find(|p| {
                    &p.name == name
                        && !(p.origin == Origin::Vendored && authored.contains(name.as_str()))
                })
                .map(|p| p.dir.clone())
                .unwrap_or_default();
            Findings {
                pack: name,
                dir: &dir,
                findings: &mut findings,
            }
            .error(
                "pack.toml",
                format!(
                    "scheme `{scheme}` is declared by {}; an identifier `{scheme}:…` would not say which pack resolves it",
                    by.join(" and ")
                ),
            );
        }
    }

    check_pins(root, &packs, &mut findings)?;

    Ok(Report {
        packs: summaries,
        findings,
    })
}

fn summary(pack: &Pack, manifest: Option<&Manifest>, shadowed: bool) -> Summary {
    Summary {
        name: pack.name.clone(),
        version: manifest.map(|m| m.pack.version.clone()),
        origin: pack.origin,
        path: slash(&pack.dir),
        schemes: manifest
            .map(|m| m.scheme.keys().cloned().collect())
            .unwrap_or_default(),
        fixtures: manifest.map_or(0, |m| m.fixtures.len()),
        shadowed,
    }
}

/// Collects findings against one pack, with paths relative to its directory.
struct Findings<'a> {
    pack: &'a str,
    dir: &'a Path,
    findings: &'a mut Vec<Finding>,
}

impl Findings<'_> {
    fn push(&mut self, severity: Severity, file: &str, message: String) {
        self.findings.push(Finding {
            severity,
            pack: self.pack.to_string(),
            path: slash(&self.dir.join(file)),
            message,
        });
    }
    fn error(&mut self, file: &str, message: String) {
        self.push(Severity::Error, file, message);
    }
    fn info(&mut self, file: &str, message: String) {
        self.push(Severity::Info, file, message);
    }
}

fn check_pack(root: &Path, pack: &Pack, m: &Manifest, out: &mut Findings<'_>) {
    let abs = root.join(&pack.dir);

    if m.pack.name != pack.name {
        out.error(
            "pack.toml",
            format!(
                "[pack] name is `{}` but the directory is `{}`; a pin names the directory",
                m.pack.name, pack.name
            ),
        );
    }
    if !pin::is_pack_name(&pack.name) {
        out.error(
            "pack.toml",
            format!(
                "`{}` is not a pack name: lowercase letters, digits and `-`, starting with a letter",
                pack.name
            ),
        );
    }
    if Version::parse(&m.pack.version).is_none() {
        out.error(
            "pack.toml",
            format!(
                "[pack] version `{}` is not MAJOR.MINOR.PATCH; a pin cannot be checked against it",
                m.pack.version
            ),
        );
    }
    if !abs.join("entry.md").is_file() {
        out.error(
            "entry.md",
            "missing; it is the body a new catalog entry from this pack starts from".into(),
        );
    }
    match (pack.origin, &m.vendored) {
        (Origin::Authored, Some(_)) => out.error(
            "pack.toml",
            "carries [vendored], which yidam-vendor-update writes on a copy; an authored pack has none"
                .into(),
        ),
        (Origin::Vendored, None) => out.error(
            "pack.toml",
            "has no [vendored] record, so nothing says which pin it satisfies; re-run `mise run yidam-vendor-update`"
                .into(),
        ),
        _ => {}
    }

    if m.scheme.is_empty() {
        out.error(
            "pack.toml",
            "declares no [scheme.*]; it resolves nothing".into(),
        );
    }
    let mut compiled: BTreeMap<&str, regex::Regex> = BTreeMap::new();
    for (name, s) in &m.scheme {
        let at = |what: &str| format!("[scheme.{name}] {what}");
        if !is_scheme_name(name) {
            out.error(
                "pack.toml",
                format!(
                    "scheme `{name}` is not lowercase letters, digits and `-` starting with a letter, or is `http`/`https`"
                ),
            );
        }
        if !crate::parse::CATALOG_TYPES.contains(&s.kind.as_str()) {
            out.error(
                "pack.toml",
                format!(
                    "{} `{}` is not a catalog type: {}",
                    at("type"),
                    s.kind,
                    crate::parse::CATALOG_TYPES.join(", ")
                ),
            );
        }
        if !(s.pattern.starts_with('^') && s.pattern.ends_with('$')) {
            out.error(
                "pack.toml",
                format!(
                    "{} is not anchored with `^…$`, so it would accept an id that only contains a match",
                    at("pattern")
                ),
            );
        }
        let names: Vec<String> = match regex::Regex::new(&s.pattern) {
            Ok(re) => {
                let names = re.capture_names().flatten().map(str::to_string).collect();
                compiled.insert(name.as_str(), re);
                names
            }
            Err(e) => {
                out.error(
                    "pack.toml",
                    format!("{} does not compile: {e}", at("pattern")),
                );
                Vec::new()
            }
        };
        let template = &s.resolve.template;
        if !(template.starts_with("https://") || template.starts_with("http://")) {
            out.error(
                "pack.toml",
                format!(
                    "{} `{template}` is not an http(s) address",
                    at("resolve.template")
                ),
            );
        }
        let slots = crate::cmd::catalog::location::slots(template);
        if slots.is_empty() {
            out.error(
                "pack.toml",
                format!(
                    "{} has no slot, so every identifier would resolve to the same address",
                    at("resolve.template")
                ),
            );
        }
        for slot in slots.iter().filter(|s| *s != "id" && !names.contains(s)) {
            out.error(
                "pack.toml",
                format!(
                    "{} slot `{{{slot}}}` is bound by nothing: it is not `{{id}}` and the pattern has no group `(?P<{slot}>…)`",
                    at("resolve.template")
                ),
            );
        }
        for (field, path) in [("describe", &s.describe), ("extract", &s.extract)] {
            let Some(path) = path else { continue };
            let inside = path.starts_with("transforms/")
                && path.ends_with(".glu")
                && !path.split('/').any(|c| c == ".." || c.is_empty());
            if !inside {
                out.error(
                    "pack.toml",
                    format!(
                        "{} `{path}` is not a `transforms/<name>.glu` path",
                        at(field)
                    ),
                );
            } else if !abs.join(path).is_file() {
                out.error(path, format!("named by {} and missing", at(field)));
            }
        }
        for then in &s.then {
            if !m.scheme.contains_key(&then.scheme) {
                out.error(
                    "pack.toml",
                    format!(
                        "{} names scheme `{}`, which this pack does not declare",
                        at("then"),
                        then.scheme
                    ),
                );
            }
            match then.from.strip_prefix("describe.") {
                Some(field) if !field.is_empty() => {
                    if s.describe.is_none() {
                        out.error(
                            "pack.toml",
                            format!(
                                "{} takes `{}` from describe, and the scheme declares no describe",
                                at("then"),
                                then.from
                            ),
                        );
                    }
                }
                _ => out.error(
                    "pack.toml",
                    format!(
                        "{} from `{}` is not `describe.<field>`",
                        at("then"),
                        then.from
                    ),
                ),
            }
        }
    }

    let t = &m.transport;
    if let Some(raw) = &t.min_interval {
        if manifest::parse_interval(raw).is_none() {
            out.error(
                "pack.toml",
                format!("[transport] min_interval `{raw}` is not `<n>ms` or `<n>s`"),
            );
        }
    }
    for var in &t.auth {
        if !is_env_name(var) {
            out.error(
                "pack.toml",
                format!(
                    "[transport] auth `{var}` is not an environment variable name; auth names a variable, never a value"
                ),
            );
        }
    }

    // Fixtures: every identifier claimed by a scheme and resolvable, every file claimed.
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    for (id, file) in &m.fixtures {
        let Some((scheme, local)) = id.split_once(':') else {
            out.error(
                "pack.toml",
                format!("[fixtures] `{id}` is not `scheme:local-id`"),
            );
            continue;
        };
        let inside = !file.is_empty()
            && !file.starts_with('/')
            && !file.split('/').any(|c| c == ".." || c.is_empty());
        if !inside {
            out.error(
                "pack.toml",
                format!("[fixtures] `{id}` names `{file}`, which is not a path inside fixtures/"),
            );
        } else {
            claimed.insert(file.clone());
            if !abs.join("fixtures").join(file).is_file() {
                out.error(
                    &format!("fixtures/{file}"),
                    format!("named by [fixtures] `{id}` and missing"),
                );
            }
        }
        let Some(s) = m.scheme.get(scheme) else {
            out.error(
                "pack.toml",
                format!("[fixtures] `{id}`: this pack declares no scheme `{scheme}`"),
            );
            continue;
        };
        let Some(re) = compiled.get(scheme) else {
            continue;
        };
        let Some(caps) = re.captures(local) else {
            out.error(
                "pack.toml",
                format!("[fixtures] `{id}`: `{local}` does not match [scheme.{scheme}] pattern"),
            );
            continue;
        };
        let mut bindings = vec![("id".to_string(), local.to_string())];
        for group in re.capture_names().flatten() {
            if let Some(v) = caps.name(group) {
                bindings.push((group.to_string(), v.as_str().to_string()));
            }
        }
        if let Err(unbound) = crate::cmd::catalog::location::bind(&s.resolve.template, &bindings) {
            out.error(
                "pack.toml",
                format!(
                    "[fixtures] `{id}` leaves {} unbound in [scheme.{scheme}] resolve.template",
                    unbound
                        .iter()
                        .map(|s| format!("`{{{s}}}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        }
    }
    if let Some(search) = &m.search {
        check_search(&abs, m, search, &compiled, &mut claimed, out);
    }
    let fixtures = abs.join("fixtures");
    for file in files_under(&fixtures) {
        if !claimed.contains(&file) {
            out.error(
                &format!("fixtures/{file}"),
                "no [fixtures] identifier names this file, so no scheme claims it".into(),
            );
        }
    }
}

/// The slots a `[search]` template may carry.
pub const SEARCH_SLOTS: &[&str] = &["query", "limit"];

fn check_search(
    abs: &Path,
    m: &Manifest,
    search: &manifest::Search,
    compiled: &BTreeMap<&str, regex::Regex>,
    claimed: &mut BTreeSet<String>,
    out: &mut Findings<'_>,
) {
    let scheme = &search.scheme;
    if !m.scheme.contains_key(scheme) {
        out.error(
            "pack.toml",
            format!("[search] scheme `{scheme}` is not one this pack declares"),
        );
    }
    let template = &search.template;
    if !(template.starts_with("https://") || template.starts_with("http://")) {
        out.error(
            "pack.toml",
            format!("[search] template `{template}` is not an http(s) address"),
        );
    }
    let slots = crate::cmd::catalog::location::slots(template);
    if !slots.iter().any(|s| s == "query") {
        out.error(
            "pack.toml",
            "[search] template has no `{query}` slot, so every query would ask the same thing"
                .into(),
        );
    }
    for slot in slots.iter().filter(|s| !SEARCH_SLOTS.contains(&s.as_str())) {
        out.error(
            "pack.toml",
            format!("[search] template slot `{{{slot}}}` is bound by nothing: a search binds `{{query}}` and `{{limit}}`"),
        );
    }
    for (field, path) in [("items", &search.items), ("id", &search.id)] {
        if path.split('/').all(str::is_empty) {
            out.error("pack.toml", format!("[search] {field} is an empty path"));
        }
    }
    for (query, file) in &search.fixtures {
        let inside = !file.is_empty()
            && !file.starts_with('/')
            && !file
                .split('/')
                .any(|c| c == ".." || c == "." || c.is_empty());
        if !inside {
            out.error(
                "pack.toml",
                format!("[search] fixture `{query}` names `{file}`, which is not a path inside fixtures/"),
            );
            continue;
        }
        claimed.insert(file.clone());
        let at = format!("fixtures/{file}");
        let Ok(bytes) = std::fs::read(abs.join("fixtures").join(file)) else {
            out.error(
                &at,
                format!("named by [search] fixture `{query}` and missing"),
            );
            continue;
        };
        let found = match search::candidates(search, &bytes) {
            Ok(found) => found,
            Err(why) if !transform::AVAILABLE && !search::is_json(&search.media) => {
                out.info(&at, format!("not read: {why}"));
                continue;
            }
            Err(why) => {
                out.error(&at, format!("[search] fixture `{query}`: {why}"));
                continue;
            }
        };
        if found.is_empty() {
            out.error(
                &at,
                format!(
                    "[search] fixture `{query}` yields no candidate: nothing at `{}` has an `{}`",
                    search.items, search.id
                ),
            );
        }
        let Some(re) = compiled.get(scheme.as_str()) else {
            continue;
        };
        for c in &found {
            let local = c.identifier.split_once(':').map_or("", |(_, l)| l);
            if !re.is_match(local) {
                out.error(
                    &at,
                    format!(
                        "[search] fixture `{query}` yields `{}`, which [scheme.{scheme}] pattern does not admit",
                        c.identifier
                    ),
                );
            }
        }
    }
}

/// Admit each transform, run it over its scheme's fixtures, and check that every `then.from`
/// a scheme declares is an identifier its describe yields for at least one of them.
///
/// At least one and not every one: a DOI's metadata names a PMCID only when there is one, so
/// a fixture without it is a fact about that paper. A key no fixture yields is a misspelling or
/// a transform that never sets it, and either way the `then` resolves nothing.
fn check_transforms(root: &Path, pack: &Pack, m: &Manifest, out: &mut Findings<'_>) {
    use transform::{Kind, Transform};

    let declared = m
        .scheme
        .values()
        .any(|s| s.describe.is_some() || s.extract.is_some());
    if !declared {
        return;
    }
    if !transform::AVAILABLE {
        out.info(
            "pack.toml",
            format!("transforms not checked: {}", transform::UNAVAILABLE),
        );
        return;
    }
    for (name, s) in &m.scheme {
        let fixtures: Vec<(&String, &String)> = m
            .fixtures
            .iter()
            .filter(|(id, _)| id.split_once(':').is_some_and(|(sc, _)| sc == name))
            .collect();
        let mut yielded: BTreeSet<String> = BTreeSet::new();
        let mut described = false;
        for (path, kind) in [(&s.describe, Kind::Describe), (&s.extract, Kind::Extract)] {
            let Some(path) = path else { continue };
            // A path `check_pack` refused, or a file it found missing, is already reported.
            let Ok(t) = Transform::read(root, pack, m, path) else {
                continue;
            };
            if let Err(why) = transform::admit(&t, kind) {
                out.error(path, why);
                continue;
            }
            if fixtures.is_empty() {
                continue;
            }
            let Some(media) = &s.resolve.media else {
                out.error(
                    "pack.toml",
                    format!(
                        "[scheme.{name}] resolve declares no media, so its fixtures cannot be parsed for {path}"
                    ),
                );
                continue;
            };
            for (id, file) in &fixtures {
                let Ok(bytes) = std::fs::read(root.join(&pack.dir).join("fixtures").join(file))
                else {
                    continue;
                };
                let at = format!("fixtures/{file}");
                match kind {
                    Kind::Describe => match transform::describe(root, pack, m, s, media, &bytes) {
                        Ok(d) if d.by.is_some() => {
                            described = true;
                            yielded.extend(d.identifiers.into_iter().map(|(k, _)| k));
                        }
                        Ok(d) => out.error(&at, format!("`{id}`: {}", d.why.unwrap_or_default())),
                        Err(e) => out.error(&at, format!("`{id}`: {e:#}")),
                    },
                    Kind::Extract => {
                        if let Err(why) = transform::extract(&t, media, &bytes) {
                            out.error(&at, format!("`{id}`: {why}"));
                        }
                    }
                }
            }
        }
        if !described {
            continue;
        }
        for then in &s.then {
            let Some(key) = then.from.strip_prefix("describe.") else {
                continue;
            };
            if !yielded.contains(key) {
                out.error(
                    "pack.toml",
                    format!(
                        "[scheme.{name}] then takes `{}`, and its describe yields no identifier `{key}` for any fixture",
                        then.from
                    ),
                );
            }
        }
    }
}

/// Every vendored pack has a pin, and every pin a vendored pack that satisfies it.
fn check_pins(root: &Path, packs: &[Pack], findings: &mut Vec<Finding>) -> Result<()> {
    let record = root.join(PROPOSALS);
    let declared = if record.is_file() {
        let text =
            std::fs::read_to_string(&record).with_context(|| format!("reading {PROPOSALS}"))?;
        match pin::declared(&text) {
            Ok(d) => d.unwrap_or_default(),
            Err(e) => {
                findings.push(Finding {
                    severity: Severity::Error,
                    pack: "prelude_sources".into(),
                    path: PROPOSALS.into(),
                    message: e,
                });
                return Ok(());
            }
        }
    } else {
        Vec::new()
    };

    let at_record = |pack: &str, message: String| Finding {
        severity: Severity::Error,
        pack: pack.to_string(),
        path: PROPOSALS.into(),
        message,
    };

    let mut pins: BTreeMap<String, Pin> = BTreeMap::new();
    for pin in declared {
        match pin {
            Err(e) => findings.push(at_record("prelude_sources", e)),
            Ok(pin) if pins.contains_key(&pin.pack) => findings.push(at_record(
                &pin.pack,
                format!("`{}` is pinned twice; one pin per pack", pin.pack),
            )),
            Ok(pin) => {
                pins.insert(pin.pack.clone(), pin);
            }
        }
    }

    let vendored: BTreeMap<&str, &Pack> = packs
        .iter()
        .filter(|p| p.origin == Origin::Vendored)
        .map(|p| (p.name.as_str(), p))
        .collect();

    for (name, pin) in &pins {
        let Some(pack) = vendored.get(name.as_str()) else {
            findings.push(at_record(
                name,
                format!("`{pin}` is pinned and {VENDORED}/{name} does not exist; run `mise run yidam-vendor-update`"),
            ));
            continue;
        };
        let Ok(m) = &pack.manifest else { continue };
        let manifest = slash(&pack.dir.join("pack.toml"));
        if let Some(v) = Version::parse(&m.pack.version) {
            if !pin.range.admits(v) {
                findings.push(Finding {
                    severity: Severity::Error,
                    pack: name.clone(),
                    path: manifest.clone(),
                    message: format!(
                        "vendored at {v}, which `{pin}` does not admit; re-vendor, or change the pin"
                    ),
                });
            }
        }
        if let Some(record) = &m.vendored {
            if record.pin != pin.to_string() {
                findings.push(Finding {
                    severity: Severity::Error,
                    pack: name.clone(),
                    path: manifest,
                    message: format!(
                        "vendored for `{}`, and {PROPOSALS} now pins `{pin}`; run `mise run yidam-vendor-update`",
                        record.pin
                    ),
                });
            }
        }
    }
    for (name, pack) in &vendored {
        if !pins.contains_key(*name) {
            findings.push(Finding {
                severity: Severity::Error,
                pack: name.to_string(),
                path: slash(&pack.dir),
                message: "vendored and not pinned in prelude_sources; pin it, or let `mise run yidam-vendor-update` remove it"
                        .to_string(),
            });
        }
    }
    Ok(())
}

/// A scheme is the prefix of an identifier. `http` and `https` are refused for the reason
/// `catalog-location-malformed` refuses them: such a value is a URL, not an identifier.
fn is_scheme_name(s: &str) -> bool {
    pin::is_pack_name(s) && s != "http" && s != "https"
}

fn is_env_name(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_uppercase() || b == b'_')
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Files under `dir`, as `/`-separated paths relative to it, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(dir) {
                out.push(slash(rel));
            }
        }
    }
    out.sort();
    out
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}
