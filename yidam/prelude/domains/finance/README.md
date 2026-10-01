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

The parity tests compare with an absolute `EPSILON = 1e-9`, in the currency unit of the
input.

A tolerance is needed because Rust computes `(1 + rate)^periods` with `powi`, which multiplies
repeatedly, while Python and TypeScript call `pow`. The two differ in the last digits:
`1.07^30` is `7.612255042662031` in Rust and `…042` in the other two. The error scales with
the result, so an absolute tolerance holds only up to a magnitude. At 7% over 30 periods it
passes for a principal of 10,000 and fails for 1,000,000, by 1.1e-8. No current fixture
reaches that far: all twelve use dyadic rates and are bit-identical in all three runtimes.
The move to a relative tolerance is #1281. The rule this follows is in
[Comparing floats](../parity/README.md#comparing-floats).
