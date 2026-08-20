//! `yidam graph` — the corpus as nodes, edges, and the classes that license them.
//!
//! Every other report answers a question about the corpus. This one hands over the corpus
//! itself, because the questions an *editor* asks are not a fixed set: where does this edge
//! go, what points back here, what may this class link to, what properties does it carry.
//!
//! # Why the CLI resolves the edges
//!
//! An edge is a filesystem-relative path inside YAML, resolved as `dir.join(target)` against
//! the instance's own directory and normalized so `../class/x.yml` and `class/x.yml` compare
//! equal. That rule is `dangling_edge`'s and `orphan_in`'s, and it is the whole of what makes
//! the graph a graph. A consumer resolving edges itself would be re-deriving it — and would
//! disagree with the gate about which edges are broken, silently, in the direction of
//! "looks fine here".
//!
//! So `resolved` and `exists` are computed here, by the same two lines the checks use.
//!
//! # And why the classes come along
//!
//! `.ont.yml` declares which relationships a class may enter into and which class each one
//! points at. Nothing exposed that outside the file, so an editor offering completion would
//! have had to parse the ontology — a second reader of a shape the prelude defines and
//! `yidam schema` already describes.

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_ont_files};

/// A class definition as the ontology writes it.
///
/// Local to this module rather than in `parse.rs`: `CorpusInstance` is read by six other
/// commands and this shape by none of them.
#[derive(Default, serde::Deserialize)]
struct OntologyFile {
    class: Option<String>,
    label: Option<String>,
    description: Option<String>,
    #[serde(default)]
    properties: Vec<OntProperty>,
    #[serde(default)]
    edges: Vec<OntEdge>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct OntProperty {
    pub name: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub description: String,
}

/// One relationship a class may enter into.
///
/// `target` is a **class name**, not a path — `holds` on `person` targets `tenure`, and the
/// instances it may point at are the ones in `tenure/`.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct OntEdge {
    pub relationship: String,
    #[serde(default)]
    pub target: String,
    /// `out` when instances of this class author the link, `in` when the other side does.
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub description: String,
}

#[derive(serde::Serialize)]
pub struct GraphClass {
    pub class: String,
    pub label: String,
    pub description: String,
    pub properties: Vec<OntProperty>,
    pub edges: Vec<OntEdge>,
}

#[derive(serde::Serialize)]
pub struct GraphLink {
    /// Exactly as written, so a consumer can find it in the file.
    pub target: String,
    pub relationship: String,
    /// Corpus-relative path of the resolved target, or empty when it lands outside the
    /// corpus. Resolution is `normalize(dir.join(target))` — the rule `dangling_edge` and
    /// `orphan_in` apply.
    pub resolved: String,
    /// Whether that path is a file. This is the gate's own test, and a consumer MUST NOT
    /// recompute it: an editor that decided for itself would disagree with `lint` about
    /// which edges are broken.
    pub exists: bool,
}

#[derive(serde::Serialize)]
pub struct GraphNode {
    /// Corpus-relative, as `corpus-index` reports it.
    pub node: String,
    pub class: String,
    pub label: String,
    pub description: String,
    pub links: Vec<GraphLink>,
}

#[derive(serde::Serialize)]
pub struct GraphReport {
    /// Repository-relative corpus root, so a consumer building a path does not have to
    /// know that it is `.yidam/corpus`.
    pub corpus_dir: String,
    pub nodes: Vec<GraphNode>,
    pub classes: Vec<GraphClass>,
}

/// One node reached by a traversal, described relative to where it was reached from.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Neighbor {
    pub id: String,
    pub relationship: String,
    /// `out` when the edge was authored on the node it was reached from, `in` otherwise.
    pub direction: &'static str,
    /// Hops from the start, 1-based.
    pub depth: usize,
}

