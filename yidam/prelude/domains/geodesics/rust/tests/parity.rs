use yidam_domain_geodesics::{haversine_km, bearing_deg, central_angle_deg};

const EPSILON: f64 = 1e-4;

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
fn parity_haversine_km() {
    let fixtures = load_fixtures("geodesics.haversine_km");
    assert!(!fixtures.is_empty(), "no geodesics.haversine_km fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = haversine_km(
            inp["lat1"].as_float().unwrap(),
            inp["lon1"].as_float().unwrap(),
            inp["lat2"].as_float().unwrap(),
            inp["lon2"].as_float().unwrap(),
        );
        let expected = fx["expected"]["km"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "haversine_km: got {result}, expected {expected}"
        );
    }
}

#[test]
fn parity_bearing_deg() {
    let fixtures = load_fixtures("geodesics.bearing_deg");
    assert!(!fixtures.is_empty(), "no geodesics.bearing_deg fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = bearing_deg(
            inp["lat1"].as_float().unwrap(),
            inp["lon1"].as_float().unwrap(),
            inp["lat2"].as_float().unwrap(),
            inp["lon2"].as_float().unwrap(),
        );
        let expected = fx["expected"]["degrees"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "bearing_deg: got {result}, expected {expected}"
        );
    }
}

#[test]
fn parity_central_angle_deg() {
    let fixtures = load_fixtures("geodesics.central_angle_deg");
    assert!(!fixtures.is_empty(), "no geodesics.central_angle_deg fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = central_angle_deg(
            inp["lat1"].as_float().unwrap(),
            inp["lon1"].as_float().unwrap(),
            inp["lat2"].as_float().unwrap(),
            inp["lon2"].as_float().unwrap(),
        );
        let expected = fx["expected"]["degrees"].as_float().unwrap();
        assert!(
            (result - expected).abs() < EPSILON,
            "central_angle_deg: got {result}, expected {expected}"
        );
    }
}
