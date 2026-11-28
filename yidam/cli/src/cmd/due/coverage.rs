//! The seventh clock: is every member of a declared universe covered by an instance? (#1491)
//!
//! Every other clock measures something already in the repository — a source's age, a
//! question's, a phase's, a version a node has not read. A thing the corpus has never heard of
//! has no node, no catalog entry and no question, so it is invisible to all six. The prelude's
//! "add a node when an edge needs a target" norm leaves exactly this gap: nothing creates the
//! edge, so nothing creates the node, so nothing is due.
//!
//! The derivation that reported it tracked Ohio legislation. It held 28 bill nodes against a
//! session list of 1,555, and one of the bills it did not hold had passed both chambers a week
//! before anyone noticed. The list was already in its catalog, fetched and cached.
//!
//! # What a universe is
//!
//! A `[[due.coverage]]` table: a catalog entry whose newest artifact enumerates identifiers, a
//! selector into it (a JSON path or a CSV column), and the class and property that should
//! carry each one. A member is covered when some instance of the class holds it under the
//! property, compared ignoring case and everything that is not a letter or a digit.
//!
//! # A registry kept as code (RFC-0051 §6)
//!
//! A path walks TOML as it walks JSON, when the name the bytes were read under ends in
//! `.toml`. An entry with a pinned `file` location is read from that file at the pin's
//! `commit`, from git and not the cache, so nothing is fetched. Such an entry has no TTL. It
//! is stale when its file changed after the read, the comparison the catalog clock makes.
//!
//! `orphans = true` asks the other direction too: an instance whose key no member names,
//! left behind when the registry dropped it. Only a fit list can say that. A stale list may
//! simply predate the key, and an empty one would make every instance an orphan, so an unfit
//! reading reports its orphans as not read. `except` leaves out instances kept on purpose,
//! and counts them.
//!
//! # Narrowing a list, and scoping a class (#1495)
//!
//! A published list is often wider than what a corpus covers, and a class often repeats its
//! identifiers across periods. The first derivation to adopt this clock hit both: an assembly's
//! legislation list mixes bills with resolutions the corpus leaves out, and bill numbers
//! restart every assembly. It worked round each locally — a list its own fetcher filtered, and
//! a synthetic property unique across assemblies — and neither workaround was visible to
//! anyone else.
//!
//! `matches` narrows the members: an identifier that does not match the pattern is outside
//! the universe. It filters *identifiers*, not the document, so the selector stays a walk and
//! not a program; and what it excluded is counted in the report, so a pattern that drops too
//! much is seen. `scope` narrows the instances: only one carrying the named edge to the named
//! node covers a member. A scope whose target is no node is unmeasurable, not a universe in
//! which everything is missing.
//!
//! # What only a reading of zero has to prove
//!
//! **A gap is owed whatever the age of the list that showed it.** A member listed in last
//! month's artifact and covered by no node is still uncovered, and a newer list can only add
//! to it. So a stale or empty source does not make a non-zero gap unmeasurable.
//!
//! **A gap of zero is a claim about the list, and the list has to be fit to make it.** A
//! source past its TTL reports *stale coverage*, not zero gap. An artifact that lists nothing
//! is not a universe that is covered — it is the second instance the report found, a meetings
//! list filed empty under a 3650-day TTL, whose hearing nobody saw. Empty is not settled.
//!
//! # Reports, never gates
//!
//! Like every clock here, `due` exits zero on it unless `--strict`. The gap is a worklist and
//! not a defect; a corpus that has narrowed what it means to cover changes the declaration.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

use super::{Actor, Clock, State};
use crate::config::CoverageConfig;
use crate::corpus::Node;
use crate::parse::{parse_frontmatter, ArtifactOrigin, CatalogArtifact, CatalogLocation};
use crate::vault::{Cache, ContentHash};

