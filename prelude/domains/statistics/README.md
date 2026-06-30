# statistics domain

This domain provides pure calculation functions for descriptive statistics. It covers four core operations: computing the arithmetic mean, the population variance, a z-score, and the Pearson correlation coefficient between two variables.

## Exposed functions

```rust
/// Returns the arithmetic mean of the values. Returns 0.0 if the slice is empty.
pub fn mean(values: &[f64]) -> f64;

/// Returns the population variance (divided by n). Returns 0.0 if fewer than 2 values.
pub fn variance(values: &[f64]) -> f64;

/// Returns (value - mean) / std_dev.
pub fn z_score(value: f64, mean: f64, std_dev: f64) -> f64;

/// Returns the Pearson correlation coefficient (population covariance / (std_x * std_y)).
/// Returns 0.0 if either standard deviation is 0, if lengths differ, or if fewer than 2 values.
pub fn pearson_correlation(xs: &[f64], ys: &[f64]) -> f64;
```

## When to use this domain

Use this domain in repositories that need lightweight, dependency-free descriptive statistics that can be called from any language. It is intentionally free of modelling assumptions — no distributions, no inference — making it suitable as a building block for higher-level pipelines or as a reference implementation for cross-language parity tests.
