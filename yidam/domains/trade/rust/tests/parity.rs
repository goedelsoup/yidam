use yidam_domain_trade::{
    revealed_comparative_advantage, tariff_revenue, terms_of_trade, trade_balance,
};

use yidam_domain_testkit::load_fixtures;

#[test]
fn parity_trade_balance() {
    let fixtures = load_fixtures("trade.trade_balance");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = trade_balance(
            inp["exports"].as_float().unwrap(),
            inp["imports"].as_float().unwrap(),
        );
        let expected = fx["expected"]["balance"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_terms_of_trade() {
    let fixtures = load_fixtures("trade.terms_of_trade");
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
