use yidam_domain_similarity::{cosine, edit_distance, jaccard};

fn fixture_dir(function: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = prelude/domains/similarity/rust/
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

// ── similarity.cosine ─────────────────────────────────────────────────────────

#[test]
fn parity_cosine() {
    let fixtures = load_fixtures("similarity.cosine");
    assert!(!fixtures.is_empty(), "no similarity.cosine fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let a: Vec<f64> = input["a"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let b: Vec<f64> = input["b"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = cosine(&a, &b);
        let expected = fx["expected"]["similarity"].as_float().unwrap();
        assert_eq!(result, expected, "cosine mismatch for fixture");
    }
}

// ── similarity.jaccard ────────────────────────────────────────────────────────

#[test]
fn parity_jaccard() {
    let fixtures = load_fixtures("similarity.jaccard");
    assert!(!fixtures.is_empty(), "no similarity.jaccard fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let a_strings: Vec<String> = input["a"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let b_strings: Vec<String> = input["b"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let a_refs: Vec<&str> = a_strings.iter().map(|s| s.as_str()).collect();
        let b_refs: Vec<&str> = b_strings.iter().map(|s| s.as_str()).collect();
        let result = jaccard(&a_refs, &b_refs);
        let expected = fx["expected"]["similarity"].as_float().unwrap();
        assert_eq!(result, expected, "jaccard mismatch for fixture");
    }
}

// ── similarity.edit_distance ──────────────────────────────────────────────────

#[test]
fn parity_edit_distance() {
    let fixtures = load_fixtures("similarity.edit_distance");
    assert!(
        !fixtures.is_empty(),
        "no similarity.edit_distance fixtures found"
    );

    for fx in &fixtures {
        let input = &fx["input"];
        let s1 = input["s1"].as_str().unwrap();
        let s2 = input["s2"].as_str().unwrap();
        let result = edit_distance(s1, s2);
        let expected = fx["expected"]["distance"].as_integer().unwrap() as usize;
        assert_eq!(result, expected, "edit_distance mismatch for fixture");
    }
}
