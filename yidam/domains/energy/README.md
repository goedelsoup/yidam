# energy domain

This domain provides pure calculation functions for classical mechanics and energy conversion. It covers kinetic and potential energy under constant gravity, mechanical power, and thermal/mechanical efficiency.

## Exposed functions

```rust
/// Returns 0.5 * mass * velocity². Returns 0.0 if velocity is 0.
pub fn kinetic_energy(mass: f64, velocity: f64) -> f64;

/// Returns mass * g * height.
pub fn potential_energy(mass: f64, height: f64, g: f64) -> f64;

/// Returns work / time. Returns 0.0 if time is 0.
pub fn power(work: f64, time: f64) -> f64;

/// Returns output / input as a dimensionless ratio. Returns 0.0 if input is 0.
pub fn efficiency(output: f64, input: f64) -> f64;
```

## When to use this domain

Use this domain for lightweight physics back-of-envelope calculations or cross-language parity tests of classical energy formulae. All functions are dependency-free pure calculations.
