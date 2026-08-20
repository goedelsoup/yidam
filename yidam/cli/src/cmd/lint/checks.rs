//! The invariants `yidam lint` enforces.
//!
//! Each function returns a [`Check`] whether or not it found anything — a check that
//! reports nothing when it passes cannot be distinguished from a check that did not run,
//! and the difference matters when someone is deciding whether the gate covers a case.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::parse::{parse_frontmatter, CorpusInstance, CATALOG_LOCATION_KINDS};

use super::model::{Check, Severity, Violation};

/// A corpus instance parsed once, with the paths needed to talk about it.
pub struct Node {
    pub path: PathBuf,
    pub rel: String,
    pub inst: CorpusInstance,
}

/// A class definition parsed once — `<class>.ont.yml`.
pub struct Class {
    pub rel: String,
    /// The `description` field: the text that says what kind of thing an instance is.
    /// Deliberately the only field [`class_asserts_purpose`] reads; see there.
    pub description: String,
}

#[derive(Default, serde::Deserialize)]
struct ClassFields {
    #[serde(default)]
    description: Option<String>,
}

pub fn load_classes(root: &Path, paths: &[PathBuf]) -> Vec<Class> {
    paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let fields: ClassFields = serde_yaml::from_str(&text).unwrap_or_default();
            Class {
                rel: rel_of(root, p),
                description: fields.description.unwrap_or_default(),
            }
        })
        .collect()
}

/// A catalog entry parsed once.
pub struct Source {
    pub rel: String,
    pub slug: String,
    pub obtained: bool,
    pub used_by: Vec<String>,
    pub locations: Vec<crate::parse::CatalogLocation>,
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub fn load_nodes(root: &Path, paths: &[PathBuf]) -> Vec<Node> {
    paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            Node {
                path: p.clone(),
                rel: rel_of(root, p),
                inst: serde_yaml::from_str(&text).unwrap_or_default(),
            }
        })
        .collect()
}

pub fn load_sources(root: &Path, paths: &[PathBuf]) -> Vec<Source> {
    paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let fm = parse_frontmatter(&text);
            Source {
                rel: rel_of(root, p),
                slug: p
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                // Absent means obtained. Only an explicit `false` claims otherwise.
                obtained: fm.obtained.unwrap_or(true),
                used_by: fm.used_by.unwrap_or_default(),
                locations: fm.location.unwrap_or_default(),
            }
        })
        .collect()
}

// ── class definitions ─────────────────────────────────────────────────────────

/// Phrasings that assert a reason rather than describe a kind.
///
/// Each is a purpose construction: it says the thing was done *in order to* bring something
/// about, which is a claim about somebody's intent. Kept small on purpose — a longer list
/// buys recall at the cost of the check being switched off.
const PURPOSE_PHRASES: &[&str] = &[
    "designed to",
    "deployed to",
    "intended to",
    "calculated to",
    "engineered to",
    "contrived to",
    "orchestrated to",
    "in order to",
    "so as to",
    "for the purpose of",
    "with the aim of",
    "with the intent",
];

