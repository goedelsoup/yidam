# finance domain

This domain provides pure calculation functions for time-value-of-money and portfolio risk. It covers present value, future value under compound interest, simple interest, and the Sharpe ratio.

## Exposed functions

```rust
/// Returns fv / (1 + rate)^periods — present value of a future cash flow.
pub fn present_value(fv: f64, rate: f64, periods: u32) -> f64;

/// Returns pv * (1 + rate)^periods — future value of a present cash flow.
pub fn future_value(pv: f64, rate: f64, periods: u32) -> f64;

/// Returns principal * rate * time — simple (non-compounding) interest earned.
pub fn simple_interest(principal: f64, rate: f64, time: f64) -> f64;

/// Returns (ret - risk_free) / std_dev — Sharpe ratio. Returns 0.0 if std_dev is 0.
pub fn sharpe_ratio(ret: f64, risk_free: f64, std_dev: f64) -> f64;
```

## When to use this domain

Use this domain for dependency-free time-value calculations or cross-language parity tests of financial formulae. It assumes annual compounding and makes no calendar assumptions.

## Parity tolerance

The parity tests compare relatively, with an absolute floor:
`|a − b| ≤ max(1e-12 · max(|a|, |b|), 1e-9)`, the floor in the currency unit of the input.

A tolerance is needed because Rust computes `(1 + rate)^periods` with `powi`, which multiplies
repeatedly, while Python and TypeScript call `pow`. The two differ in the last digits, and the
difference scales with the result, which is why the comparison is relative.
`future_value/million-at-seven-percent` lands 1.1e-8 apart, ten times the absolute `1e-9`
this domain used until #1281, and `future_value/thirty-years-monthly` 9.8e-8 apart, which is
1.6e-14 relative. That is the largest divergence measured here, and `1e-12` leaves it about
sixty times the room. The floor is for the two fixtures that expect `0.0`, where a relative
tolerance alone admits nothing. The rule this follows is in
[Comparing floats](../parity/README.md#comparing-floats).
