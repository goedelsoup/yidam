use yidam_domain_economics::{gdp_expenditure, opportunity_cost, price_elasticity};

use yidam_domain_testkit::load_fixtures;

#[test]
fn parity_gdp_expenditure() {
    let fixtures = load_fixtures("economics.gdp_expenditure");
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
