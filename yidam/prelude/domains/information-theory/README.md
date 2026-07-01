# information-theory

This domain computes information-theoretic metrics over probability distributions. It provides pure mathematical functions for quantifying uncertainty, divergence, and information content — the core building blocks for reasoning about probability distributions in the context of corpus analysis and knowledge graph construction.

## Exposed functions

```rust
/// Shannon entropy in bits. Returns -sum(p * log2(p)) for all p > 0.
/// By convention, 0 * log2(0) = 0 (zero-probability terms contribute nothing).
fn entropy(probs: &[f64]) -> f64

/// KL divergence D(P||Q) in bits. Returns sum(p_i * log2(p_i / q_i)) for all i where p_i > 0.
/// Terms where p_i = 0 are skipped (they contribute zero). Undefined if q_i = 0 and p_i > 0.
fn kl_divergence(p: &[f64], q: &[f64]) -> f64
```

## When to use

Use this domain for corpus uncertainty analysis — for example, measuring how evenly distributed evidence tags are across a set of claims (high entropy = diverse evidence; low entropy = concentrated evidence). `kl_divergence` is useful for comparing two claim distributions, such as detecting how much a document's evidence profile diverges from a reference corpus. Both functions are appropriate for assessing diversity of evidence tags, evaluating prior vs. posterior belief shifts, and surfacing outliers in claim or link distributions.
