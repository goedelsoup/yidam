use yidam_domain_graph_topology::{clustering_coefficient, connected_components};

use yidam_domain_testkit::load_fixtures;

// ── graph_topology.clustering_coefficient ─────────────────────────────────────

#[test]
fn parity_clustering_coefficient() {
    let fixtures = load_fixtures("graph_topology.clustering_coefficient");

    for fx in &fixtures {
        let input = &fx["input"];
        let degree = input["degree"].as_integer().unwrap() as u32;
        let triangle_count = input["triangle_count"].as_integer().unwrap() as u32;
        let result = clustering_coefficient(degree, triangle_count);
        let expected = fx["expected"]["coefficient"].as_float().unwrap();
        assert_eq!(
            result, expected,
            "clustering_coefficient mismatch for fixture"
        );
    }
}

// ── graph_topology.connected_components ──────────────────────────────────────

#[test]
fn parity_connected_components() {
    let fixtures = load_fixtures("graph_topology.connected_components");

    for fx in &fixtures {
        let input = &fx["input"];
        let node_count = input["node_count"].as_integer().unwrap() as u32;
        let edges: Vec<[u32; 2]> = input["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let arr = e.as_array().unwrap();
                [
                    arr[0].as_integer().unwrap() as u32,
                    arr[1].as_integer().unwrap() as u32,
                ]
            })
            .collect();
        let result = connected_components(node_count, &edges);
        let expected = fx["expected"]["components"].as_integer().unwrap() as u32;
        assert_eq!(
            result, expected,
            "connected_components mismatch for fixture"
        );
    }
}
