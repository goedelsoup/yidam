# similarity domain

This domain provides pure calculation functions for measuring similarity between data. It covers three core tasks: cosine similarity between float vectors, Jaccard similarity between string sets, and Levenshtein edit distance between strings.

## Exposed functions

```rust
/// Cosine similarity of two float vectors. Returns 0.0 if either is a zero vector.
pub fn cosine(a: &[f64], b: &[f64]) -> f64;

/// Jaccard similarity of two string sets: |intersection| / |union|. Returns 0.0 if both empty.
pub fn jaccard(a: &[&str], b: &[&str]) -> f64;

/// Levenshtein edit distance (insert/delete/substitute each cost 1). Returns 0 for identical strings.
pub fn edit_distance(s1: &str, s2: &str) -> usize;
```

The `cosine` function computes the standard dot-product-over-norms formula, with a zero-vector guard. The `jaccard` function converts slices to sets before computing intersection and union, so duplicate elements are deduplicated. The `edit_distance` function implements the classic Wagner-Fischer dynamic programming algorithm with O(n) space.

## When to use this domain

Use this domain when you need lightweight, dependency-free similarity metrics that can be called from any language. It is intentionally free of model or embedding assumptions — no ML dependencies, no external calls — making it suitable as a building block for ranking, deduplication, fuzzy matching, or cross-language parity tests.
