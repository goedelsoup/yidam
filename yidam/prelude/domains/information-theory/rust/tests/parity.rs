use yidam_domain_information_theory::{entropy, kl_divergence};

use yidam_domain_testkit::load_fixtures;

// ── entropy ───────────────────────────────────────────────────────────────────

#[test]
fn parity_entropy() {
    let fixtures = load_fixtures("information_theory.entropy");

    for fx in &fixtures {
        let inp = &fx["input"];
        let probs: Vec<f64> = inp["probs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = entropy(&probs);
        let expected = fx["expected"]["entropy"].as_float().unwrap();
        assert_eq!(result, expected, "entropy({probs:?})");
    }
}

// ── kl_divergence ─────────────────────────────────────────────────────────────

#[test]
fn parity_kl_divergence() {
    let fixtures = load_fixtures("information_theory.kl_divergence");

    for fx in &fixtures {
        let inp = &fx["input"];
        let p: Vec<f64> = inp["p"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let q: Vec<f64> = inp["q"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();
        let result = kl_divergence(&p, &q);
        let expected = fx["expected"]["kl"].as_float().unwrap();
        assert_eq!(result, expected, "kl_divergence({p:?}, {q:?})");
    }
}
