# hydrology domain

This domain provides pure calculation functions for surface-water hydrology. It covers the rational-method product used in peak-discharge estimation, the Manning velocity for open-channel flow, and the Weibull return-period formula for flood-frequency analysis.

## Exposed functions

```rust
/// Returns C * i * A — the product of runoff coefficient, rainfall intensity, and catchment area.
/// Multiply by an appropriate unit factor (e.g. 1/360 for SI discharge in m³/s from mm/hr and ha).
pub fn rational_product(c: f64, i: f64, a: f64) -> f64;

/// Returns (1/n) * R^(2/3) * S^(1/2) — cross-sectional mean velocity via Manning's equation.
/// n is Manning's roughness, R is hydraulic radius (m), S is channel slope (m/m).
pub fn manning_velocity(n: f64, r: f64, s: f64) -> f64;

/// Returns (record_years + 1) / rank — Weibull return period in years for a ranked flood series.
pub fn return_period(record_years: f64, rank: f64) -> f64;
```

## When to use this domain

Use this domain for dependency-free hydrological back-of-envelope calculations or cross-language parity testing of water-resources formulae. It makes no assumptions about units beyond what is documented per function.
