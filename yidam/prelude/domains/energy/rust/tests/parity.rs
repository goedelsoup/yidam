use yidam_domain_energy::{kinetic_energy, potential_energy, power, efficiency};

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
fn parity_kinetic_energy() {
    let fixtures = load_fixtures("energy.kinetic_energy");
    assert!(!fixtures.is_empty(), "no energy.kinetic_energy fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = kinetic_energy(inp["mass"].as_float().unwrap(), inp["velocity"].as_float().unwrap());
        let expected = fx["expected"]["joules"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_potential_energy() {
    let fixtures = load_fixtures("energy.potential_energy");
    assert!(!fixtures.is_empty(), "no energy.potential_energy fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = potential_energy(
            inp["mass"].as_float().unwrap(),
            inp["height"].as_float().unwrap(),
            inp["g"].as_float().unwrap(),
        );
        let expected = fx["expected"]["joules"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_power() {
    let fixtures = load_fixtures("energy.power");
    assert!(!fixtures.is_empty(), "no energy.power fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = power(inp["work"].as_float().unwrap(), inp["time"].as_float().unwrap());
        let expected = fx["expected"]["watts"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_efficiency() {
    let fixtures = load_fixtures("energy.efficiency");
    assert!(!fixtures.is_empty(), "no energy.efficiency fixtures found");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = efficiency(inp["output"].as_float().unwrap(), inp["input"].as_float().unwrap());
        let expected = fx["expected"]["ratio"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}