/// Class definitions whose `description` asserts a purpose.
///
/// **Why this check exists where no instance-level check could.** Every other rule in a
/// yidam corpus runs on claim tags, and a tag attaches to a claim somebody makes on a node.
/// A class definition is not that: it is the meaning every instance takes on by being filed
/// under the class — asserted identically, silently, untagged, and for each. So a class
/// defined as "a procedural mechanism *deployed to obtain* an outcome the ordinary path
/// would not yield" makes every one of its instances assert a purpose, and no amount of
/// tagging on the instances can reach it. That definition is real: it survived five
/// resolutions and three separate arguments about its instances in a derived repository,
/// because every safeguard that repository had was pointed at instances.
///
/// **Only `description` is read.** That field says what kind of thing an instance is, and
/// is what an instance silently asserts. A class may perfectly well *discuss* purpose in
/// `analytic_note` — the corrected version of the class above does exactly that, quoting
/// "designed to produce this outcome" in order to forbid it — and reading commentary would
/// flag the discussion of the rule as a violation of it.
///
/// **Warn, and a prompt rather than a proof.** This is a check over wording. It cannot see
/// a purpose asserted in words that are not on the list, and a description can name a
/// purpose someone else attributed without asserting it. Read the finding; do not clear it.
pub fn class_asserts_purpose(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .filter_map(|c| {
            let lowered = c.description.to_lowercase();
            let hit = PURPOSE_PHRASES.iter().find(|p| lowered.contains(**p))?;
            Some(Violation::new(
                &c.rel,
                format!("`description` says \"{hit}\" — a purpose, not a kind"),
            ))
        })
        .collect();
    Check::new(
        "class-asserts-purpose",
        "Class definition asserts a purpose rather than describing a kind",
        Severity::Warn,
        "A claim tag attaches to a claim somebody makes on a node. A class definition is \
         not that — it is the meaning every instance takes on by being filed under the \
         class, asserted identically and untagged for each. So a class whose description \
         states a reason ('deployed to obtain an outcome the ordinary path would not \
         yield') puts that assertion beyond the reach of every other check here, which all \
         run on tags. The case this was written from survived five resolutions and three \
         arguments about its instances. Rewrite the description to say what an instance IS \
         — the observable shape, the admission conditions — and move any characterization \
         of why to a field that attributes it, or to `analytic_note`, which this check does \
         not read.",
        violations,
    )
}

// ── corpus structure ──────────────────────────────────────────────────────────

pub fn missing_class(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.class.is_none())
        .map(|n| Violation::new(&n.rel, "no `class:` field"))
        .collect();
    Check::new(
        "missing-class",
        "Instance declaring no class",
        Severity::Error,
        "An instance's class is what binds it to a schema. Without one there is nothing to \
         validate the node against and nothing to render it as — it is a YAML file in a \
         corpus directory, not a node.",
        violations,
    )
}

pub fn unknown_class(nodes: &[Node], defined: &HashSet<String>) -> Check {
    // With no .ont.yml files at all, every class is "unknown" and the check would report
    // the whole corpus. That is a repository without a schema layer, which is a different
    // problem than a mistyped class name.
    let violations = if defined.is_empty() {
        Vec::new()
    } else {
        nodes
            .iter()
            .filter_map(|n| {
                let class = n.inst.class.as_ref()?;
                (!defined.contains(class)).then(|| {
                    Violation::new(&n.rel, format!("class `{class}` has no {class}.ont.yml"))
                })
            })
            .collect()
    };
    Check::new(
        "unknown-class",
        "Instance of a class that has no schema",
        Severity::Error,
        "Either the class file was never written or the instance misspells it. Both leave \
         the node unvalidatable, and a misspelling additionally splits one class into two \
         that no query will join.",
        violations,
    )
}

pub fn missing_label(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.label.is_none())
        .map(|n| Violation::new(&n.rel, "no `label:` field"))
        .collect();
    Check::new(
        "missing-label",
        "Instance with no human-readable label",
        Severity::Warn,
        "The label is what every index, export, and graph rendering shows. Without it a \
         reader gets a filename, which is an identifier chosen for stability rather than \
         for saying what the thing is.",
        violations,
    )
}

pub fn missing_description(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.description.is_none())
        .map(|n| Violation::new(&n.rel, "no `description:` field"))
        .collect();
    Check::new(
        "missing-description",
        "Instance with no description",
        Severity::Warn,
        "The description is the node's content. An instance with properties and links but \
         nothing said about it records that something exists without recording what is \
         known about it.",
        violations,
    )
}

pub fn orphan_out(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.links.as_deref().unwrap_or(&[]).is_empty())
        .map(|n| Violation::new(&n.rel, "no outgoing links"))
        .collect();
    Check::new(
        "orphan-out",
        "Node connected to nothing",
        Severity::Error,
        "Every node must carry at least one outgoing edge. A node with none is unreachable \
         by traversal from anywhere, which in a graph whose value is its connectivity means \
         it is not in the graph at all.",
        violations,
    )
}

