use yidam_domain_causal::{ate, confounding_score};

fn fixture_dir(function: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = prelude/domains/causal/rust/
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

// ── causal.ate ────────────────────────────────────────────────────────────────

#[test]
fn parity_ate() {
    let fixtures = load_fixtures("causal.ate");
    assert!(!fixtures.is_empty(), "no causal.ate fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let treated: Vec<f64> = input["treated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let control: Vec<f64> = input["control"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = ate(&treated, &control);
        let expected = fx["expected"]["ate"].as_float().unwrap();
        assert_eq!(result, expected, "ATE mismatch for fixture");
    }
}

// ── causal.confounding_score ──────────────────────────────────────────────────

#[test]
fn parity_confounding_score() {
    let fixtures = load_fixtures("causal.confounding_score");
    assert!(!fixtures.is_empty(), "no causal.confounding_score fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let r_treatment = input["r_treatment"].as_float().unwrap();
        let r_outcome = input["r_outcome"].as_float().unwrap();
        let result = confounding_score(r_treatment, r_outcome);
        let expected = fx["expected"]["score"].as_float().unwrap();
        assert_eq!(result, expected, "confounding_score mismatch for fixture");
    }
}
