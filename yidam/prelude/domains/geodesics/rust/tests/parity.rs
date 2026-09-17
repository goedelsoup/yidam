use yidam_domain_geodesics::{bearing_deg, central_angle_deg, haversine_km};

const EPSILON: f64 = 1e-4;

use yidam_domain_testkit::load_fixtures;

#[test]
fn parity_haversine_km() {
    let fixtures = load_fixtures("geodesics.haversine_km");
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