pub fn dangling_edge(nodes: &[Node]) -> Check {
    let mut violations = Vec::new();
    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for link in n.inst.links.as_deref().unwrap_or(&[]) {
            match &link.target {
                None => violations.push(Violation::new(&n.rel, "link entry with no `target:`")),
                Some(target) => {
                    if !dir.join(target).exists() {
                        violations.push(Violation::new(
                            &n.rel,
                            format!("target does not exist: {target}"),
                        ));
                    }
                }
            }
        }
    }
    Check::new(
        "dangling-edge",
        "Edge pointing at nothing",
        Severity::Error,
        "A broken edge is worse than a missing one: it asserts a relationship to something \
         that is not there, so a reader following it learns nothing and a traversal counting \
         it overstates the graph's connectivity. Renaming a node severs every edge into it, \
         which is why node filenames are meant to be stable.",
        violations,
    )
}

pub fn orphan_in(nodes: &[Node]) -> Check {
    let mut targeted: HashSet<PathBuf> = HashSet::new();
    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for link in n.inst.links.as_deref().unwrap_or(&[]) {
            if let Some(t) = &link.target {
                // Normalize so `../class/x.yml` and `class/x.yml` compare equal.
                targeted.insert(normalize(&dir.join(t)));
            }
        }
    }
    let violations = nodes
        .iter()
        .filter(|n| !targeted.contains(&normalize(&n.path)))
        .map(|n| Violation::new(&n.rel, "nothing links to this node"))
        .collect();
    Check::new(
        "orphan-in",
        "Node nothing points to",
        Severity::Info,
        "Reported rather than gated: a node authored this morning legitimately has no \
         inbound edges yet, and the hub nodes of a young corpus often point outward at \
         everything while nothing points back. Worth seeing, not worth blocking on.",
        violations,
    )
}

/// Resolve `.` and `..` without touching the filesystem.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

// ── catalog ───────────────────────────────────────────────────────────────────

/// Corpus nodes citing each source slug, by repo-relative path.
pub fn citations(sources: &[Source], nodes: &[Node], texts: &[String]) -> Vec<Vec<String>> {
    sources
        .iter()
        .map(|s| {
            texts
                .iter()
                .zip(nodes)
                .filter(|(t, _)| t.contains(&s.slug))
                .map(|(_, n)| n.rel.clone())
                .collect()
        })
        .collect()
}

pub fn catalog_uncited(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let violations = sources
        .iter()
        .zip(cites)
        .filter(|(s, c)| s.obtained && c.is_empty())
        .map(|(s, _)| Violation::new(&s.rel, "no corpus node draws on this source"))
        .collect();
    Check::new(
        "catalog-uncited",
        "Registered source nothing draws from",
        Severity::Info,
        "A catalog entry describes a source, not knowledge, so an uncited one is not an \
         error. It is either a source registered ahead of the extraction that will use it, \
         or a claim resting on evidence that never became a node. The two look identical \
         from here and are worth telling apart — which is what `obtained: false` declares. \
         Entries that have said they are the former are not reported.",
        violations,
    )
}

pub fn catalog_unobtained_but_cited(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let violations = sources
        .iter()
        .zip(cites)
        .filter(|(s, c)| !s.obtained && !c.is_empty())
        .map(|(s, c)| {
            Violation::new(
                &s.rel,
                format!(
                    "`obtained: false` but {} node(s) cite it, e.g. {}",
                    c.len(),
                    c[0]
                ),
            )
        })
        .collect();
    Check::new(
        "catalog-unobtained-but-cited",
        "Source nobody has fetched, cited anyway",
        Severity::Error,
        "This is the price of the `obtained: false` exemption, which is otherwise the \
         cheapest way to silence `catalog-uncited`. A node drawing on a source the catalog \
         says was never retrieved is one of two things and both are defects: the flag is \
         stale and was not cleared when the source arrived, or the citation rests on \
         something nobody has read.",
        violations,
    )
}

