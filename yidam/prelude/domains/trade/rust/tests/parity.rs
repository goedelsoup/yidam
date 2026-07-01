use yidam_domain_trade::{trade_balance, terms_of_trade, tariff_revenue, revealed_comparative_advantage};

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
fn parity_trade_balance() {
    let fixtures = load_fixtures("trade.trade_balance");
    assert!(!fixtures.is_empty(), "no trade.trade_balance fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = trade_balance(inp["exports"].as_float().unwrap(), inp["imports"].as_float().unwrap());
        let expected = fx["expected"]["balance"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_terms_of_trade() {
    let fixtures = load_fixtures("trade.terms_of_trade");
    assert!(!fixtures.is_empty(), "no trade.terms_of_trade fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = terms_of_trade(
            inp["export_index"].as_float().unwrap(),
            inp["import_index"].as_float().unwrap(),
        );
        let expected = fx["expected"]["tot"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_tariff_revenue() {
    let fixtures = load_fixtures("trade.tariff_revenue");
    assert!(!fixtures.is_empty(), "no trade.tariff_revenue fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = tariff_revenue(
            inp["import_value"].as_float().unwrap(),
            inp["rate"].as_float().unwrap(),
        );
        let expected = fx["expected"]["revenue"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_revealed_comparative_advantage() {
    let fixtures = load_fixtures("trade.revealed_comparative_advantage");
    assert!(!fixtures.is_empty(), "no trade.revealed_comparative_advantage fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = revealed_comparative_advantage(
            inp["country_share"].as_float().unwrap(),
            inp["world_share"].as_float().unwrap(),
        );
        let expected = fx["expected"]["rca"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}
