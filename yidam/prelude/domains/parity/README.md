# Domain Parity

Cross-language fixture suite for domain calculator functions. Same discipline as
[`prelude/sdks/parity/`](../../sdks/parity/README.md) — same TOML format, same MUST
rule, Rust is always the reference implementation.

## What belongs here

Domain functions: pure mathematical or analytical operations that are specific to a
domain and shared across derived repos using that domain. Examples: causal effect
estimation, confounding scoring, information-theoretic metrics, graph centrality measures.

Core prelude functions (`parse_node`, `classify_commit`, etc.) stay in `prelude/sdks/parity/`.
Anything domain-specific goes here.

## Fixture format

Identical to `prelude/sdks/parity/` fixtures:

```toml
function = "<domain>.<function_name>"
description = "<what this case exercises>"

[input]
# function-specific fields

[expected]
# expected output
```

The `function` field is namespaced by domain to avoid collisions across domains:

```toml
function = "causal.estimate_effect"
function = "graph_metrics.betweenness_centrality"
```

Fixtures live under `fixtures/<domain>.<function_name>/<case>.toml`.

## The MUST rule

**Every domain function must have at least one fixture before the domain is usable.**

`mise run domain-parity` enforces this. A domain directory without at least one
fixture for every function it exposes fails the check.

## Directory layout

```
fixtures/
  <domain>.<function>/     ← one directory per function
    <descriptive-case>.toml
```

## Parity runner

```
mise run domain-parity
```

Runs all three language implementations of each domain function against every fixture
in this directory. Rust is the reference; TypeScript and Python must produce identical
outputs. Any divergence is a parity failure.

## Adding a domain function

1. Implement the function in all three language SDKs for the domain
2. Add at least one fixture to `fixtures/<domain>.<function>/`
3. Bump `VERSION` if the function's contract changes

Fixture filenames have no semantic meaning. Use kebab-case that describes the case
being exercised (`basic-linear`, `no-confounders`, `edge-empty-set`).
