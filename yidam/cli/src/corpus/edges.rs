//! Which node points at which — the corpus as a graph, resolved once.
//!
//! A `links:` entry is a path written relative to the file it appears in, so turning one
//! into *the node it means* is two steps: join it onto the authoring node's directory, and
//! resolve the `..` segments without asking the filesystem. Both steps were written out at
//! eight call sites before #925 — [`crate::cmd::lint::checks`] three times,
//! `graph`, `history`, `scope`, `lsp`, `migrate` and `rename` once each — and `normalize`
//! itself existed three times, in `checks.rs`, `graph.rs` and `rename.rs`. The copy in
//! `graph.rs` carried the condition for ending this in its own doc comment: *"Duplicated
//! rather than shared because that one is private to a module whose surface is checks; if a
//! third caller appears, move it rather than copying it again."* `rename.rs` was the third
//! caller.
//!
//! **Resolution is not membership.** A link resolves to a *path*; whether that path is a
//! node is a separate question, and the two have been answered differently by different
//! commands. `graph` asks the filesystem (`is_file`), so a link into the catalog or at a
//! `.ont.yml` counts as resolved there. The licensing checks ask the corpus
//! ([`Edges::to`] is `None` unless the path is an instance the walk found), because an
//! edge to a class file is an `instance-of` citation and not a relationship. [`Edge`] keeps
//! both: [`Edge::resolved`] is the path, [`Edge::to`] is the node, and a caller says which
//! one it means.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::parse::CorpusLink;

use super::Node;

/// Resolve `.` and `..` without touching the filesystem.
///
/// Purely lexical, which is the point: the checks that consume it compare a link's target
/// against the set of files the walk found, and a `canonicalize` would resolve symlinks and
/// fail outright on a path that is not there — the case `dangling-edge` exists to report.
pub fn normalize(p: &Path) -> PathBuf {
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

/// What a link written in `from` as `target` points at.
///
/// `from` is the *node's* path, not its directory: every caller had the node in hand and
/// each one re-derived the parent, one of them (`checks::dangling_edge`) falling back to the
/// node's own path when there was none — which is what a `.yml` at the filesystem root would
/// do, and is preserved here rather than tidied, because a corpus directory is always at
/// least two deep and no real call can reach it.
pub fn resolve_target(from: &Path, target: &str) -> PathBuf {
    let dir = from.parent().unwrap_or(from);
    normalize(&dir.join(target))
}

/// One `links:` entry, resolved.
#[derive(Debug, Clone)]
pub struct Edge {
    /// Position in the authoring node's `inst.links`. Read through [`Self::written_as`],
    /// which is how a caller recovers the entry as written — its `relationship`, its
    /// `claim_tag`, its `source` — without this type growing a copy of
    /// [`crate::parse::CorpusLink`] that could disagree with it.
    pub index: usize,
    /// Where the target lands, as an absolute path. Empty `target:` resolves to the
    /// authoring node's own directory, which is not an instance and so never a [`Self::to`].
    pub resolved: PathBuf,
    /// Index into the corpus's nodes, when [`Self::resolved`] is one of them.
    pub to: Option<usize>,
}

impl Edge {
    /// The `links:` entry this edge was resolved from.
    ///
    /// `from` must be the node that authored it — the node at the index this edge came back
    /// under. Passing another node's is a caller error that cannot be checked here, which is
    /// why nothing but [`Edges::out`] and [`Edges::instance_links`] hands one out.
    pub fn written_as<'a>(&self, from: &'a Node) -> &'a CorpusLink {
        &from.inst.links.as_deref().unwrap_or_default()[self.index]
    }
}

/// Every node's outgoing links, resolved, plus the reverse direction.
///
/// Built once from a `&[Node]` and indexed by position in that slice, so it holds no
/// borrow of the nodes and can live beside them in a [`super::Corpus`]. The alternative —
/// `HashMap<PathBuf, &Node>` — is what four call sites built independently, and a map of
/// references cannot be stored next to what it refers to.
#[derive(Debug, Default)]
pub struct Edges {
    by_path: HashMap<PathBuf, usize>,
    out: Vec<Vec<Edge>>,
    incoming: Vec<Vec<usize>>,
}

impl Edges {
    /// Resolve every link in `nodes` against `nodes`.
    pub fn build(nodes: &[Node]) -> Self {
        let by_path: HashMap<PathBuf, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (normalize(&n.path), i))
            .collect();

        let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        let mut out: Vec<Vec<Edge>> = Vec::with_capacity(nodes.len());
        for (i, n) in nodes.iter().enumerate() {
            let mut edges = Vec::new();
            for (index, l) in n.inst.links.iter().flatten().enumerate() {
                let Some(target) = l.target.as_deref() else {
                    continue;
                };
                let resolved = resolve_target(&n.path, target);
                let to = by_path.get(&resolved).copied();
                if let Some(t) = to {
                    incoming[t].push(i);
                }
                edges.push(Edge {
                    index,
                    resolved,
                    to,
                });
            }
            out.push(edges);
        }