pub fn catalog_used_by_drift(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let mut violations = Vec::new();
    for (s, actual) in sources.iter().zip(cites) {
        if s.used_by.is_empty() {
            continue; // the list is optional; absent is not drift
        }
        let claimed: HashSet<&str> = s.used_by.iter().map(|u| basename(u)).collect();
        let found: HashSet<&str> = actual.iter().map(|a| basename(a)).collect();
        let mut detail = Vec::new();
        let mut missing: Vec<&str> = claimed.difference(&found).copied().collect();
        let mut extra: Vec<&str> = found.difference(&claimed).copied().collect();
        missing.sort_unstable();
        extra.sort_unstable();
        if !missing.is_empty() {
            detail.push(format!("claims {} that do not cite it", missing.join(", ")));
        }
        if !extra.is_empty() {
            detail.push(format!("omits {} that do", extra.join(", ")));
        }
        if !detail.is_empty() {
            violations.push(Violation::new(&s.rel, detail.join("; ")));
        }
    }
    Check::new(
        "catalog-used-by-drift",
        "`used-by` list disagreeing with the citations",
        Severity::Warn,
        "The citations are authoritative — they cannot drift from the corpus, and a \
         hand-maintained list can. Both are kept so the disagreement is visible rather than \
         averaged away. The list is optional; an entry that declares one is asserting it is \
         current.",
        violations,
    )
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub fn catalog_location_malformed(sources: &[Source]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, loc) in s.locations.iter().enumerate() {
            let mut problems = Vec::new();
            match loc.kind.as_deref() {
                None => problems.push("no `kind`".to_string()),
                Some(k) if !CATALOG_LOCATION_KINDS.contains(&k) => {
                    problems.push(format!(
                        "kind `{k}` is not one of {}",
                        CATALOG_LOCATION_KINDS.join(", ")
                    ));
                }
                Some(k) => {
                    // The type decides whether a reader is offered a link, so a value
                    // whose shape contradicts its type renders wrongly.
                    let v = loc.value.as_deref().unwrap_or("");
                    if k == "url" && !v.starts_with("http://") && !v.starts_with("https://") {
                        problems.push(format!("kind `url` but value is not a URL: {v}"));
                    }
                    if k == "url_template" && !v.contains('{') {
                        problems
                            .push("kind `url_template` but value has no `{…}` placeholder".into());
                    }
                }
            }
            if loc.value.as_deref().unwrap_or("").trim().is_empty() {
                problems.push("no `value`".to_string());
            }
            if s.locations.len() > 1 && loc.description.is_none() {
                problems.push("several locations and no `description` to tell them apart".into());
            }
            if !problems.is_empty() {
                violations.push(Violation::new(
                    &s.rel,
                    format!("location[{i}]: {}", problems.join("; ")),
                ));
            }
        }
    }
    Check::new(
        "catalog-location-malformed",
        "Catalog location missing or mistyped",
        Severity::Warn,
        "A `location` is a list of typed places. The type decides whether a reader is \
         offered a link, so a value whose shape contradicts its type renders wrongly; and \
         where an entry has several locations, the description is the only thing \
         distinguishing them.",
        violations,
    )
}

// ── presentation ──────────────────────────────────────────────────────────────

/// Cells of a markdown table row, ignoring the leading and trailing pipes.
fn table_cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').collect()
}

fn is_delimiter_row(line: &str) -> bool {
    let cells = table_cells(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.trim().is_empty() && c.trim().chars().all(|ch| ch == '-' || ch == ':'))
}