/// What one universe read as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Universe {
    /// `source → class.property`, the name a reader recognises from the declaration.
    pub label: String,
    /// The catalog entry's file stem, once found.
    pub entry: String,
    /// The class and property the declaration names, trimmed.
    pub class: String,
    pub property: String,
    /// The edge a covering instance must carry, as `relationship → target`, when the
    /// declaration scopes the class.
    pub scope: Option<String>,
    pub reading: Reading,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reading {
    /// The artifact was read and compared.
    Measured {
        /// Distinct members, in the order the artifact lists them.
        listed: usize,
        /// Listed members no instance carries, verbatim as the artifact gives them.
        missing: Vec<String>,
        /// Distinct identifiers the artifact lists that `matches` puts outside the universe.
        excluded: usize,
        /// Why a gap of zero here would not be a claim of coverage — the source is past its
        /// TTL, or the artifact lists nothing. `None` when the reading is fit to say zero.
        unfit: Option<String>,
        /// The reverse direction, when the declaration asks for it.
        orphans: Orphans,
    },
    /// Nothing could be compared, and why.
    Unread(String),
}

/// Instances of the class whose property names no member (RFC-0051 §6, #1526).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Orphans {
    /// The declaration does not set `orphans = true`.
    Undeclared,
    /// The list is unfit to say what it no longer holds; see the module header.
    Unread,
    Found {
        /// One line per orphan: the node and the identifier it carries.
        nodes: Vec<String>,
        /// Orphans `except` left out, counted so an `except` that hides too much is seen.
        kept: usize,
    },
}

/// The key a member is compared under: lowercase letters and digits, nothing else.
///
/// Identifiers in a published list and identifiers a person typed into a node differ in
/// exactly the ways this drops — `SB 214`, `S.B. 214`, `sb-214`. A key with nothing left is no
/// key, and its member is skipped rather than matched against every other empty one.
pub(crate) fn key(raw: &str) -> Option<String> {
    let k: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    (!k.is_empty()).then_some(k)
}

// ── selecting members out of an artifact ──────────────────────────────────────

/// One step of a JSON path.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(String),
    Index(usize),
    All,
}

/// Parse the subset of JSONPath this reads: an optional leading `$`, then `.key`, `.*`,
/// `['key']`, `["key"]`, `[n]` and `[*]`.
///
/// A subset on purpose. A universe is a list of identifiers somewhere in a document, and every
/// path to one is a walk down keys and across arrays. Filters and recursive descent would make
/// the declaration a program, and a wrong one would select silently.
fn parse_path(path: &str) -> Result<Vec<Step>, String> {
    let s = path.trim();
    let (rooted, s) = match s.strip_prefix('$') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let b = s.as_bytes();
    let mut steps = Vec::new();
    let mut i = 0;
    let bad = |why: &str| format!("selector `{path}` {why}");
    while i < b.len() {
        match b[i] {
            b'.' => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != b'.' && b[j] != b'[' {
                    j += 1;
                }
                let name = &s[start..j];
                match name {
                    "" => return Err(bad("has an empty key after `.`")),
                    "*" => steps.push(Step::All),
                    _ => steps.push(Step::Key(name.to_string())),
                }
                i = j;
            }
            b'[' => {
                let close = s[i..]
                    .find(']')
                    .map(|n| i + n)
                    .ok_or_else(|| bad("opens a `[` it never closes"))?;
                let inner = s[i + 1..close].trim();
                let quoted = inner.len() >= 2
                    && ((inner.starts_with('\'') && inner.ends_with('\''))
                        || (inner.starts_with('"') && inner.ends_with('"')));
                if inner == "*" {
                    steps.push(Step::All);
                } else if quoted {
                    steps.push(Step::Key(inner[1..inner.len() - 1].to_string()));
                } else if let Ok(n) = inner.parse::<usize>() {
                    steps.push(Step::Index(n));
                } else {
                    return Err(bad(&format!(
                        "has `[{inner}]`, which is not `[*]`, an index or a quoted key"
                    )));
                }
                i = close + 1;
            }
            // A bare first key, as in `items[*].number` — but not glued to a `$`.
            _ if i == 0 && !rooted => {
                let mut j = 0;
                while j < b.len() && b[j] != b'.' && b[j] != b'[' {
                    j += 1;
                }
                steps.push(Step::Key(s[..j].to_string()));
                i = j;
            }
            c => {
                return Err(bad(&format!(
                    "has `{}` where a `.` or `[` belongs",
                    c as char
                )))
            }
        }
    }
    Ok(steps)
}

