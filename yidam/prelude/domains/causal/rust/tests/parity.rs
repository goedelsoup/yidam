use yidam_domain_causal::{ate, confounding_score};

use yidam_domain_testkit::load_fixtures;

// ── causal.ate ────────────────────────────────────────────────────────────────

#[test]
fn parity_ate() {
    let fixtures = load_fixtures("causal.ate");

    for fx in &fixtures {
        let input = &fx["input"];
        let treated: Vec<f64> = input["treated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let control: Vec<f64> = input["control"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = ate(&treated, &control);
        let expected = fx["expected"]["ate"].as_float().unwrap();
        assert_eq!(result, expected, "ATE mismatch for fixture");
    }
}

// ── causal.confounding_score ──────────────────────────────────────────────────

#[test]
fn parity_confounding_score() {
    let fixtures = load_fixtures("causal.confounding_score");

    for fx in &fixtures {
        let input = &fx["input"];
        let r_treatment = input["r_treatment"].as_float().unwrap();
        let r_outcome = input["r_outcome"].as_float().unwrap();
        let result = confounding_score(r_treatment, r_outcome);
        let expected = fx["expected"]["score"].as_float().unwrap();
        assert_eq!(result, expected, "confounding_score mismatch for fixture");
    }
}