/// Breadth-first over the **undirected** view of the graph.
///
/// Each reached node is reported once, at its shortest hop distance, carrying the direction
/// of the edge it was reached by. Undirected because a neighbourhood is a question about
/// what a node is connected to, and half the interesting connections are inbound —
/// `orphan-in` exists precisely because nodes are routinely pointed at rather than pointing.
///
/// **This is the traversal `serve --mcp`'s `neighbors` tool performs**, and it is here rather
/// than there so the two surfaces cannot come apart. It used to live in `serve/tools.rs`,
/// which is behind the heavy `index` feature — a light binary could not have answered the
/// same question at all, let alone answered it the same way.
///
/// `edges` is `(from, to, relationship)` in node order; that order is what decides the
/// output's, so a caller building it must walk its nodes in a stable one.
pub(crate) fn walk_neighbors(
    edges: &[(String, String, String)],
    start: &str,
    depth: usize,
) -> Vec<Neighbor> {
    use std::collections::{BTreeSet, VecDeque};

    let mut seen: BTreeSet<String> = BTreeSet::from([start.to_string()]);
    let mut queue: VecDeque<(String, usize)> = VecDeque::from([(start.to_string(), 0)]);
    let mut found = Vec::new();

    while let Some((current, hop)) = queue.pop_front() {
        if hop == depth {
            continue;
        }
        let outward = edges
            .iter()
            .filter(|(from, _, _)| *from == current)
            .map(|(_, to, rel)| (to.clone(), rel.clone(), "out"));
        let inward = edges
            .iter()
            .filter(|(_, to, _)| *to == current)
            .map(|(from, _, rel)| (from.clone(), rel.clone(), "in"));

        for (next, relationship, direction) in outward.chain(inward).collect::<Vec<_>>() {
            if !seen.insert(next.clone()) {
                continue;
            }
            found.push(Neighbor {
                id: next.clone(),
                relationship,
                direction,
                depth: hop + 1,
            });
            queue.push_back((next, hop + 1));
        }
    }
    found
}

/// Resolve `.` and `..` without touching the filesystem.
///
/// The same walk `lint::checks::normalize` performs. Duplicated rather than shared because
/// that one is private to a module whose surface is checks; if a third caller appears, move
/// it rather than copying it again.
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

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

pub(crate) fn graph_data(root: &Path, corpus: &Path) -> GraphReport {
    let nodes = walk_corpus_instances(corpus)
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
            let dir = path.parent().unwrap_or(path);
            let links = inst
                .links
                .unwrap_or_default()
                .into_iter()
                .map(|l| {
                    let target = l.target.unwrap_or_default();
                    let absolute = normalize(&dir.join(&target));
                    GraphLink {
                        exists: !target.is_empty() && dir.join(&target).is_file(),
                        resolved: absolute.strip_prefix(corpus).map(slash).unwrap_or_default(),
                        target,
                        relationship: l.relationship.unwrap_or_default(),
                    }
                })
                .collect();
            GraphNode {
                node: slash(path.strip_prefix(corpus).unwrap_or(path)),
                class: inst.class.unwrap_or_default(),
                label: inst.label.unwrap_or_default(),
                description: inst.description.unwrap_or_default(),
                links,
            }
        })
        .collect();

    let classes = walk_ont_files(corpus)
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let ont: OntologyFile = serde_yaml::from_str(&text).unwrap_or_default();
            // The filename is the class when the field is absent: `<class>.ont.yml` is what
            // `graph-check` matches an instance's directory against, so it is the name that
            // actually governs.
            let from_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .unwrap_or_default()
                .to_string();
            GraphClass {
                class: ont.class.unwrap_or(from_name),
                label: ont.label.unwrap_or_default(),
                description: ont.description.unwrap_or_default(),
                properties: ont.properties,
                edges: ont.edges,
            }
        })
        .collect();

    GraphReport {
        corpus_dir: slash(corpus.strip_prefix(root).unwrap_or(corpus)),
        nodes,
        classes,
    }
}

// ── neighbors ─────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct NeighborRow {
    /// Corpus-relative, as every other report identifies a node.
    pub node: String,
    pub class: String,
    pub label: String,
    pub description: String,
    pub relationship: String,
    pub direction: &'static str,
    /// Hops from the start.
    ///
    /// `hops`, not `depth`: `depth` on this report is the maximum that was asked for, and a
    /// field meaning two things in one document is how a consumer reads one as the other.
    /// (`serve --mcp` calls it `depth` in its own output; that contract is unchanged.)
    pub hops: usize,
    /// False for an `.ont.yml` target or a broken edge — reached, and not a corpus node.
    pub is_node: bool,
}

