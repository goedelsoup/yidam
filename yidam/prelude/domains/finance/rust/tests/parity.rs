use yidam_domain_finance::{future_value, present_value, sharpe_ratio, simple_interest};

const EPSILON: f64 = 1e-9;

use yidam_domain_testkit::load_fixtures;

#[test]
fn parity_present_value() {
    let fixtures = load_fixtures("finance.present_value");
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