/// Ragged markdown tables in any committed prose.
///
/// Applied to catalog entries and the corpus's own READMEs — anywhere a reader meets a
/// table. A table whose rows disagree on width does not render as a table; it renders as
/// a paragraph of pipes.
pub fn malformed_table(files: &[(String, String)]) -> Check {
    let mut violations = Vec::new();
    for (rel, text) in files {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            // A table is a header row followed by a delimiter row.
            if !lines[i].trim_start().starts_with('|') || i + 1 >= lines.len() {
                i += 1;
                continue;
            }
            if !is_delimiter_row(lines[i + 1]) {
                i += 1;
                continue;
            }
            let width = table_cells(lines[i]).len();
            if table_cells(lines[i + 1]).len() != width {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "line {}: header has {width} cells, delimiter has {} — renders as a \
                         paragraph of pipes",
                        i + 1,
                        table_cells(lines[i + 1]).len()
                    ),
                ));
            }
            let mut j = i + 2;
            let mut ragged = Vec::new();
            while j < lines.len() && lines[j].trim_start().starts_with('|') {
                if table_cells(lines[j]).len() != width {
                    ragged.push(j + 1);
                }
                j += 1;
            }
            if !ragged.is_empty() {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "row(s) at line {} have a different width than the {width}-cell header",
                        ragged
                            .iter()
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
            i = j;
        }
    }
    Check::new(
        "malformed-table",
        "Markdown table whose rows disagree on width",
        Severity::Warn,
        "A ragged table does not render as a table. Every reader of that file sees a \
         paragraph of pipes instead of the thing the author was trying to show, and the \
         author usually never looks at the rendered output again.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(rel: &str, yaml: &str) -> Node {
        Node {
            path: PathBuf::from(rel),
            rel: rel.to_string(),
            inst: serde_yaml::from_str(yaml).unwrap(),
        }
    }

    fn source(slug: &str, obtained: bool, used_by: &[&str]) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            slug: slug.to_string(),
            obtained,
            used_by: used_by.iter().map(|s| s.to_string()).collect(),
            locations: vec![],
        }
    }

    #[test]
    fn orphan_out_flags_a_node_with_no_links() {
        let nodes = vec![
            node("a.yml", "class: c\nlinks: []\n"),
            node("b.yml", "class: c\nlinks:\n  - target: a.yml\n"),
        ];
        let c = orphan_out(&nodes);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "a.yml");
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn unknown_class_is_silent_when_no_schema_layer_exists() {
        let nodes = vec![node("a.yml", "class: whatever\n")];
        assert!(unknown_class(&nodes, &HashSet::new()).passed());
    }

    #[test]
    fn unknown_class_fires_once_a_schema_layer_exists() {
        let nodes = vec![node("a.yml", "class: typo\n")];
        let defined: HashSet<String> = ["real".to_string()].into_iter().collect();
        assert_eq!(unknown_class(&nodes, &defined).violations.len(), 1);
    }

    #[test]
    fn orphan_in_sees_through_relative_traversal() {
        // `../reach/a.yml` from reach/ must match `reach/a.yml`, or every cross-class
        // edge would look like it points somewhere else.
        let a = Node {
            path: PathBuf::from("corpus/reach/a.yml"),
            rel: "corpus/reach/a.yml".into(),
            inst: serde_yaml::from_str("class: c\nlinks: []\n").unwrap(),
        };
        let b = Node {
            path: PathBuf::from("corpus/other/b.yml"),
            rel: "corpus/other/b.yml".into(),
            inst: serde_yaml::from_str("class: c\nlinks:\n  - target: ../reach/a.yml\n").unwrap(),
        };
        let c = orphan_in(&[a, b]);
        let flagged: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            flagged,
            vec!["corpus/other/b.yml"],
            "only b is unpointed-at"
        );
    }

    #[test]
    fn an_unobtained_source_is_not_reported_as_uncited() {
        let s = vec![source("not-yet-fetched", false, &[])];
        assert!(catalog_uncited(&s, &[vec![]]).passed());
    }

    #[test]
    fn an_obtained_source_nothing_cites_is_reported() {
        let s = vec![source("fetched", true, &[])];
        assert_eq!(catalog_uncited(&s, &[vec![]]).violations.len(), 1);
    }

    #[test]
    fn citing_an_unfetched_source_is_an_error() {
        let s = vec![source("not-yet-fetched", false, &[])];
        let c = catalog_unobtained_but_cited(&s, &[vec!["corpus/x/y.yml".to_string()]]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn used_by_drift_reports_both_directions() {
        let s = vec![source("src", true, &["../corpus/a/one.yml"])];
        let c = catalog_used_by_drift(&s, &[vec!["corpus/a/two.yml".to_string()]]);
        assert_eq!(c.violations.len(), 1);
        let d = &c.violations[0].detail;
        assert!(d.contains("one.yml"), "should name the stale claim: {d}");
        assert!(d.contains("two.yml"), "should name the omitted citer: {d}");
    }

    #[test]
    fn an_absent_used_by_list_is_not_drift() {
        let s = vec![source("src", true, &[])];
        assert!(catalog_used_by_drift(&s, &[vec!["corpus/a/x.yml".to_string()]]).passed());
    }

    #[test]
    fn a_url_location_must_be_a_url() {
        let mut s = source("src", true, &[]);
        s.locations = vec![crate::parse::CatalogLocation {
            kind: Some("url".into()),
            value: Some("see the reading room".into()),
            description: None,
        }];
        assert_eq!(catalog_location_malformed(&[s]).violations.len(), 1);
    }

    #[test]
    fn a_well_formed_location_passes() {
        let mut s = source("src", true, &[]);
        s.locations = vec![crate::parse::CatalogLocation {
            kind: Some("url".into()),
            value: Some("https://example.org/x".into()),
            description: None,
        }];
        assert!(catalog_location_malformed(&[s]).passed());
    }

    #[test]
    fn a_well_formed_table_passes() {
        let text = "intro\n\n| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\nafter\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }

    #[test]
    fn a_ragged_body_row_is_flagged_with_its_line_number() {
        let text = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 |\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains('4'),
            "should name line 4: {}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_header_and_delimiter_of_different_widths_are_flagged() {
        let text = "| A | B | C |\n|---|---|\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert!(c.violations[0].detail.contains("paragraph of pipes"));
    }

    #[test]
    fn a_line_of_pipes_that_is_not_a_table_is_ignored() {
        // No delimiter row, so this is prose that happens to contain pipes.
        let text = "the grammar is `a | b | c` in this notation\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }

    // ── class-asserts-purpose ─────────────────────────────────────────────────

    fn class(rel: &str, description: &str) -> Class {
        Class {
            rel: rel.into(),
            description: description.into(),
        }
    }

    #[test]
    fn the_definition_that_motivated_the_check_is_caught() {
        // Verbatim from a derived repository's genesis. Every instance of this class
        // asserted a purpose by existing, and no instance-level tag could reach it.
        let c = class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A procedural mechanism deployed to obtain an outcome the ordinary path \
             would not yield.",
        )]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("deployed to"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn the_documentary_rewrite_of_that_definition_passes() {
        // The same class after it was redefined: an observable shape and its admission
        // conditions, with no claim about anyone's reason.
        assert!(class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A dated procedural sequence that departed from a baseline the record fixed \
             before the sequence and independently of it. The departure is the whole of \
             the class's content, and it is a comparison rather than a motive.",
        )])
        .passed());
    }

    #[test]
    fn commentary_outside_description_is_not_read() {
        // `analytic_note` is where a class discusses the purpose rule — often by quoting
        // the forbidden phrasing in order to forbid it. Reading it would flag the
        // discussion of the rule as a violation of the rule.
        let text = "class: maneuver\ndescription: A dated procedural sequence.\n\
                    analytic_note: |\n  \"designed to produce this outcome\" is an \
                    allegation, not a record.\n";
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("maneuver.ont.yml");
        std::fs::write(&p, text).unwrap();
        let loaded = load_classes(dir.path(), &[p]);
        assert_eq!(loaded[0].description.trim(), "A dated procedural sequence.");
        assert!(class_asserts_purpose(&loaded).passed());
    }

    #[test]
    fn the_match_is_case_insensitive() {
        let c = class_asserts_purpose(&[class("x.ont.yml", "An instrument Designed To pass.")]);
        assert_eq!(c.violations.len(), 1);
    }

    #[test]
    fn a_class_with_no_description_passes() {
        assert!(class_asserts_purpose(&[class("x.ont.yml", "")]).passed());
    }

    #[test]
    fn the_check_warns_rather_than_gates() {
        // A heuristic over wording. Gating on it would make every false positive a
        // blocked commit, and the check would be switched off within a week.
        assert_eq!(class_asserts_purpose(&[]).severity, Severity::Warn);
    }
}

