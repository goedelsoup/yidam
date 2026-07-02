use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

/// All nodes reachable from `node_path` following directed edges (BFS).
/// The start node is not included. Result is sorted for determinism.
pub fn find_reachable(edges: &[GraphEdge], node_path: &str) -> Vec<String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    visited.insert(node_path.to_string());
    queue.push_back(node_path.to_string());
    let mut reachable: Vec<String> = Vec::new();
    while let Some(current) = queue.pop_front() {
        for edge in edges {
            if edge.from == current && !visited.contains(&edge.to) {
                visited.insert(edge.to.clone());
                reachable.push(edge.to.clone());
                queue.push_back(edge.to.clone());
            }
        }
    }
    reachable.sort();
    reachable
}

/// All nodes that have a directed edge pointing to `node_path`.
/// Result is sorted and deduplicated for determinism.
pub fn find_citations(edges: &[GraphEdge], node_path: &str) -> Vec<String> {
    let mut citations: Vec<String> = edges
        .iter()
        .filter(|e| e.to == node_path)
        .map(|e| e.from.clone())
        .collect();
    citations.sort();
    citations.dedup();
    citations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(from: &str, to: &str) -> GraphEdge {
        GraphEdge { from: from.into(), to: to.into() }
    }

    #[test]
    fn reachable_linear_chain() {
        let edges = vec![e("a.md", "b.md"), e("b.md", "c.md")];
        assert_eq!(find_reachable(&edges, "a.md"), vec!["b.md", "c.md"]);
    }

    #[test]
    fn reachable_cycle_terminates() {
        let edges = vec![e("a.md", "b.md"), e("b.md", "a.md")];
        assert_eq!(find_reachable(&edges, "a.md"), vec!["b.md"]);
    }

    #[test]
    fn reachable_isolated_node() {
        let edges = vec![e("a.md", "b.md")];
        assert_eq!(find_reachable(&edges, "b.md"), Vec::<String>::new());
    }

    #[test]
    fn citations_single() {
        let edges = vec![e("a.md", "b.md"), e("b.md", "c.md")];
        assert_eq!(find_citations(&edges, "b.md"), vec!["a.md"]);
    }

    #[test]
    fn citations_multiple() {
        let edges = vec![e("a.md", "c.md"), e("b.md", "c.md")];
        assert_eq!(find_citations(&edges, "c.md"), vec!["a.md", "b.md"]);
    }

    #[test]
    fn citations_none() {
        let edges = vec![e("a.md", "b.md")];
        assert_eq!(find_citations(&edges, "a.md"), Vec::<String>::new());
    }
}