#[derive(serde::Serialize)]
pub struct NeighborsReport {
    pub corpus_dir: String,
    /// The node asked about, or empty when it was not found.
    pub node: String,
    pub class: String,
    pub label: String,
    pub description: String,
    /// The maximum hop count this traversal was allowed.
    pub depth: usize,
    pub neighbors: Vec<NeighborRow>,
}

/// Accept `concept/tailwater.yml`, `concept/tailwater`, or a repo-relative path.
///
/// The same tolerance `serve --mcp`'s `find_node` offers, for the same reason: an id is
/// written by hand at least as often as it is copied.
pub(crate) fn find_node<'a>(graph: &'a GraphReport, id: &str) -> Option<&'a GraphNode> {
    let want = id.trim().trim_start_matches('/');
    let bare = |s: &str| s.trim_end_matches(".yml").to_string();
    let target = bare(
        want.strip_prefix(&format!("{}/", graph.corpus_dir))
            .unwrap_or(want),
    );
    graph.nodes.iter().find(|n| bare(&n.node) == target)
}

pub(crate) fn neighbors_data(graph: &GraphReport, id: &str, depth: usize) -> NeighborsReport {
    let empty = NeighborsReport {
        corpus_dir: graph.corpus_dir.clone(),
        node: String::new(),
        class: String::new(),
        label: String::new(),
        description: String::new(),
        depth,
        neighbors: vec![],
    };
    let Some(start) = find_node(graph, id) else {
        return empty;
    };

    // Node order decides output order, and `walk_corpus_instances` sorts.
    let edges: Vec<(String, String, String)> = graph
        .nodes
        .iter()
        .flat_map(|n| {
            n.links
                .iter()
                .filter(|l| !l.resolved.is_empty())
                .map(move |l| (n.node.clone(), l.resolved.clone(), l.relationship.clone()))
        })
        .collect();

    let neighbors = walk_neighbors(&edges, &start.node, depth.max(1))
        .into_iter()
        .map(|hit| {
            let node = graph.nodes.iter().find(|n| n.node == hit.id);
            NeighborRow {
                class: node.map(|n| n.class.clone()).unwrap_or_default(),
                label: node.map(|n| n.label.clone()).unwrap_or_default(),
                description: node.map(|n| n.description.clone()).unwrap_or_default(),
                is_node: node.is_some(),
                node: hit.id,
                relationship: hit.relationship,
                direction: hit.direction,
                hops: hit.depth,
            }
        })
        .collect();

    NeighborsReport {
        node: start.node.clone(),
        class: start.class.clone(),
        label: start.label.clone(),
        description: start.description.clone(),
        neighbors,
        ..empty
    }
}

pub(crate) fn render_neighbors(r: &NeighborsReport) -> String {
    if r.node.is_empty() {
        return "Node not found.".to_string();
    }
    let mut out = format!(
        "{} — {}
{} neighbour(s) within {} hop(s)
",
        r.label,
        r.node,
        r.neighbors.len(),
        r.depth
    );
    for n in &r.neighbors {
        out.push_str(&format!(
            "  {}{} {} {}  ({})
",
            "  ".repeat(n.hops - 1),
            if n.direction == "out" { "-→" } else { "←-" },
            n.relationship,
            if n.label.is_empty() {
                &n.node
            } else {
                &n.label
            },
            n.node
        ));
    }
    out.trim_end().to_string()
}

/// Report the neighbourhood of one node.
pub fn neighbors(id: &str, depth: usize, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let graph = graph_data(&root, &yidam_corpus_dir(&root));
    let data = neighbors_data(&graph, id, depth);
    if format.is_json() {
        return crate::report::emit(&root, data);
    }
    println!("{}", render_neighbors(&data));
    Ok(())
}