/// The scalars a path reaches, in document order. An array at the end of the path is read
/// as its elements, so `$.numbers` and `$.numbers[*]` select the same members.
fn select_json(doc: &serde_json::Value, steps: &[Step]) -> Vec<String> {
    let mut here: Vec<&serde_json::Value> = vec![doc];
    for step in steps {
        here = here
            .into_iter()
            .flat_map(|v| -> Vec<&serde_json::Value> {
                match (step, v) {
                    (Step::Key(k), serde_json::Value::Object(m)) => m.get(k).into_iter().collect(),
                    (Step::Index(n), serde_json::Value::Array(a)) => {
                        a.get(*n).into_iter().collect()
                    }
                    (Step::All, serde_json::Value::Array(a)) => a.iter().collect(),
                    (Step::All, serde_json::Value::Object(m)) => m.values().collect(),
                    _ => Vec::new(),
                }
            })
            .collect();
    }
    let mut out = Vec::new();
    for v in here {
        match v {
            serde_json::Value::Array(a) => out.extend(a.iter().filter_map(scalar)),
            v => out.extend(scalar(v)),
        }
    }
    out
}

fn scalar(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.trim().to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The values of one CSV column, by its header, in row order.
fn select_csv(bytes: &[u8], column: &str) -> Result<Vec<String>, String> {
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|e| format!("does not read as CSV: {e}"))?
        .clone();
    let want = column.trim();
    let Some(at) = headers.iter().position(|h| h.trim() == want) else {
        let names: Vec<String> = headers.iter().map(|h| format!("`{}`", h.trim())).collect();
        return Err(format!(
            "has no column `{want}`; its columns are {}",
            names.join(", ")
        ));
    };
    let mut out = Vec::new();
    for row in reader.records() {
        let row = row.map_err(|e| format!("does not read as CSV: {e}"))?;
        if let Some(v) = row.get(at).map(str::trim).filter(|v| !v.is_empty()) {
            out.push(v.to_string());
        }
    }
    Ok(out)
}

/// How a universe's bytes are parsed when `select` walks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Toml,
}

impl Format {
    /// TOML when the name the bytes were read under ends in `.toml`, JSON otherwise (RFC-0051
    /// §6). No selector field says which: the file's own name already does.
    fn of(names: &[Option<&str>]) -> Self {
        let toml = names.iter().flatten().any(|n| {
            let n = n.trim();
            let n = n.split(['?', '#']).next().unwrap_or(n);
            n.to_ascii_lowercase().ends_with(".toml")
        });
        if toml {
            Self::Toml
        } else {
            Self::Json
        }
    }
}

/// The members a universe's artifact lists, from its bytes.
fn members(bytes: &[u8], decl: &CoverageConfig, format: Format) -> Result<Vec<String>, String> {
    match (decl.select.as_deref(), decl.column.as_deref()) {
        (Some(_), Some(_)) => Err("declares both `select` and `column`; a universe is read \
                                   one way — `select` for JSON or TOML, `column` for CSV"
            .to_string()),
        (None, None) => Err(
            "declares neither `select` (JSON or TOML) nor `column` (CSV), so \
                             nothing says where its members are"
                .to_string(),
        ),
        (Some(path), None) => {
            let steps = parse_path(path)?;
            let doc: serde_json::Value = match format {
                Format::Json => serde_json::from_slice(bytes)
                    .map_err(|e| format!("does not read as JSON: {e}"))?,
                Format::Toml => {
                    let text = std::str::from_utf8(bytes)
                        .map_err(|e| format!("does not read as TOML: {e}"))?;
                    let table: toml::Table = toml::from_str(text)
                        .map_err(|e| format!("does not read as TOML: {}", e.message()))?;
                    serde_json::to_value(table)
                        .map_err(|e| format!("does not read as TOML: {e}"))?
                }
            };
            Ok(select_json(&doc, &steps))
        }
        (None, Some(column)) => select_csv(bytes, column),
    }
}

