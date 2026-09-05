use yidam_domain_set_theory::{difference, intersection, is_subset, union};

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

fn to_i64_vec(val: &toml::Value) -> Vec<i64> {
    val.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_integer().unwrap())
        .collect()
}

#[test]
fn parity_union() {
    let fixtures = load_fixtures("set_theory.union");
    assert!(!fixtures.is_empty(), "no set_theory.union fixtures found");
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
    assert!(
        !fixtures.is_empty(),
        "no set_theory.intersection fixtures found"
    );
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
    assert!(
        !fixtures.is_empty(),
        "no set_theory.difference fixtures found"
    );
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
    assert!(
        !fixtures.is_empty(),
        "no set_theory.is_subset fixtures found"
    );
    for fx in &fixtures {
        let a = to_i64_vec(&fx["input"]["a"]);
        let b = to_i64_vec(&fx["input"]["b"]);
        let expected = fx["expected"]["result"].as_bool().unwrap();
        assert_eq!(is_subset(&a, &b), expected);
    }
}
