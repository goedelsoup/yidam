# graph-metrics domain

This domain computes structural metrics over the corpus graph, treating corpus nodes as vertices and their cross-references as edges. It provides a lightweight, dependency-free way to characterize the topology of a corpus — how densely interconnected it is, and which nodes sit at the center of the network.

## Exposed functions

```rust
/// Undirected graph density: ratio of present edges to maximum possible edges.
/// Returns 0.0 if node_count < 2.
pub fn density(node_count: u32, edge_count: u32) -> f64

/// Degree centrality for each node: degree[i] / (node_count - 1).
/// Returns 0.0 for every node if node_count < 2.
pub fn degree_centrality(degrees: &[u32], node_count: u32) -> Vec<f64>
```

## When to use it

Use `density` to measure overall graph connectivity — a value near 0 indicates a sparse corpus where most nodes are isolated, while a value near 1 indicates a tightly cross-referenced knowledge graph. Use `degree_centrality` to identify the most central nodes in the corpus: high-centrality nodes are referenced by or link to many others and are good candidates for hub pages, concept glossaries, or navigation landmarks.