// ── reading a universe ────────────────────────────────────────────────────────

/// The record a universe reads: the last one in the entry's list that carries a digest and,
/// when the declaration names a resource, names the same one.
///
/// Last in list order is newest. Records are appended as they are fetched
/// (`catalog::record::append_artifacts`), and the list is never re-sorted.
fn newest<'a>(
    artifacts: &'a [CatalogArtifact],
    resource: Option<&str>,
) -> Option<&'a CatalogArtifact> {
    let want = resource.map(str::trim).filter(|r| !r.is_empty());
    artifacts.iter().rev().find(|a| {
        let has_digest = a.sha256.as_deref().is_some_and(|h| !h.trim().is_empty());
        let named = a
            .resource
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty());
        has_digest && (want.is_none() || named == want)
    })
}

/// The pinned `file` location a universe reads, when its entry has one: the one whose path is
/// the declaration's `resource` when it names one, else the first.
fn pinned<'a>(
    locations: &'a [CatalogLocation],
    resource: Option<&str>,
) -> Option<&'a CatalogLocation> {
    let mut pins = locations
        .iter()
        .filter(|l| l.is_pinned() && l.kind.as_deref() == Some("file"));
    match resource.map(str::trim).filter(|r| !r.is_empty()) {
        None => pins.next(),
        Some(r) => {
            let r = r.strip_prefix("./").unwrap_or(r);
            pins.find(|l| {
                let v = l.value.as_deref().unwrap_or_default().trim();
                v.strip_prefix("./").unwrap_or(v) == r
            })
        }
    }
}

/// What a universe read: its bytes, how a reader names them, and how they parse.
type Read = (Vec<u8>, String, Format);

/// Read a pinned registry from its file at the pin's `commit` (RFC-0051 §6). Nothing is
/// fetched or cached: the commit is the read, so the figure and the catalog clock describe
/// the same bytes.
fn read_pin(root: &Path, entry: &str, loc: &CatalogLocation) -> Result<Read, String> {
    use crate::cmd::lint::pins;
    let (commit, file) = pins::shape(loc).map_err(|why| format!("`{entry}`'s pin: {why}"))?;
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root))
        .map_err(|e| format!("`{entry}`'s pin cannot be read: {e:#}"))?;
    let dir = match pins::place(root, loc.repo.as_deref(), &config) {
        pins::Place::Dir(d) => d,
        pins::Place::Unchecked(why) | pins::Place::Unresolved(why) => {
            return Err(format!("`{entry}`'s pin cannot be read: {why}"))
        }
    };
    let short = commit.get(..7).unwrap_or(&commit);
    let what = match loc.repo.as_deref() {
        Some(r) => format!("{r}:{file} at {short}"),
        None => format!("{file} at {short}"),
    };
    let query = pins::Query {
        commit: commit.clone(),
        path: file.clone(),
        contents: true,
    };
    let answer = pins::resolve(&dir, &[query])
        .and_then(|a| a.into_iter().next())
        .ok_or_else(|| format!("`{entry}`'s pin ({what}) could not be read: git did not run"))?;
    let Some(text) = answer.text else {
        return Err(format!(
            "`{entry}`'s pin names {what}, which git does not hold; `yidam lint` says why"
        ));
    };
    Ok((text.into_bytes(), what, Format::of(&[Some(&file)])))
}

