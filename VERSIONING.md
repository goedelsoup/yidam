# Versioning

Yidam has three independent versioning layers. They share a monorepo but are released
on separate trains with separate semantics. Understand the layers before bumping anything.

---

## Layer 1 — Template

The template layer covers everything a derived repo inherits at bootstrap time:
directory layout, prelude documents, REGEN marker format, `mise.toml` shape, the
`samudaya/` and `sangha/` scaffolding, and the `BOOTSTRAP.md` entry prompt.

**Tags:** `v{major}.{minor}.{patch}` on the monorepo root (e.g. `v0.1.0`).

**Pinning in derived repos.** Every derived repo carries a `.yidam.toml` at its root:

```toml
[yidam]
origin   = "git@github.com:goedelsoup/yidam.git"
template = "0.1.0"   # template layer — matches a monorepo tag
bootstrap = "0.1.0"  # bootstrap protocol — matches PROTOCOL_VERSION in the harness
```

`claudesync sync` reads `.yidam.toml` and reports drift against the origin tags.
`claudesync upgrade --template 0.2.0` fetches the target release and applies forward
changes to template-owned files (prelude/, BOOTSTRAP.md, mise.toml skeleton), leaving
domain-owned content (corpus/, agents/, crates/) untouched.

**Semver meaning for this layer:**

| Bump | What changed |
|---|---|
| Patch | Typo or documentation fix in prelude |
| Minor | New prelude document, new skill, new optional REGEN section |
| Major | Directory layout change, REGEN marker format change, constitutional revision |

---

## Layer 2 — SDKs

Three independently-versioned packages living under `prelude/sdks/`:

| Package | Manifest | Registry |
|---|---|---|
| `yidam-core` | `prelude/sdks/rust/Cargo.toml` | crates.io |
| `@yidam/core` | `prelude/sdks/typescript/package.json` | npm |
| `yidam-core` | `prelude/sdks/python/pyproject.toml` | PyPI |

Each package is tagged independently: `sdk/rust/v0.1.0`, `sdk/ts/v0.1.0`,
`sdk/python/v0.1.0`.

**Parity surface version.** The six parity functions (`parse_node`, `extract_claims`,
`extract_links`, `classify_commit`, `parse_markers`, `update_regen`) are versioned
jointly in `prelude/sdks/parity/VERSION`. A change to any parity function's contract
requires:

1. Bump `prelude/sdks/parity/VERSION`
2. Update all three SDK implementations in the same PR
3. Update parity fixtures in `prelude/sdks/parity/fixtures/`
4. All three SDK packages release with a matching major bump (if breaking)

SDK packages may diverge from each other on non-parity additions. They must never
diverge on the parity surface — the parity test suite enforces this.

**Semver meaning for SDK packages:**

| Bump | What changed |
|---|---|
| Patch | Bug fix with no contract change |
| Minor | New public API not on the parity surface |
| Major | Parity surface change OR breaking type / return-value change |

---

## Layer 3 — Bootstrap protocol

The bootstrap protocol covers the harness contract: which structural checks exist,
what the genesis commit must contain, the scenario schema, the result snapshot format,
and the judge rubric thresholds.

The protocol version is a `const` in the harness crate:

```rust
// tests/harness/yidam-harness/src/lib.rs
pub const PROTOCOL_VERSION: &str = "0.1.0";
```

Every result snapshot records the protocol version it was taken under. Regression
comparisons are only valid between snapshots taken at the same protocol version;
the harness rejects cross-version diffs with an explicit error rather than silently
producing misleading output.

**Tags:** `bootstrap/v{major}.{minor}.{patch}` (e.g. `bootstrap/v0.1.0`).

**Pinning in derived repos.** The `bootstrap` field in `.yidam.toml` (see Layer 1)
records which protocol version the repo's `tests/results/` snapshots are valid for.
A `claudesync upgrade --bootstrap 0.2.0` re-runs the harness against all scenarios and
commits the new baseline snapshots.

**Semver meaning for this layer:**

| Bump | What changed |
|---|---|
| Patch | Rubric clarification; new optional scenario field |
| Minor | New structural check (S-check) added; existing passing repos still pass |
| Major | Existing S-check removed or changed; genesis commit requirements changed; snapshot format changed |

---

## Release process

1. **Decide which layers are affected** by the changeset.
2. **Update the relevant manifests** (`Cargo.toml` version, `package.json`, `pyproject.toml`,
   `PROTOCOL_VERSION` const, or `prelude/sdks/parity/VERSION`).
3. **Run `mise run ci`** — all tests must pass.
4. **For SDK changes**, run `mise run parity` — all three SDK parity suites must pass.
5. **Tag** the affected layers (`git tag -s v0.2.0`, `git tag -s sdk/rust/v0.2.0`, etc.).
6. **Push tags** to origin. CI publishes to registries on matching tag patterns.

Never bump a layer as a side effect of another layer's release. Each bump is an
intentional signal to downstream consumers.