// ── Resolution annotations ───────────────────────────────────────────────────
//
// A resolution record is dated, and its `What remains open` describes the world on that
// date. The world moves, so PROTOCOL allows a dated annotation to be appended beneath an
// item that has moved — never an edit to the original.
//
// The risk that convention carries is not that an annotation is wrong. It is what an
// annotation structurally *is*: a place where one elector, in one commit, having read no
// `ma/*` tip and transported nothing, could perform a resolution in a file the protocol
// never routes through one. Article V puts synthesis in resolution events, so an
// annotation may record movement and never outcome.

/// One `> **Moved …**` annotation found in a resolution record.
pub struct Annotation {
    pub file: String,
    pub line: usize,
    /// The `## ` heading it sits under, or empty if it precedes the first one.
    pub section: String,
    pub text: String,
}

/// Find every annotation in a resolution record, with the section it sits under.
pub fn annotations_in(file: &str, text: &str) -> Vec<Annotation> {
    let mut section = String::new();
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("## ") {
            section = rest.trim().to_string();
            continue;
        }
        // The annotation form PROTOCOL prescribes. Matched on the block-quote marker plus
        // the bold `Moved` opener, so ordinary prose mentioning the word is not a finding.
        if line.starts_with("> **Moved") {
            out.push(Annotation {
                file: file.to_string(),
                line: i + 1,
                section: section.clone(),
                text: line.to_string(),
            });
        }
    }
    out
}