/// Read a universe from its entry's newest cached artifact.
fn read_artifact(
    entry: &str,
    fm: &crate::parse::Frontmatter,
    locations: &[CatalogLocation],
    resource: Option<&str>,
    cache: Option<&Cache>,
) -> Result<Read, String> {
    let Some(record) = newest(fm.artifacts.as_deref().unwrap_or_default(), resource) else {
        let which = match resource {
            Some(r) => format!(" for resource `{}`", r.trim()),
            None => String::new(),
        };
        return Err(format!(
            "`{entry}` records no artifact{which}; `yidam catalog-fetch {entry}` obtains one"
        ));
    };
    let digest = record.sha256.as_deref().unwrap_or_default().trim();
    let short = digest.get(..12).unwrap_or(digest);
    let Ok(hash) = ContentHash::parse(digest) else {
        return Err(format!(
            "`{entry}`'s newest artifact carries `{short}`, which is not a sha256"
        ));
    };
    let bytes = cache
        .map(|c| c.path_of(&hash))
        .and_then(|p| std::fs::read(p).ok());
    let Some(bytes) = bytes else {
        return Err(format!(
            "the bytes of `{entry}` (sha256:{short}) are not in the local cache; \
             `yidam vault pull` or `yidam catalog-fetch {entry}` fetches them"
        ));
    };
    let origin = match &record.from {
        Some(ArtifactOrigin::Location(i)) => locations.get(*i).and_then(|l| l.value.as_deref()),
        Some(ArtifactOrigin::Url(u)) => Some(u.as_str()),
        None => None,
    };
    let media = record
        .media_type
        .as_deref()
        .filter(|m| m.to_ascii_lowercase().contains("toml"))
        .map(|_| ".toml");
    let format = Format::of(&[record.resource.as_deref(), origin, media]);
    Ok((bytes, format!("sha256:{short}"), format))
}

