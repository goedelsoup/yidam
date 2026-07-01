# Prelude Domains

Pure function libraries for domain-specific calculations, shared across yidam-derived
repositories that work in the same domain. An extension of the `prelude/sdks/` layer —
same three languages, same parity discipline, Rust as reference.

---

## What this layer is

`prelude/sdks/` gives every derived repo the six core prelude operations: parsing corpus
nodes, classifying commits, finding markers, and so on. These are universal — every repo
needs them regardless of domain.

`prelude/domains/` adds the domain-specific math. A repository working on causal inference
needs tools for estimating treatment effects. One working on graph-theoretic linguistics
needs centrality and clustering measures. These are not universal, but they are *reusable*
across repos in the same domain — and they are pure functions, the same in Rust, TypeScript,
and Python, verified by the same parity fixture discipline.

This is the layer between the generic prelude and the domain computer (`crates/`). Domain
functions live here when they are:

- Pure: same input always produces the same output, no I/O
- Cross-language: needed in Rust (precision computation), TypeScript (agent context), and
  Python (embedding pipeline, statistical analysis)
- Reusable: likely to be shared across more than one derived repo in the same domain

One-off calculations specific to a single repo live in `crates/` or `packages/`, not here.

---

## Relationship to samudaya

When a derived repo is bootstrapped with a domain-specific `samudaya/` configuration, the
bootstrap agent activates the relevant domain from `prelude/domains/` by wiring it into the
repo's language workspaces (`crates/Cargo.toml`, `packages/`, etc.). The domain functions
become available to the corpus agents, calculators, and index pipeline from that point.

`prelude/domains/` does not itself depend on any domain. It is the template; derived repos
consume it.

---

## Directory structure

```
prelude/domains/
  README.md               ← this document
  parity/                 ← cross-language fixture suite (same format as sdks/parity/)
    VERSION               ← version of the domain parity surface
    README.md
    fixtures/
      <domain>.<function>/
        <case>.toml
  <domain>/               ← one directory per domain
    README.md             ← what this domain computes; which functions it exposes
    rust/                 ← Rust implementation (reference)
    typescript/           ← TypeScript implementation (must match Rust via parity)
    python/               ← Python implementation (must match Rust via parity)
    spec/                 ← optional Dafny invariants for domain-specific properties
```

---

## The parity contract

Domain functions follow the same cross-language parity discipline as `prelude/sdks/`:

- Rust is always the reference implementation
- TypeScript and Python must produce identical outputs for every fixture
- Every exposed function must have at least one fixture in `parity/fixtures/` before the
  domain is considered usable
- Parity fixtures are immutable once merged — changing expected output is a breaking change

See [`parity/README.md`](parity/README.md) for fixture format and the MUST rule.

---

## Adding a domain

1. Create `prelude/domains/<domain>/` with `rust/`, `typescript/`, `python/`, and `README.md`
2. Implement the domain's functions in all three languages
3. Add at least one fixture per function to `parity/fixtures/<domain>.<function>/`
4. Add a `spec/` directory with Dafny invariants for any non-obvious properties
5. Run `mise run domain-parity` — the parity check enforces fixture coverage

Domain names are lowercase, hyphenated: `causal`, `graph-metrics`, `information-theory`.

---

## What domains are not

- **Not adapters or wrappers.** This layer is pure functions only — no MCP tool definitions,
  no pipeline orchestration, no streaming. Those layers are additive and come later.
- **Not repo-specific logic.** Calculations that belong only to one derived repo go in that
  repo's `crates/` or `packages/`, not here.
- **Not a dependency of `prelude/sdks/`.** The two layers are peers. A derived repo may use
  both, or only the SDK layer, depending on whether its domain is represented here.