/// `> **Moved YYYY-MM-DD.**` — an ISO date immediately after the word.
fn carries_iso_date(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("> **Moved") else {
        return false;
    };
    let rest = rest.trim_start();
    let date: String = rest.chars().take(10).collect();
    if date.len() != 10 {
        return false;
    }
    let b = date.as_bytes();
    b[4] == b'-'
        && b[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|&i| b[i].is_ascii_digit())
}

pub fn resolution_annotation_malformed(annotations: &[Annotation]) -> Check {
    let violations = annotations
        .iter()
        .filter_map(|a| {
            let node = format!("{}:{}", a.file, a.line);
            if !carries_iso_date(&a.text) {
                return Some(Violation::new(
                    node,
                    "no ISO date after `Moved` — an annotation carries the date of the \
                     movement, not of the resolution",
                ));
            }
            // Annotating anything but the open items edits the record rather than
            // extending it.
            if a.section != "What remains open" {
                return Some(Violation::new(
                    node,
                    format!(
                        "sits under `{}` — annotate `What remains open` and nowhere else",
                        if a.section.is_empty() {
                            "(no section)"
                        } else {
                            &a.section
                        }
                    ),
                ));
            }
            None
        })
        .collect();
    Check::new(
        "resolution-annotation-malformed",
        "Resolution annotation is malformed or in the wrong section",
        Severity::Error,
        "Both halves are structural rather than a judgement about wording, which is why \
         this gates where its sibling does not. An annotation with no date cannot be read \
         against the record it extends — the whole point is that the record speaks for its \
         own date and the annotation for a later one. An annotation under `What was \
         resolved` or `What changed` edits history instead of extending it, and those \
         sections are the ones a reader trusts to describe a settled past.",
        violations,
    )
}

/// Words that announce a decision rather than report a movement.
const DECIDING_WORDS: &[&str] = &[
    "resolved",
    "settled",
    "decided",
    "concluded",
    "agreed",
    "closes",
    "closed",
    "adopted",
    "rejected",
    "supersedes",
    "superseded",
];