/// Read one universe against the corpus.
///
/// `instances` is every node, and `stale` answers whether an entry (by its repo-relative path)
/// is past its TTL — both passed in so that seven universes over one corpus are one walk and
/// one TTL read, not seven.
pub(crate) fn read_universe(
    root: &Path,
    decl: &CoverageConfig,
    instances: &[Node],
    stale: &dyn Fn(&str) -> Option<String>,
    cache: Option<&Cache>,
) -> Universe {
    let class = decl.class.trim().to_string();
    let property = decl.property.trim().to_string();
    let scope = decl.scope.as_ref().map(|s| {
        format!(
            "{} → {}",
            s.relationship.trim(),
            s.target.trim().trim_start_matches("./")
        )
    });
    // The scope is part of the name: one table per period shares its source, class and
    // property with the next, and two lines reading alike would not say which is which.
    let label = format!(
        "{} → {}.{}{}",
        decl.source.trim(),
        decl.class.trim(),
        decl.property.trim(),
        scope
            .as_ref()
            .map(|s| format!(" with `{s}`"))
            .unwrap_or_default()
    );
    let unread = |entry: String, why: String| Universe {
        label: label.clone(),
        entry,
        class: class.clone(),
        property: property.clone(),
        scope: scope.clone(),
        reading: Reading::Unread(why),
    };

    // The two narrowings are checked before the artifact is, so a typo in either is reported
    // as itself rather than as a gap: a pattern that does not compile, or a scope naming a
    // node this corpus does not hold, would otherwise read every member as missing.
    let pattern = match decl.matches.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(p) => match regex::Regex::new(p) {
            Ok(re) => Some(re),
            Err(e) => {
                return unread(
                    String::new(),
                    format!("`matches = \"{p}\"` is not a regular expression: {e}"),
                )
            }
        },
    };
    let scope_target = match &decl.scope {
        None => None,
        Some(s) => {
            let rel = s.relationship.trim();
            let target = s.target.trim();
            if rel.is_empty() || target.is_empty() {
                return unread(
                    String::new(),
                    "`scope` needs both a `relationship` and a `target`".to_string(),
                );
            }
            let at =
                crate::corpus::edges::normalize(&crate::paths::yidam_corpus_dir(root).join(target));
            if !instances
                .iter()
                .any(|n| crate::corpus::edges::normalize(&n.path) == at)
            {
                return unread(
                    String::new(),
                    format!("`scope` targets `{target}`, which is no node under .yidam/corpus/"),
                );
            }
            Some((rel, at))
        }
    };
    let mut except = Vec::new();
    for t in decl.except.iter().flat_map(|e| e.tables()) {
        let field = t.field.trim();
        if field.is_empty() {
            return unread(
                String::new(),
                "an `except` table needs a `field`: a property name, or `label`".to_string(),
            );
        }
        match regex::Regex::new(t.matches.trim()) {
            Ok(re) => except.push((field.to_string(), re)),
            Err(e) => {
                return unread(
                    String::new(),
                    format!(
                        "`except` on `{field}` has `matches = \"{}\"`, which is not a regular \
                         expression: {e}",
                        t.matches.trim()
                    ),
                )
            }
        }
    }
    let in_scope = |n: &Node| -> bool {
        let Some((rel, at)) = &scope_target else {
            return true;
        };
        n.inst.links.as_deref().unwrap_or_default().iter().any(|l| {
            l.relationship.as_deref().map(str::trim) == Some(*rel)
                && l.target
                    .as_deref()
                    .is_some_and(|t| crate::corpus::edges::resolve_target(&n.path, t.trim()) == *at)
        })
    };

    let catalog = crate::paths::yidam_catalog_dir(root);
    let found = crate::cmd::catalog::select_entries(&catalog, Some(&decl.source))
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.file_name().is_some_and(|n| n != "README.md"));
    let Some(path) = found else {
        return unread(
            String::new(),
            format!(
                "names no catalog entry `{}` — by file stem or `name:`",
                decl.source.trim()
            ),
        );
    };
    let entry = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let rel = crate::paths::slash(path.strip_prefix(root).unwrap_or(&path));

    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let fm = parse_frontmatter(&text);
    let locations = fm.location.as_deref().unwrap_or_default();
    let read = match pinned(locations, decl.resource.as_deref()) {
        Some(loc) => read_pin(root, &entry, loc),
        None => read_artifact(&entry, &fm, locations, decl.resource.as_deref(), cache),
    };
    let (bytes, what, format) = match read {
        Ok(r) => r,
        Err(why) => return unread(entry.clone(), why),
    };

    let listed = match members(&bytes, decl, format) {
        Ok(m) => m,
        Err(why) => return unread(entry.clone(), format!("`{entry}` ({what}) {why}")),
    };

    let candidates: Vec<&Node> = instances
        .iter()
        .filter(|n| crate::paths::class_of_path(&n.path) == class)
        .filter(|n| in_scope(n))
        .collect();
    let covered: HashSet<String> = candidates
        .iter()
        .flat_map(|n| crate::cmd::query::exec::node_scalars(n, &property))
        .filter_map(|v| key(&v))
        .collect();

    let mut seen = HashSet::new();
    let mut distinct = 0usize;
    let mut excluded = 0usize;
    let mut missing = Vec::new();
    let mut universe = HashSet::new();
    for raw in listed {
        let Some(k) = key(&raw) else { continue };
        if !seen.insert(k.clone()) {
            continue;
        }
        if pattern.as_ref().is_some_and(|re| !re.is_match(&raw)) {
            excluded += 1;
            continue;
        }
        distinct += 1;
        universe.insert(k.clone());
        if !covered.contains(&k) {
            missing.push(raw);
        }
    }

    let unfit = match (distinct, stale(&rel)) {
        (0, _) if excluded > 0 => Some(format!(
            "`matches` puts all {excluded} of `{entry}`'s identifiers outside the universe — \
             empty is not settled"
        )),
        (0, _) => Some(format!(
            "`{entry}` ({what}) lists no members — empty is not settled"
        )),
        (_, Some(why)) => Some(why),
        _ => None,
    };
    let orphans = match (decl.orphans, &unfit) {
        (false, _) => Orphans::Undeclared,
        (true, Some(_)) => Orphans::Unread,
        (true, None) => {
            let mut nodes = Vec::new();
            let mut kept = 0usize;
            for n in &candidates {
                let values = crate::cmd::query::exec::node_scalars(n, &property);
                // A value `matches` puts outside the universe names nothing the list could
                // hold, so its instance is not an orphan of this list.
                let named: Vec<&String> = values
                    .iter()
                    .filter(|v| key(v).is_some())
                    .filter(|v| pattern.as_ref().is_none_or(|re| re.is_match(v.trim())))
                    .collect();
                let Some(first) = named.first() else { continue };
                if named
                    .iter()
                    .any(|v| key(v).is_some_and(|k| universe.contains(&k)))
                {
                    continue;
                }
                if except.iter().any(|(field, re)| excepted(n, field, re)) {
                    kept += 1;
                    continue;
                }
                let at = crate::paths::slash_str(&n.rel);
                let at = at.strip_prefix(".yidam/corpus/").unwrap_or(&at).to_string();
                nodes.push(format!(
                    "{at} — its `{property}` `{}` is no member of {entry}",
                    first.trim()
                ));
            }
            Orphans::Found { nodes, kept }
        }
    };
    Universe {
        label,
        entry,
        class,
        property,
        scope,
        reading: Reading::Measured {
            listed: distinct,
            missing,
            excluded,
            unfit,
            orphans,
        },
    }
}

