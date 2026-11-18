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
use crate::parse::{parse_frontmatter, CatalogArtifact};
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
        /// Why a gap of zero here would not be a claim of coverage — the source is past its
        /// TTL, or the artifact lists nothing. `None` when the reading is fit to say zero.
        unfit: Option<String>,
    },
    /// Nothing could be compared, and why.
    Unread(String),
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

/// The members a universe's artifact lists, from its bytes.
fn members(bytes: &[u8], decl: &CoverageConfig) -> Result<Vec<String>, String> {
    match (decl.select.as_deref(), decl.column.as_deref()) {
        (Some(_), Some(_)) => Err("declares both `select` and `column`; a universe is read \
                                   one way — `select` for JSON, `column` for CSV"
            .to_string()),
        (None, None) => Err("declares neither `select` (JSON) nor `column` (CSV), so \
                             nothing says where its members are"
            .to_string()),
        (Some(path), None) => {
            let steps = parse_path(path)?;
            let doc: serde_json::Value =
                serde_json::from_slice(bytes).map_err(|e| format!("does not read as JSON: {e}"))?;
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
    let label = format!(
        "{} → {}.{}",
        decl.source.trim(),
        decl.class.trim(),
        decl.property.trim()
    );
    let class = decl.class.trim().to_string();
    let property = decl.property.trim().to_string();
    let unread = |entry: String, why: String| Universe {
        label: label.clone(),
        entry,
        class: class.clone(),
        property: property.clone(),
        reading: Reading::Unread(why),
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
    let rel = path
        .strip_prefix(root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");

    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let fm = parse_frontmatter(&text);
    let Some(record) = newest(
        fm.artifacts.as_deref().unwrap_or_default(),
        decl.resource.as_deref(),
    ) else {
        let which = match decl.resource.as_deref() {
            Some(r) => format!(" for resource `{}`", r.trim()),
            None => String::new(),
        };
        return unread(
            entry.clone(),
            format!(
                "`{entry}` records no artifact{which}; `yidam catalog-fetch {entry}` obtains one"
            ),
        );
    };
    let digest = record.sha256.as_deref().unwrap_or_default().trim();
    let short = digest.get(..12).unwrap_or(digest);
    let Ok(hash) = ContentHash::parse(digest) else {
        return unread(
            entry.clone(),
            format!("`{entry}`'s newest artifact carries `{short}`, which is not a sha256"),
        );
    };
    let bytes = cache
        .map(|c| c.path_of(&hash))
        .and_then(|p| std::fs::read(p).ok());
    let Some(bytes) = bytes else {
        return unread(
            entry.clone(),
            format!(
                "the bytes of `{entry}` (sha256:{short}) are not in the local cache; \
                 `yidam vault pull` or `yidam catalog-fetch {entry}` fetches them"
            ),
        );
    };

    let listed = match members(&bytes, decl) {
        Ok(m) => m,
        Err(why) => return unread(entry.clone(), format!("`{entry}` (sha256:{short}) {why}")),
    };

    let covered: HashSet<String> = instances
        .iter()
        .filter(|n| crate::paths::class_of_path(&n.path) == class)
        .flat_map(|n| crate::cmd::query::exec::node_scalars(n, &property))
        .filter_map(|v| key(&v))
        .collect();

    let mut seen = HashSet::new();
    let mut distinct = 0usize;
    let mut missing = Vec::new();
    for raw in listed {
        let Some(k) = key(&raw) else { continue };
        if !seen.insert(k.clone()) {
            continue;
        }
        distinct += 1;
        if !covered.contains(&k) {
            missing.push(raw);
        }
    }

    let unfit = match (distinct, stale(&rel)) {
        (0, _) => Some(format!(
            "`{entry}` (sha256:{short}) lists no members — empty is not settled"
        )),
        (_, Some(why)) => Some(why),
        _ => None,
    };
    Universe {
        label,
        entry,
        class,
        property,
        reading: Reading::Measured {
            listed: distinct,
            missing,
            unfit,
        },
    }
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
    for u in universes {
        match &u.reading {
            Reading::Unread(why) => {
                unmeasured += 1;
                parts.push(format!("{}: {why}", u.label));
            }
            Reading::Measured {
                listed,
                missing,
                unfit,
            } => {
                let mut p = format!(
                    "{}: {} of {listed} covered, {} missing",
                    u.label,
                    listed - missing.len(),
                    missing.len()
                );
                if let Some(why) = unfit {
                    // Only a zero needs the list to be fit; see the module header.
                    if missing.is_empty() {
                        unmeasured += 1;
                    }
                    let _ = write!(p, " ({why})");
                }
                parts.push(p);
                subjects.extend(missing.iter().map(|m| {
                    format!(
                        "{m} — listed by {}, carried by no `{}` as `{}`",
                        u.entry, u.class, u.property
                    )
                }));
            }
        }
    }
    let detail = parts.join("; ");

    if !subjects.is_empty() {
        let mut clock = Clock::new(Clock::COVERAGE, Q, State::Due, detail)
            .at(interval)
            // Not `propose`. Whether a listed member earns a node — and what it says — is the
            // judgement; a drafted stub per member would discharge the clock with nothing
            // read. A corpus that means to cover less narrows the declaration instead.
            .owing(
                subjects.len(),
                "add an instance for each member below, or narrow the `[[due.coverage]]` that \
                 lists it — the gap is a worklist, not a defect",
                Actor::Person,
            );
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

    #[test]
    fn a_key_ignores_case_and_punctuation() {
        assert_eq!(key("S.B. 214"), key("sb214"));
        assert_eq!(key("SB-214"), key("sb 214"));
        assert_ne!(key("SB 214"), key("HB 214"));
        assert_eq!(key(" - "), None);
    }
}
