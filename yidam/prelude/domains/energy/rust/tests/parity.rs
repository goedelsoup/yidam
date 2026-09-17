use yidam_domain_energy::{efficiency, kinetic_energy, potential_energy, power};

use yidam_domain_testkit::load_fixtures;

#[test]
fn parity_kinetic_energy() {
    let fixtures = load_fixtures("energy.kinetic_energy");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = kinetic_energy(
            inp["mass"].as_float().unwrap(),
            inp["velocity"].as_float().unwrap(),
        );
        let expected = fx["expected"]["joules"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_potential_energy() {
    let fixtures = load_fixtures("energy.potential_energy");
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
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = power(
            inp["work"].as_float().unwrap(),
            inp["time"].as_float().unwrap(),
        );
        let expected = fx["expected"]["watts"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn parity_efficiency() {
    let fixtures = load_fixtures("energy.efficiency");
    for fx in &fixtures {
        let inp = &fx["input"];
        let result = efficiency(
            inp["output"].as_float().unwrap(),
            inp["input"].as_float().unwrap(),
        );
        let expected = fx["expected"]["ratio"].as_float().unwrap();
        assert_eq!(result, expected);
    }
}
