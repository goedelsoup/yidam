# economics domain

This domain provides pure calculation functions for introductory microeconomics and macroeconomics. It covers the expenditure-side GDP identity, point price elasticity of demand, and opportunity cost.

## Exposed functions

```rust
/// Returns C + I + G + NX — gross domestic product via the expenditure approach.
pub fn gdp_expenditure(c: f64, i: f64, g: f64, nx: f64) -> f64;

/// Returns pct_qty_change / pct_price_change — point price elasticity of demand.
/// Returns 0.0 if pct_price_change is 0.
pub fn price_elasticity(pct_qty_change: f64, pct_price_change: f64) -> f64;

/// Returns foregone - chosen — the net opportunity cost of a choice expressed as
/// the value of the forgone alternative minus the value of the chosen alternative.
pub fn opportunity_cost(foregone: f64, chosen: f64) -> f64;
```

## When to use this domain

Use this domain for teaching or cross-language parity tests of introductory economic identities. The functions are dependency-free and make no assumptions about currency or units.
