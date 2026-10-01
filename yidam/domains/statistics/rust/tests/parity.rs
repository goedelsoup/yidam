use yidam_domain_statistics::{mean, pearson_correlation, variance, z_score};

use yidam_domain_testkit::load_fixtures;

// ── statistics.mean ───────────────────────────────────────────────────────────

#[test]
fn parity_mean() {
    let fixtures = load_fixtures("statistics.mean");

    for fx in &fixtures {
        let input = &fx["input"];
        let values: Vec<f64> = input["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = mean(&values);
        let expected = fx["expected"]["mean"].as_float().unwrap();
        assert_eq!(result, expected, "mean mismatch for fixture");
    }
}

// ── statistics.variance ───────────────────────────────────────────────────────

#[test]
fn parity_variance() {
    let fixtures = load_fixtures("statistics.variance");

    for fx in &fixtures {
        let input = &fx["input"];
        let values: Vec<f64> = input["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = variance(&values);
        let expected = fx["expected"]["variance"].as_float().unwrap();
        assert_eq!(result, expected, "variance mismatch for fixture");
    }
}

// ── statistics.z_score ────────────────────────────────────────────────────────

#[test]
fn parity_z_score() {
    let fixtures = load_fixtures("statistics.z_score");

    for fx in &fixtures {
        let input = &fx["input"];
        let value = input["value"].as_float().unwrap();
        let m = input["mean"].as_float().unwrap();
        let std_dev = input["std_dev"].as_float().unwrap();
        let result = z_score(value, m, std_dev);
        let expected = fx["expected"]["z_score"].as_float().unwrap();
        assert_eq!(result, expected, "z_score mismatch for fixture");
    }
}

// ── statistics.pearson_correlation ────────────────────────────────────────────

#[test]
fn parity_pearson_correlation() {
    let fixtures = load_fixtures("statistics.pearson_correlation");

    for fx in &fixtures {
        let input = &fx["input"];
        let xs: Vec<f64> = input["xs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let ys: Vec<f64> = input["ys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = pearson_correlation(&xs, &ys);
        let expected = fx["expected"]["r"].as_float().unwrap();
        assert_eq!(result, expected, "pearson_correlation mismatch for fixture");
    }
}
