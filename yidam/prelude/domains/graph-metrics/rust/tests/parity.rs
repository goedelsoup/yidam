use yidam_domain_graph_metrics::{degree_centrality, density};

fn fixture_dir(function: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = prelude/domains/graph-metrics/rust/
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

// ── density ───────────────────────────────────────────────────────────────────

#[test]
fn parity_density() {
    let fixtures = load_fixtures("graph_metrics.density");
    assert!(
        !fixtures.is_empty(),
        "no graph_metrics.density fixtures found"
    );

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
    assert!(
        !fixtures.is_empty(),
        "no graph_metrics.degree_centrality fixtures found"
    );

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
