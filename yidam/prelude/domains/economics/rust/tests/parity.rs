use yidam_domain_economics::{gdp_expenditure, price_elasticity, opportunity_cost};

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
fn parity_gdp_expenditure() {
    let fixtures = load_fixtures("economics.gdp_expenditure");
    assert!(!fixtures.is_empty(), "no economics.gdp_expenditure fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = gdp_expenditure(
            inp["c"].as_float().unwrap(),
            inp["i"].as_float().unwrap(),
            inp["g"].as_float().unwrap(),
            inp["nx"].as_float().unwrap(),
        );
        let expected = fx["expected"]["gdp"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_price_elasticity() {
    let fixtures = load_fixtures("economics.price_elasticity");
    assert!(!fixtures.is_empty(), "no economics.price_elasticity fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = price_elasticity(
            inp["pct_qty_change"].as_float().unwrap(),
            inp["pct_price_change"].as_float().unwrap(),
        );
        let expected = fx["expected"]["elasticity"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_opportunity_cost() {
    let fixtures = load_fixtures("economics.opportunity_cost");
    assert!(!fixtures.is_empty(), "no economics.opportunity_cost fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = opportunity_cost(
            inp["foregone"].as_float().unwrap(),
            inp["chosen"].as_float().unwrap(),
        );
        let expected = fx["expected"]["cost"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}