/// Whether an `except` table leaves `n` out: some value of `field` matches. `label` is the
/// node's label, because a corpus may mark a retired instance nowhere else.
fn excepted(n: &Node, field: &str, re: &regex::Regex) -> bool {
    if field == "label" {
        return n
            .inst
            .label
            .as_deref()
            .is_some_and(|l| re.is_match(l.trim()));
    }
    crate::cmd::query::exec::node_scalars(n, field)
        .iter()
        .any(|v| re.is_match(v.trim()))
}

/// Fold every universe into the one clock.
pub(crate) fn clock(universes: &[Universe]) -> Clock {
    const Q: &str = "Is every member of a declared universe covered by an instance?";
    if universes.is_empty() {
        return Clock::new(
            Clock::COVERAGE,
            Q,
            State::Undeclared,
            "no universe declared",
        )
        .unset(
            "`[[due.coverage]]` in .yidam/config.toml — a catalog entry that lists what this \
             corpus means to cover, and the class that should cover it",
        );
    }
    let interval = match universes.len() {
        1 => "[[due.coverage]]".to_string(),
        n => format!("[[due.coverage]] × {n}"),
    };

    let mut parts = Vec::new();
    let mut subjects = Vec::new();
    let mut unmeasured = 0usize;
    let mut orphaned = 0usize;
    for u in universes {
        match &u.reading {
            Reading::Unread(why) => {
                unmeasured += 1;
                parts.push(format!("{}: {why}", u.label));
            }
            Reading::Measured {
                listed,
                missing,
                excluded,
                unfit,
                orphans,
            } => {
                let mut p = format!(
                    "{}: {} of {listed} covered, {} missing",
                    u.label,
                    listed - missing.len(),
                    missing.len()
                );
                if *excluded > 0 {
                    let _ = write!(p, ", {excluded} listed outside `matches`");
                }
                match orphans {
                    Orphans::Undeclared => {}
                    Orphans::Unread => p.push_str(", orphans not read"),
                    Orphans::Found { nodes, kept } => {
                        let _ = write!(p, ", {} orphan(s)", nodes.len());
                        if *kept > 0 {
                            let _ = write!(p, ", {kept} kept by `except`");
                        }
                        orphaned += nodes.len();
                    }
                }
                if let Some(why) = unfit {
                    // Only a zero needs the list to be fit; see the module header.
                    if missing.is_empty() {
                        unmeasured += 1;
                    }
                    let _ = write!(p, " ({why})");
                }
                parts.push(p);
                let within = u
                    .scope
                    .as_ref()
                    .map(|s| format!(" with `{s}`"))
                    .unwrap_or_default();
                subjects.extend(missing.iter().map(|m| {
                    format!(
                        "{m} — listed by {}, carried by no `{}`{within} as `{}`",
                        u.entry, u.class, u.property
                    )
                }));
                if let Orphans::Found { nodes, .. } = orphans {
                    subjects.extend(nodes.iter().cloned());
                }
            }
        }
    }
    let detail = parts.join("; ");

    if !subjects.is_empty() {
        let remedy = if orphaned == 0 {
            "add an instance for each member below, or narrow the `[[due.coverage]]` that \
             lists it — the gap is a worklist, not a defect"
        } else {
            "add an instance for each member below, re-key or name in `except` each node whose \
             key no member names, or narrow the `[[due.coverage]]` — the gap is a worklist, \
             not a defect"
        };
        let mut clock = Clock::new(Clock::COVERAGE, Q, State::Due, detail)
            .at(interval)
            // Not `propose`. Whether a listed member earns a node — and what it says — is the
            // judgement; a drafted stub per member would discharge the clock with nothing
            // read. A corpus that means to cover less narrows the declaration instead.
            .owing(subjects.len(), remedy, Actor::Person);
        clock.subjects = subjects;
        return clock;
    }
    let state = match unmeasured {
        0 => State::Ok,
        _ => State::Unmeasurable,
    };
    Clock::new(Clock::COVERAGE, Q, state, detail).at(interval)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(doc: &str, path: &str) -> Vec<String> {
        let doc: serde_json::Value = serde_json::from_str(doc).unwrap();
        select_json(&doc, &parse_path(path).unwrap())
    }

    #[test]
    fn a_path_walks_keys_and_arrays() {
        let doc = r#"{"data":{"bills":[{"n":"HB 1"},{"n":"SB 2"},{"x":1}],"count":2}}"#;
        assert_eq!(pick(doc, "$.data.bills[*].n"), ["HB 1", "SB 2"]);
        assert_eq!(pick(doc, "data.bills[*].n"), ["HB 1", "SB 2"]);
        assert_eq!(pick(doc, "$['data'][\"bills\"][1].n"), ["SB 2"]);
        assert_eq!(pick(doc, "$.data.count"), ["2"]);
        assert!(pick(doc, "$.data.nothing[*]").is_empty());
    }

    #[test]
    fn an_array_at_the_end_is_its_elements() {
        assert_eq!(pick(r#"{"ids":["a",3,null,{"k":1}]}"#, "$.ids"), ["a", "3"]);
        assert_eq!(pick(r#"["a","b"]"#, "$"), ["a", "b"]);
        assert_eq!(
            pick(r#"{"a":{"x":"1"},"b":{"x":"2"}}"#, "$.*.x"),
            ["1", "2"]
        );
    }

    #[test]
    fn a_path_outside_the_subset_is_refused_with_its_reason() {
        for (path, says) in [
            ("$.a[", "never closes"),
            ("$.a[?(@.x)]", "not `[*]`"),
            ("$..a", "empty key"),
            ("$a", "where a `.` or `[` belongs"),
        ] {
            let err = parse_path(path).unwrap_err();
            assert!(err.contains(says), "{path}: {err}");
        }
    }

    /// city-engine's registry shape: `[[entry]]` tables, walked with the path JSON uses.
    #[test]
    fn a_path_walks_a_toml_registry() {
        let decl = CoverageConfig {
            select: Some("$.entry[*].id".into()),
            ..Default::default()
        };
        let toml = b"[[entry]]\nid = \"build-period\"\nvalue = 3\n\n[[entry]]\nid = \"cap-rate\"\n";
        assert_eq!(
            members(toml, &decl, Format::Toml).unwrap(),
            ["build-period", "cap-rate"]
        );
        let err = members(b"{not toml", &decl, Format::Toml).unwrap_err();
        assert!(err.starts_with("does not read as TOML"), "{err}");
        // Bytes named as JSON are still read as JSON.
        assert!(members(toml, &decl, Format::Json)
            .unwrap_err()
            .contains("JSON"));
    }

    #[test]
    fn a_name_ending_in_toml_reads_as_toml() {
        assert_eq!(Format::of(&[Some("crates/x/defaults.TOML")]), Format::Toml);
        assert_eq!(
            Format::of(&[None, Some("https://x.test/a.toml?raw=1")]),
            Format::Toml
        );
        assert_eq!(Format::of(&[Some("list.json"), None]), Format::Json);
        assert_eq!(Format::of(&[]), Format::Json);
    }

    #[test]
    fn a_key_ignores_case_and_punctuation() {
        assert_eq!(key("S.B. 214"), key("sb214"));
        assert_eq!(key("SB-214"), key("sb 214"));
        assert_ne!(key("SB 214"), key("HB 214"));
        assert_eq!(key(" - "), None);
    }
}