        Self {
            by_path,
            out,
            incoming,
        }
    }

    /// Whether `path` is a node of this corpus — membership without reading the filesystem.
    pub fn holds(&self, path: &Path) -> bool {
        self.by_path.contains_key(&normalize(path))
    }

    /// Every link node `i` authored, in the order the file wrote them.
    pub fn out(&self, i: usize) -> &[Edge] {
        self.out.get(i).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The nodes that point at node `i`, in corpus order.
    ///
    /// A node appears once per edge it authored, not once per node: two links from the same
    /// file to the same target are two citations of it, and `orphan_in` counts subjects
    /// while a future inbound-degree report would count edges. Neither is served by a
    /// silently deduplicated list.
    pub fn incoming(&self, i: usize) -> &[usize] {
        self.incoming.get(i).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Node `i`'s links that land on another instance, as `(edge, target index)`.
    ///
    /// Everything else — the `instance-of` link to the class file, a citation into the
    /// catalog, an edge to a file that is not there — is not an ontology edge. This is the
    /// predicate `checks::instance_links` applied, kept word for word: a broken edge is
    /// `dangling-edge`'s finding and is not reported a second time by a licensing check.
    pub fn instance_links(&self, i: usize) -> impl Iterator<Item = (&Edge, usize)> {
        self.out(i).iter().filter_map(|e| e.to.map(|t| (e, t)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, links: &[&str]) -> Node {
        let body = links
            .iter()
            .map(|t| format!("  - target: {t}\n    relationship: cites\n"))
            .collect::<String>();
        let text = if links.is_empty() {
            "class: concept\n".to_string()
        } else {
            format!("class: concept\nlinks:\n{body}")
        };
        Node::parse(PathBuf::from(path), path, text)
    }

    #[test]
    fn a_relative_target_resolves_against_the_authoring_node() {
        assert_eq!(
            resolve_target(Path::new("/c/concept/a.yml"), "../gauge/g.yml"),
            PathBuf::from("/c/gauge/g.yml")
        );
    }

    #[test]
    fn an_edge_to_a_node_carries_its_index_and_an_edge_out_of_the_corpus_does_not() {
        let nodes = vec![
            node(
                "/c/concept/a.yml",
                &["../gauge/g.yml", "../../catalog/s.md"],
            ),
            node("/c/gauge/g.yml", &[]),
        ];
        let e = Edges::build(&nodes);
        let out = e.out(0);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].to, Some(1));
        assert_eq!(
            out[0].written_as(&nodes[0]).relationship.as_deref(),
            Some("cites")
        );
        assert_eq!(out[0].resolved, PathBuf::from("/c/gauge/g.yml"));
        // Resolves to a path, is not a node: the distinction the module note draws.
        assert_eq!(out[1].to, None);
        assert_eq!(out[1].resolved, PathBuf::from("/catalog/s.md"));
        assert_eq!(e.instance_links(0).count(), 1);
    }

    #[test]
    fn incoming_is_the_reverse_of_out() {
        let nodes = vec![
            node("/c/concept/a.yml", &["../gauge/g.yml"]),
            node("/c/concept/b.yml", &["../gauge/g.yml"]),
            node("/c/gauge/g.yml", &[]),
        ];
        let e = Edges::build(&nodes);
        assert_eq!(e.incoming(2), &[0, 1]);
        assert!(e.incoming(0).is_empty());
    }

    /// Two links from one file to one target are two edges, not one — see [`Edges::incoming`].
    #[test]
    fn a_node_cited_twice_by_one_file_appears_twice() {
        let nodes = vec![
            node("/c/concept/a.yml", &["../gauge/g.yml", "../gauge/g.yml"]),
            node("/c/gauge/g.yml", &[]),
        ];
        let e = Edges::build(&nodes);
        assert_eq!(e.incoming(1), &[0, 0]);
    }

    #[test]
    fn a_link_with_no_target_is_not_an_edge() {
        let n = Node::parse(
            PathBuf::from("/c/concept/a.yml"),
            "concept/a.yml",
            "class: concept\nlinks:\n  - relationship: cites\n",
        );
        let e = Edges::build(&[n]);
        assert!(e.out(0).is_empty());
    }

    #[test]
    fn membership_answers_without_the_filesystem() {
        let nodes = vec![node("/c/concept/a.yml", &[])];
        let e = Edges::build(&nodes);
        assert!(e.holds(Path::new("/c/concept/./a.yml")));
        assert!(!e.holds(Path::new("/c/concept/b.yml")));
        // The question is asked with whatever path a link resolved to, `..` and all.
        assert!(e.holds(Path::new("/c/gauge/../concept/a.yml")));
    }

    /// The index is by position, and a query about a node that is not in it answers rather
    /// than panicking — `out` and `incoming` are read from loops the caller writes.
    #[test]
    fn an_index_past_the_corpus_is_empty_rather_than_a_panic() {
        let e = Edges::build(&[]);
        assert!(e.out(7).is_empty());
        assert!(e.incoming(7).is_empty());
    }
}
