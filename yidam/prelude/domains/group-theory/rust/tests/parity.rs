use yidam_domain_group_theory::{additive_order, modular_add, modular_mul};

fn fixture_dir(function: &str) -> std::path::PathBuf {
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

#[test]
fn parity_modular_add() {
    let fixtures = load_fixtures("group_theory.modular_add");
    assert!(
        !fixtures.is_empty(),
        "no group_theory.modular_add fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let a = inp["a"].as_integer().unwrap();
        let b = inp["b"].as_integer().unwrap();
        let n = inp["n"].as_integer().unwrap();
        let expected = fx["expected"]["result"].as_integer().unwrap();
        assert_eq!(modular_add(a, b, n), expected);
    }
}

#[test]
fn parity_modular_mul() {
    let fixtures = load_fixtures("group_theory.modular_mul");
    assert!(
        !fixtures.is_empty(),
        "no group_theory.modular_mul fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let a = inp["a"].as_integer().unwrap();
        let b = inp["b"].as_integer().unwrap();
        let n = inp["n"].as_integer().unwrap();
        let expected = fx["expected"]["result"].as_integer().unwrap();
        assert_eq!(modular_mul(a, b, n), expected);
    }
}

#[test]
fn parity_additive_order() {
    let fixtures = load_fixtures("group_theory.additive_order");
    assert!(
        !fixtures.is_empty(),
        "no group_theory.additive_order fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let a = inp["a"].as_integer().unwrap();
        let n = inp["n"].as_integer().unwrap();
        let expected = fx["expected"]["order"].as_integer().unwrap();
        assert_eq!(additive_order(a, n), expected);
    }
}
