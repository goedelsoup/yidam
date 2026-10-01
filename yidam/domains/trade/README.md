# trade domain

This domain provides pure calculation functions for international trade analysis. It covers the trade balance, terms of trade index, tariff revenue estimation, and the Balassa revealed comparative advantage index.

## Exposed functions

```rust
/// Returns exports - imports. Positive indicates a trade surplus; negative a deficit.
pub fn trade_balance(exports: f64, imports: f64) -> f64;

/// Returns (export_index / import_index) * 100 — terms of trade as an index number.
/// Returns 0.0 if import_index is 0.
pub fn terms_of_trade(export_index: f64, import_index: f64) -> f64;

/// Returns import_value * rate — estimated tariff revenue.
pub fn tariff_revenue(import_value: f64, rate: f64) -> f64;

/// Returns country_share / world_share — Balassa RCA index.
/// Values > 1 indicate comparative advantage. Returns 0.0 if world_share is 0.
pub fn revealed_comparative_advantage(country_share: f64, world_share: f64) -> f64;
```

## When to use this domain

Use this domain for dependency-free trade-economics calculations or cross-language parity tests of international trade identities. All functions are unit-agnostic.
