use yidam_domain_statistics::{mean, pearson_correlation, variance, z_score};

fn fixture_dir(function: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = prelude/domains/statistics/rust/
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

// ── statistics.mean ───────────────────────────────────────────────────────────

#[test]
fn parity_mean() {
    let fixtures = load_fixtures("statistics.mean");
    assert!(!fixtures.is_empty(), "no statistics.mean fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let values: Vec<f64> = input["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = mean(&values);
        let expected = fx["expected"]["mean"].as_float().unwrap();
        assert_eq!(result, expected, "mean mismatch for fixture");
    }
}

// ── statistics.variance ───────────────────────────────────────────────────────

#[test]
fn parity_variance() {
    let fixtures = load_fixtures("statistics.variance");
    assert!(
        !fixtures.is_empty(),
        "no statistics.variance fixtures found"
    );

    for fx in &fixtures {
        let input = &fx["input"];
        let values: Vec<f64> = input["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = variance(&values);
        let expected = fx["expected"]["variance"].as_float().unwrap();
        assert_eq!(result, expected, "variance mismatch for fixture");
    }
}

// ── statistics.z_score ────────────────────────────────────────────────────────

#[test]
fn parity_z_score() {
    let fixtures = load_fixtures("statistics.z_score");
    assert!(!fixtures.is_empty(), "no statistics.z_score fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let value = input["value"].as_float().unwrap();
        let m = input["mean"].as_float().unwrap();
        let std_dev = input["std_dev"].as_float().unwrap();
        let result = z_score(value, m, std_dev);
        let expected = fx["expected"]["z_score"].as_float().unwrap();
        assert_eq!(result, expected, "z_score mismatch for fixture");
    }
}

// ── statistics.pearson_correlation ────────────────────────────────────────────

#[test]
fn parity_pearson_correlation() {
    let fixtures = load_fixtures("statistics.pearson_correlation");
    assert!(
        !fixtures.is_empty(),
        "no statistics.pearson_correlation fixtures found"
    );

    for fx in &fixtures {
        let input = &fx["input"];
        let xs: Vec<f64> = input["xs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let ys: Vec<f64> = input["ys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = pearson_correlation(&xs, &ys);
        let expected = fx["expected"]["r"].as_float().unwrap();
        assert_eq!(result, expected, "pearson_correlation mismatch for fixture");
    }
}
