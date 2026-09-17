use yidam_domain_set_theory::{difference, intersection, is_subset, union};

use yidam_domain_testkit::load_fixtures;

fn to_i64_vec(val: &yidam_domain_testkit::toml::Value) -> Vec<i64> {
    val.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_integer().unwrap())
        .collect()
}

#[test]
fn parity_union() {
    let fixtures = load_fixtures("set_theory.union");
    for fx in &fixtures {
        let a = to_i64_vec(&fx["input"]["a"]);
        let b = to_i64_vec(&fx["input"]["b"]);
        let expected = to_i64_vec(&fx["expected"]["elements"]);
        assert_eq!(union(&a, &b), expected);
    }
}

#[test]
fn parity_intersection() {
    let fixtures = load_fixtures("set_theory.intersection");
    for fx in &fixtures {
        let a = to_i64_vec(&fx["input"]["a"]);
        let b = to_i64_vec(&fx["input"]["b"]);
        let expected = to_i64_vec(&fx["expected"]["elements"]);
        assert_eq!(intersection(&a, &b), expected);
    }
}

#[test]
fn parity_difference() {
    let fixtures = load_fixtures("set_theory.difference");
    for fx in &fixtures {
        let a = to_i64_vec(&fx["input"]["a"]);
        let b = to_i64_vec(&fx["input"]["b"]);
        let expected = to_i64_vec(&fx["expected"]["elements"]);
        assert_eq!(difference(&a, &b), expected);
    }
}

#[test]
fn parity_is_subset() {
    let fixtures = load_fixtures("set_theory.is_subset");
    for fx in &fixtures {
        let a = to_i64_vec(&fx["input"]["a"]);
        let b = to_i64_vec(&fx["input"]["b"]);
        let expected = fx["expected"]["result"].as_bool().unwrap();
        assert_eq!(is_subset(&a, &b), expected);
    }
}
