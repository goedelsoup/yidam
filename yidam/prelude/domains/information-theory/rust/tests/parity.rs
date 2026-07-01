use yidam_domain_information_theory::{entropy, kl_divergence};

fn fixture_dir(function: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = prelude/domains/information-theory/rust/
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

// ── entropy ───────────────────────────────────────────────────────────────────

#[test]
fn parity_entropy() {
    let fixtures = load_fixtures("information_theory.entropy");
    assert!(!fixtures.is_empty(), "no information_theory.entropy fixtures found");

    for fx in &fixtures {
        let inp = &fx["input"];
        let probs: Vec<f64> = inp["probs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = entropy(&probs);
        let expected = fx["expected"]["entropy"].as_float().unwrap();
        assert_eq!(result, expected, "entropy({probs:?})");
    }
}

// ── kl_divergence ─────────────────────────────────────────────────────────────

#[test]
fn parity_kl_divergence() {
    let fixtures = load_fixtures("information_theory.kl_divergence");
    assert!(!fixtures.is_empty(), "no information_theory.kl_divergence fixtures found");

    for fx in &fixtures {
        let inp = &fx["input"];
        let p: Vec<f64> = inp["p"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let q: Vec<f64> = inp["q"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = kl_divergence(&p, &q);
        let expected = fx["expected"]["kl"].as_float().unwrap();
        assert_eq!(result, expected, "kl_divergence({p:?}, {q:?})");
    }
}
