use yidam_domain_finance::{future_value, present_value, sharpe_ratio, simple_interest};

const EPSILON: f64 = 1e-9;

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
fn parity_present_value() {
    let fixtures = load_fixtures("finance.present_value");
    assert!(
        !fixtures.is_empty(),
        "no finance.present_value fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = present_value(
            inp["fv"].as_float().unwrap(),
            inp["rate"].as_float().unwrap(),
            inp["periods"].as_integer().unwrap() as u32,
        );
        let expected = fx["expected"]["pv"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "present_value: got {result}, expected {expected}"
        );
    }
}

#[test]
fn parity_future_value() {
    let fixtures = load_fixtures("finance.future_value");
    assert!(
        !fixtures.is_empty(),
        "no finance.future_value fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = future_value(
            inp["pv"].as_float().unwrap(),
            inp["rate"].as_float().unwrap(),
            inp["periods"].as_integer().unwrap() as u32,
        );
        let expected = fx["expected"]["fv"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "future_value: got {result}, expected {expected}"
        );
    }
}

#[test]
fn parity_simple_interest() {
    let fixtures = load_fixtures("finance.simple_interest");
    assert!(
        !fixtures.is_empty(),
        "no finance.simple_interest fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = simple_interest(
            inp["principal"].as_float().unwrap(),
            inp["rate"].as_float().unwrap(),
            inp["time"].as_float().unwrap(),
        );
        let expected = fx["expected"]["interest"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "simple_interest: got {result}, expected {expected}"
        );
    }
}

#[test]
fn parity_sharpe_ratio() {
    let fixtures = load_fixtures("finance.sharpe_ratio");
    assert!(
        !fixtures.is_empty(),
        "no finance.sharpe_ratio fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = sharpe_ratio(
            inp["ret"].as_float().unwrap(),
            inp["risk_free"].as_float().unwrap(),
            inp["std_dev"].as_float().unwrap(),
        );
        let expected = fx["expected"]["ratio"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "sharpe_ratio: got {result}, expected {expected}"
        );
    }
}
