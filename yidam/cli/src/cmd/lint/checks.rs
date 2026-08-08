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
}
