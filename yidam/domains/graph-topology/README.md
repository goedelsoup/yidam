# graph-topology domain

This domain provides pure calculation functions for graph topology analysis. It covers two core structural metrics: computing the clustering coefficient of a node given its degree and triangle count, and counting connected components in an undirected graph.

## Exposed functions

```rust
/// Returns triangle_count / (degree*(degree-1)/2). Returns 0.0 if degree < 2.
pub fn clustering_coefficient(degree: u32, triangle_count: u32) -> f64;

/// Returns the number of connected components via union-find. Returns 0 if node_count == 0.
pub fn connected_components(node_count: u32, edges: &[[u32; 2]]) -> u32;
```

The `clustering_coefficient` function measures how close a node's neighbors are to forming a complete graph — it divides the actual number of triangles the node participates in by the maximum possible number. The `connected_components` function uses union-find with path halving to count the number of disjoint subgraphs in an undirected graph.

## When to use this domain

Use this domain in graph analysis repositories where you need lightweight, dependency-free structural metrics that can be called from any language. It is intentionally free of external graph library dependencies, making it suitable as a building block for higher-level pipelines or as a reference implementation for cross-language parity tests.
