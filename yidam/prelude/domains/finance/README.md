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
