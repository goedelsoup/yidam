use yidam_domain_graph_metrics::{degree_centrality, density};

use yidam_domain_testkit::load_fixtures;

// ── density ───────────────────────────────────────────────────────────────────

#[test]
fn parity_density() {
    let fixtures = load_fixtures("graph_metrics.density");

    for fx in &fixtures {
        let inp = &fx["input"];
        let node_count = inp["node_count"].as_integer().unwrap() as u32;
        let edge_count = inp["edge_count"].as_integer().unwrap() as u32;
        let result = density(node_count, edge_count);
        let expected = fx["expected"]["density"].as_float().unwrap();
        assert_eq!(result, expected, "density({node_count}, {edge_count})");
    }
}

// ── degree_centrality ─────────────────────────────────────────────────────────

#[test]
fn parity_degree_centrality() {
    let fixtures = load_fixtures("graph_metrics.degree_centrality");

    for fx in &fixtures {
        let inp = &fx["input"];
        let degrees: Vec<u32> = inp["degrees"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_integer().unwrap() as u32)
            .collect();
        let node_count = inp["node_count"].as_integer().unwrap() as u32;
        let result = degree_centrality(&degrees, node_count);
        let expected: Vec<f64> = fx["expected"]["centrality"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        assert_eq!(result.len(), expected.len(), "centrality length mismatch");
        for (i, (r, e)) in result.iter().zip(expected.iter()).enumerate() {
            assert_eq!(r, e, "centrality[{i}]");
        }
    }
}
