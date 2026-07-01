# causal domain

This domain provides pure calculation functions for observational causal inference. It covers two core estimation tasks: computing the Average Treatment Effect (ATE) between a treated and a control group, and producing a proxy score for confounding severity given correlations between a variable and both treatment assignment and the outcome of interest.

## Exposed functions

```rust
/// Returns mean(treated) - mean(control). Returns 0.0 if either slice is empty.
pub fn ate(treated: &[f64], control: &[f64]) -> f64;

/// Returns abs(r_treatment * r_outcome) as a proxy for confounding strength.
pub fn confounding_score(r_treatment: f64, r_outcome: f64) -> f64;
```

The `ate` function implements the simplest unbiased estimator for a binary treatment under randomization. The `confounding_score` function multiplies the correlation of a candidate confounder with treatment assignment by its correlation with the outcome, then takes the absolute value — yielding a scalar in [0, 1] that grows as the variable's potential to bias an unadjusted ATE estimate increases.

## When to use this domain

Use this domain in observational causal inference repositories where you need lightweight, dependency-free estimators that can be called from any language. It is intentionally free of statistical modelling assumptions — no propensity scoring, no regression adjustment — making it suitable as a building block for higher-level pipelines or as a reference implementation for cross-language parity tests.