pub fn resolution_annotation_decides(annotations: &[Annotation]) -> Check {
    let violations = annotations
        .iter()
        .filter_map(|a| {
            let lower = a.text.to_lowercase();
            let hit = DECIDING_WORDS.iter().find(|w| {
                // Word-boundary-ish: avoid matching inside a longer word.
                lower
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .any(|t| t == **w)
            })?;
            Some(Violation::new(
                format!("{}:{}", a.file, a.line),
                format!(
                    "`{hit}` announces an outcome — an annotation records movement only; \
                     an open item is closed by a later resolution"
                ),
            ))
        })
        .collect();
    Check::new(
        "resolution-annotation-decides",
        "Resolution annotation announces an outcome",
        Severity::Warn,
        "An annotation may say a question was proposed, retrieved, measured or superseded \
         in fact; it may not say it was settled. Closing an open item is synthesis, and \
         Article V puts synthesis in resolution events — so an annotation that decides \
         something is a resolution performed by one elector, in one commit, in a file the \
         protocol never routes through a resolution. Warn rather than Error because this \
         is a heuristic over wording: it cannot see an outcome announced in words that are \
         not on the list, and gating on it would make every false positive a blocked \
         commit and the check would be switched off within a week.",
        violations,
    )
}

#[cfg(test)]
mod annotation_tests {
    use super::*;

    const WELL_FORMED: &str = "\
# local-infrastructure

## What was resolved

The project and site classes are adopted.

## What remains open

Whether `body` splits into fields.

> **Moved 2026-08-18.** A budget proposal now exists on `ma/goedelsoup`, measured over
> six commits. One branch holds it, so it is a proposal rather than a tension.

Whether block-quoted host error text should be indexed.
";

    fn annos(text: &str) -> Vec<Annotation> {
        annotations_in("resolutions/x.md", text)
    }

    #[test]
    fn a_well_formed_annotation_passes_both_checks() {
        let a = annos(WELL_FORMED);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].section, "What remains open");
        assert!(resolution_annotation_malformed(&a).passed());
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// The date is what lets a reader tell the record's own date from the movement's.
    #[test]
    fn an_annotation_without_a_date_is_an_error() {
        let a = annos("## What remains open\n\n> **Moved.** Somebody proposed it.\n");
        let c = resolution_annotation_malformed(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("ISO date"),
            "{:?}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_non_iso_date_is_an_error() {
        let a = annos("## What remains open\n\n> **Moved 18/08/2026.** Proposed.\n");
        assert_eq!(resolution_annotation_malformed(&a).violations.len(), 1);
    }

    /// Annotating a settled section edits history rather than extending it.
    #[test]
    fn an_annotation_under_what_was_resolved_is_an_error() {
        let a = annos("## What was resolved\n\n> **Moved 2026-08-18.** It was reopened.\n");
        let c = resolution_annotation_malformed(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("What was resolved"),
            "{:?}",
            c.violations[0].detail
        );
    }

    /// The one that matters: an annotation performing a resolution.
    #[test]
    fn an_annotation_announcing_an_outcome_is_warned() {
        let a = annos(
            "## What remains open\n\n> **Moved 2026-08-18.** This is now settled: the \
             bodies stay indexed.\n",
        );
        let c = resolution_annotation_decides(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("settled"),
            "{:?}",
            c.violations[0].detail
        );
        // Structurally fine — it is the wording that offends.
        assert!(resolution_annotation_malformed(&a).passed());
    }

    /// Reporting a movement in fact is exactly what the convention is for, even when the
    /// movement happens to be a supersession that occurred elsewhere.
    #[test]
    fn reporting_a_movement_is_not_announcing_an_outcome() {
        let a = annos(
            "## What remains open\n\n> **Moved 2026-08-19.** Measured at 714 bytes per \
             document and recorded on `ma/auditor`.\n",
        );
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// A deciding word inside a longer word is not a hit.
    #[test]
    fn the_word_list_respects_boundaries() {
        let a = annos("## What remains open\n\n> **Moved 2026-08-19.** Unresolvedness aside.\n");
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// Prose that merely mentions the word is not an annotation.
    #[test]
    fn only_the_prescribed_form_is_read_as_an_annotation() {
        let a = annos("## What remains open\n\nNothing has moved since this was written.\n");
        assert!(a.is_empty());
    }

    /// A repository with no sangha has no annotations and no findings.
    #[test]
    fn no_annotations_is_clean() {
        assert!(resolution_annotation_malformed(&[]).passed());
        assert!(resolution_annotation_decides(&[]).passed());
    }
}
