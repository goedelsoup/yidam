use yidam_domain_hydrology::{manning_velocity, rational_product, return_period};

// Relative, with an absolute floor in each function's output unit, as Python's math.isclose computes it.
// Why this domain is not exact: hydrology/README.md#parity-tolerance
const REL_TOL: f64 = 1e-12;
const ABS_TOL: f64 = 1e-9;

fn is_close(a: f64, b: f64) -> bool {
    (a - b).abs() <= (REL_TOL * a.abs().max(b.abs())).max(ABS_TOL)
}

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
            is_close(result, expected),
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
            is_close(result, expected),
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
            is_close(result, expected),
            "return_period: got {result}, expected {expected}"
        );
    }
}
