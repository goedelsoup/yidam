use yidam_domain_hydrology::{manning_velocity, rational_product, return_period};

const EPSILON: f64 = 1e-9;

use yidam_domain_testkit::load_fixtures;

#[test]
fn parity_rational_product() {
    let fixtures = load_fixtures("hydrology.rational_product");
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
