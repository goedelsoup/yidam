use yidam_domain_graph_topology::{clustering_coefficient, connected_components};

fn fixture_dir(function: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = prelude/domains/graph-topology/rust/
    // ../../parity/fixtures/<function>  →  prelude/domains/parity/fixtures/<function>
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../parity/fixtures")
        .join(function)
}

fn load_fixtures(function: &str) -> Vec<toml::Value> {
    let dir = fixture_dir(function);
    if !dir.exists() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
        .collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let raw = std::fs::read_to_string(entry.path()).unwrap();
        out.push(toml::from_str::<toml::Value>(&raw).unwrap());
    }
    out
}

// ── graph_topology.clustering_coefficient ─────────────────────────────────────

#[test]
fn parity_clustering_coefficient() {
    let fixtures = load_fixtures("graph_topology.clustering_coefficient");
    assert!(!fixtures.is_empty(), "no graph_topology.clustering_coefficient fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let degree = input["degree"].as_integer().unwrap() as u32;
        let triangle_count = input["triangle_count"].as_integer().unwrap() as u32;
        let result = clustering_coefficient(degree, triangle_count);
        let expected = fx["expected"]["coefficient"].as_float().unwrap();
        assert_eq!(result, expected, "clustering_coefficient mismatch for fixture");
    }
}

// ── graph_topology.connected_components ──────────────────────────────────────

#[test]
fn parity_connected_components() {
    let fixtures = load_fixtures("graph_topology.connected_components");
    assert!(!fixtures.is_empty(), "no graph_topology.connected_components fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let node_count = input["node_count"].as_integer().unwrap() as u32;
        let edges: Vec<[u32; 2]> = input["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let arr = e.as_array().unwrap();
                [arr[0].as_integer().unwrap() as u32, arr[1].as_integer().unwrap() as u32]
            })
            .collect();
        let result = connected_components(node_count, &edges);
        let expected = fx["expected"]["components"].as_integer().unwrap() as u32;
        assert_eq!(result, expected, "connected_components mismatch for fixture");
    }
}
