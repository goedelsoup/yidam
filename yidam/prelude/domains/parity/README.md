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

## Comparing floats

**Exact by default.** The parity contract is bit-identical output: a domain's tests compare
an `f64` with `assert_eq!` in Rust, `==` in Python and `toBe()` in TypeScript. Eleven of the
fourteen domains do. That holds because IEEE-754 makes `+ − × ÷` and `sqrt` correctly
rounded, so three runtimes performing the same operations *in the same order* produce the
same bits. The order is part of the contract, not only the formula. A fused multiply-add, a
pairwise summation, or writing `x * π / 180` where the reference has `x * (π / 180)` is
correct and still different. That last one is measured in `geodesics` (#1280). When you
reimplement a function, follow the Rust reference's order of operations.

**Exact passes only where the fixtures let it.** `sin`, `cos`, `atan2`, `log2` and `pow`
are not required to be correctly rounded, and the three runtimes do not share one
implementation of them. Rust's `powi` also multiplies repeatedly where Python and TypeScript
call `pow`. `information-theory` calls `log2` and still compares exactly, because every
fixture it has is a power of two, and `log2` of a power of two is exact everywhere. Every
fixture whose result is exactly representable passes either way, which is the shape a
divergence would hide in. A non-dyadic fixture there would need a tolerance.

**Opting into a tolerance.** A domain whose functions call one of those on inputs where the
result is not exact may compare with a tolerance. It must:

1. Declare it under `## Parity tolerance` in the domain's README: the value, the **unit** it
   is measured in, and the function that makes it necessary. A bare `1e-4` does not say it
   means 10 cm, and the next domain to copy it would inherit the number without the reasoning.
2. Use the same value in all three languages, with a comment on each constant pointing to
   that section.
3. Choose the form. An absolute tolerance is a statement in the output's unit, and it holds
   only up to some magnitude. Where error grows with the result, as it does through `pow`,
   compare relatively with an absolute floor: `|a − b| ≤ max(abs, rel · |b|)`. The floor is
   needed because a relative tolerance is undefined at a `0.0` fixture.
4. Write expected values from the Rust reference at full precision, not as rounded literals.
   [`geodesics.haversine_km/paris-london.toml`](fixtures/geodesics.haversine_km/paris-london.toml)
   records what a two-decimal literal cost.

Three domains carry one today. `geodesics` is absolute; `finance` and `hydrology` are
relative with an absolute floor, as Python's `math.isclose` computes it, with the same formula
written out in Rust and TypeScript:

| domain | tolerance | unit of the absolute part | made necessary by |
|---|---|---|---|
| [`geodesics`](../geodesics/README.md#parity-tolerance) | `1e-4` absolute | km and degrees | `atan2` |
| [`finance`](../finance/README.md#parity-tolerance) | `1e-12` relative, `1e-9` floor | the currency of the input | `powi` against `pow` |
| [`hydrology`](../hydrology/README.md#parity-tolerance) | `1e-12` relative, `1e-9` floor | m/s, years, and the rational product | `powf(2/3)` |

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
outputs, or outputs within the domain's declared tolerance (see
[Comparing floats](#comparing-floats)). Any other divergence is a parity failure.

It is run by the `ci (domains)` job on every push and pull request — which it was not
until #682. Until then every mention of this task in the repository, including the one
you are reading, described a gate that no workflow invoked; and the task's own loop had
no `set -e`, so running it by hand exited 0 with a domain failing inside. What that cost
is recorded in #683: `geodesics.haversine_km` was wrong by 2 km, in the one fixture of
that function that was not a geometric identity, and nothing found it.

## Adding a domain function

1. Implement the function in all three language SDKs for the domain
2. Add at least one fixture to `fixtures/<domain>.<function>/`
3. Compare exactly, unless the function needs a tolerance under [Comparing floats](#comparing-floats)
4. Bump `VERSION` if the function's contract changes

Fixture filenames have no semantic meaning. Use kebab-case that describes the case
being exercised (`basic-linear`, `no-confounders`, `edge-empty-set`).
