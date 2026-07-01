# group-theory domain

This domain provides pure calculation functions for finite group arithmetic. It covers modular addition and multiplication in the cyclic group Z_n, and the additive order of an element — the smallest positive multiple that reduces to the identity.

## Exposed functions

```rust
/// Returns (a + b) % n. Assumes n > 0.
pub fn modular_add(a: i64, b: i64, n: i64) -> i64;

/// Returns (a * b) % n. Assumes n > 0.
pub fn modular_mul(a: i64, b: i64, n: i64) -> i64;

/// Returns the smallest k > 0 such that k*a ≡ 0 (mod n). Equal to n / gcd(a, n).
/// Returns n when a ≡ 0 (mod n) (the identity element has order 1; by convention we return n
/// only if a == 0 to preserve group axioms for the additive group).
pub fn additive_order(a: i64, n: i64) -> i64;
```

## When to use this domain

Use this domain for lightweight modular arithmetic in cryptographic building blocks, clock-arithmetic problems, or cross-language parity tests over finite cyclic groups. It is intentionally free of big-integer or prime-checking dependencies.
