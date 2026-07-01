# set-theory domain

This domain provides pure set operations over integer sequences. All functions treat their inputs as mathematical sets — duplicates are removed and outputs are returned in sorted ascending order.

## Exposed functions

```rust
/// Returns all elements that appear in a or b (deduplicated, sorted).
pub fn union(a: &[i64], b: &[i64]) -> Vec<i64>;

/// Returns elements that appear in both a and b (deduplicated, sorted).
pub fn intersection(a: &[i64], b: &[i64]) -> Vec<i64>;

/// Returns elements that are in a but not in b (deduplicated, sorted).
pub fn difference(a: &[i64], b: &[i64]) -> Vec<i64>;

/// Returns true if every element of a is also in b.
pub fn is_subset(a: &[i64], b: &[i64]) -> bool;
```

## When to use this domain

Use this domain when you need dependency-free, cross-language set primitives — membership testing, overlap detection, or subset relationships — without pulling in a full collections library.
