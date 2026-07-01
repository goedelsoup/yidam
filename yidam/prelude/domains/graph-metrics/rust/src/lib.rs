pub fn density(node_count: u32, edge_count: u32) -> f64 {
    if node_count < 2 {
        return 0.0;
    }
    let max_edges = (node_count as f64) * (node_count as f64 - 1.0) / 2.0;
    edge_count as f64 / max_edges
}

pub fn degree_centrality(degrees: &[u32], node_count: u32) -> Vec<f64> {
    if node_count < 2 {
        return degrees.iter().map(|_| 0.0).collect();
    }
    let n = (node_count - 1) as f64;
    degrees.iter().map(|&d| d as f64 / n).collect()
}