pub(crate) fn render_graph(r: &GraphReport) -> String {
    if r.nodes.is_empty() && r.classes.is_empty() {
        return "Empty corpus — no instances and no class definitions.".to_string();
    }
    let edges: usize = r.nodes.iter().map(|n| n.links.len()).sum();
    let broken: usize = r
        .nodes
        .iter()
        .map(|n| n.links.iter().filter(|l| !l.exists).count())
        .sum();
    let mut out = format!(
        "{} node(s), {} class(es), {edges} edge(s){}\n",
        r.nodes.len(),
        r.classes.len(),
        if broken > 0 {
            format!(" — {broken} pointing at nothing")
        } else {
            String::new()
        }
    );
    for n in &r.nodes {
        for l in &n.links {
            out.push_str(&format!(
                "  {} -[{}]-> {}{}\n",
                n.node,
                l.relationship,
                if l.resolved.is_empty() {
                    &l.target
                } else {
                    &l.resolved
                },
                if l.exists { "" } else { "  (missing)" }
            ));
        }
    }
    out.trim_end().to_string()
}

/// Report the corpus graph: nodes, resolved edges, and the classes that license them.
pub fn graph(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let data = graph_data(&root, &yidam_corpus_dir(&root));
    if format.is_json() {
        return crate::report::emit(&root, data);
    }
    println!("{}", render_graph(&data));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn corpus() -> (TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let corpus = root.join(".yidam/corpus");
        std::fs::create_dir_all(corpus.join("concept")).unwrap();
        std::fs::create_dir_all(corpus.join("gauge")).unwrap();
        (tmp, root, corpus)
    }

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    /// `../class/x.yml` and `class/x.yml` resolve to the same node, which is the rule
    /// `orphan_in` normalizes for and the reason a consumer must not resolve edges itself.
    #[test]
    fn an_edge_resolves_the_way_the_gate_resolves_it() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\nlabel: A\nlinks:\n  - target: ../gauge/g.yml\n    relationship: reads\n",
        );
        write(&corpus.join("gauge/g.yml"), "class: gauge\nlabel: G\n");

        let r = graph_data(&root, &corpus);
        let a = r.nodes.iter().find(|n| n.node == "concept/a.yml").unwrap();
        assert_eq!(a.links[0].resolved, "gauge/g.yml");
        assert!(a.links[0].exists);
        // The raw text is kept, because that is what a consumer has to find in the file.
        assert_eq!(a.links[0].target, "../gauge/g.yml");
    }

    #[test]
    fn an_edge_pointing_at_nothing_is_reported_as_such_and_still_resolves() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\nlabel: A\nlinks:\n  - target: ../gauge/gone.yml\n    relationship: reads\n",
        );
        let r = graph_data(&root, &corpus);
        let link = &r.nodes[0].links[0];
        assert!(!link.exists);
        // Resolved anyway: a consumer offering "create this node" needs to know where it
        // would go, and a blank would make a broken edge indistinguishable from an
        // unparseable one.
        assert_eq!(link.resolved, "gauge/gone.yml");
    }

    /// A directory is not a node. `dangling_edge` tests `.exists()`; this tests `is_file()`,
    /// which is the same answer for every real target and the right one for `../gauge`.
    #[test]
    fn a_directory_is_not_a_target() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\nlabel: A\nlinks:\n  - target: ../gauge\n    relationship: reads\n",
        );
        assert!(!graph_data(&root, &corpus).nodes[0].links[0].exists);
    }

    #[test]
    fn a_class_carries_its_properties_and_edges() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept.ont.yml"),
            "class: concept\nlabel: Concept\ndescription: A unit.\n\
             properties:\n  - name: datum\n    type: string\n    description: Which datum.\n\
             edges:\n  - relationship: reads\n    target: gauge\n    direction: out\n    \
             description: The concept reads a gauge.\n",
        );
        let r = graph_data(&root, &corpus);
        let c = &r.classes[0];
        assert_eq!(c.class, "concept");
        assert_eq!(c.properties[0].name, "datum");
        assert_eq!(c.edges[0].relationship, "reads");
        assert_eq!(c.edges[0].target, "gauge");
        assert_eq!(c.edges[0].direction, "out");
    }

    /// The filename governs when the field is absent — `graph-check` matches an instance's
    /// directory against `<class>.ont.yml`, not against the `class:` field.
    #[test]
    fn a_class_without_a_class_field_is_named_by_its_file() {
        let (_t, root, corpus) = corpus();
        write(&corpus.join("gauge.ont.yml"), "label: Gauge\n");
        assert_eq!(graph_data(&root, &corpus).classes[0].class, "gauge");
    }

    #[test]
    fn the_corpus_root_is_reported_so_nobody_has_to_hardcode_it() {
        let (_t, root, corpus) = corpus();
        assert_eq!(graph_data(&root, &corpus).corpus_dir, ".yidam/corpus");
    }

    fn edge(from: &str, to: &str, rel: &str) -> (String, String, String) {
        (from.to_string(), to.to_string(), rel.to_string())
    }

    /// Undirected: half the interesting connections are inbound, which is why `orphan-in`
    /// exists at all.
    #[test]
    fn the_walk_goes_both_ways() {
        let edges = vec![edge("a", "b", "cites")];
        let from_b = walk_neighbors(&edges, "b", 1);
        assert_eq!(from_b.len(), 1);
        assert_eq!(from_b[0].id, "a");
        assert_eq!(from_b[0].direction, "in");
        assert_eq!(walk_neighbors(&edges, "a", 1)[0].direction, "out");
    }

    /// Each node once, at its *shortest* hop. A node reachable at 1 and 2 is a 1.
    #[test]
    fn a_node_is_reported_once_at_its_shortest_distance() {
        let edges = vec![
            edge("a", "b", "x"),
            edge("b", "c", "y"),
            edge("a", "c", "z"),
        ];
        let found = walk_neighbors(&edges, "a", 2);
        assert_eq!(found.len(), 2);
        assert_eq!(found.iter().find(|n| n.id == "c").unwrap().depth, 1);
    }

    #[test]
    fn a_cycle_terminates() {
        let edges = vec![edge("a", "b", "x"), edge("b", "a", "y")];
        assert_eq!(walk_neighbors(&edges, "a", 9).len(), 1);
    }

    #[test]
    fn depth_bounds_the_walk() {
        let edges = vec![edge("a", "b", "x"), edge("b", "c", "y")];
        assert_eq!(walk_neighbors(&edges, "a", 1).len(), 1);
        assert_eq!(walk_neighbors(&edges, "a", 2).len(), 2);
    }

    /// A target that is not a corpus node — an `.ont.yml`, or a broken edge — is still
    /// reached. Dropping it would make a neighbourhood disagree with the graph it came from.
    #[test]
    fn a_reached_non_node_is_reported_as_one() {
        let (_t, root, corpus) = corpus();
        write(
            &corpus.join("concept/a.yml"),
            "class: concept\nlabel: A\nlinks:\n  - target: ../concept.ont.yml\n    relationship: instance-of\n",
        );
        write(
            &corpus.join("concept.ont.yml"),
            "class: concept\nlabel: Concept\n",
        );
        let g = graph_data(&root, &corpus);
        let r = neighbors_data(&g, "concept/a", 1);
        assert_eq!(r.node, "concept/a.yml", "a bare id resolves");
        assert_eq!(r.neighbors.len(), 1);
        assert_eq!(r.neighbors[0].node, "concept.ont.yml");
        assert!(!r.neighbors[0].is_node);
    }

    #[test]
    fn an_unknown_node_is_an_empty_report_rather_than_an_error() {
        let (_t, root, corpus) = corpus();
        let g = graph_data(&root, &corpus);
        let r = neighbors_data(&g, "nowhere/at-all", 1);
        assert!(r.node.is_empty());
        assert!(render_neighbors(&r).contains("not found"));
    }

    #[test]
    fn an_empty_corpus_says_so() {
        let (_t, root, corpus) = corpus();
        assert!(render_graph(&graph_data(&root, &corpus)).contains("Empty corpus"));
    }
}
