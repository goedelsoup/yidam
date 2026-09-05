use yidam_domain_hydrology::{manning_velocity, rational_product, return_period};

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
fn parity_rational_product() {
    let fixtures = load_fixtures("hydrology.rational_product");
    assert!(
        !fixtures.is_empty(),
        "no hydrology.rational_product fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = rational_product(
            inp["c"].as_float().unwrap(),
            inp["i"].as_float().unwrap(),
            inp["a"].as_float().unwrap(),
        );
        let expected = fx["expected"]["result"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "rational_product: got {result}, expected {expected}"
        );
    }
}

#[test]
fn parity_manning_velocity() {
    let fixtures = load_fixtures("hydrology.manning_velocity");
    assert!(
        !fixtures.is_empty(),
        "no hydrology.manning_velocity fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = manning_velocity(
            inp["n"].as_float().unwrap(),
            inp["r"].as_float().unwrap(),
            inp["s"].as_float().unwrap(),
        );
        let expected = fx["expected"]["velocity"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "manning_velocity: got {result}, expected {expected}"
        );
    }
}

#[test]
fn parity_return_period() {
    let fixtures = load_fixtures("hydrology.return_period");
    assert!(
        !fixtures.is_empty(),
        "no hydrology.return_period fixtures found"
    );
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = return_period(
            inp["record_years"].as_float().unwrap(),
            inp["rank"].as_float().unwrap(),
        );
        let expected = fx["expected"]["years"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "return_period: got {result}, expected {expected}"
        );
    }
}
