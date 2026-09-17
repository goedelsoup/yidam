use yidam_domain_group_theory::{additive_order, modular_add, modular_mul};

use yidam_domain_testkit::load_fixtures;

#[test]
fn parity_modular_add() {
    let fixtures = load_fixtures("group_theory.modular_add");
    for fx in &fixtures {
        let inp = &fx["input"];
        let a = inp["a"].as_integer().unwrap();
        let b = inp["b"].as_integer().unwrap();
        let n = inp["n"].as_integer().unwrap();
        let expected = fx["expected"]["result"].as_integer().unwrap();
        assert_eq!(modular_add(a, b, n), expected);
    }
}

#[test]
fn parity_modular_mul() {
    let fixtures = load_fixtures("group_theory.modular_mul");
    for fx in &fixtures {
        let inp = &fx["input"];
        let a = inp["a"].as_integer().unwrap();
        let b = inp["b"].as_integer().unwrap();
        let n = inp["n"].as_integer().unwrap();
        let expected = fx["expected"]["result"].as_integer().unwrap();
        assert_eq!(modular_mul(a, b, n), expected);
    }
}

#[test]
fn parity_additive_order() {
    let fixtures = load_fixtures("group_theory.additive_order");
    for fx in &fixtures {
        let inp = &fx["input"];
        let a = inp["a"].as_integer().unwrap();
        let n = inp["n"].as_integer().unwrap();
        let expected = fx["expected"]["order"].as_integer().unwrap();
        assert_eq!(additive_order(a, n), expected);
    }
}
