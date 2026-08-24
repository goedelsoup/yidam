# Versioning

Yidam has four independent versioning layers. They share a monorepo but are released
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

**Prelude errata propagate by re-vendor, not by freezing.** A typo or vocabulary fix to the
prelude is a **patch** bump (above). Derived repos adopt it by re-vendoring the prelude at the new
tag — `claudesync upgrade --template` applies the forward change to the inherited `prelude/` while
leaving domain content untouched. A derived repo is never frozen at its birth prelude: a correction
made upstream reaches it on the next template bump it adopts.

---

## Layer 2 — SDKs

Three independently-versioned packages living under `yidam/prelude/sdks/`:

| Package | Manifest | Registry |
|---|---|---|
| `yidam-core` | `yidam/prelude/sdks/rust/Cargo.toml` | crates.io |
| `@yidam/core` | `yidam/prelude/sdks/typescript/package.json` | not published |
| `yidam-core` | `yidam/prelude/sdks/python/pyproject.toml` | not published |

Only the Rust package is released, and it is tagged `sdk/rust/v{major}.{minor}.{patch}`.
The other two are versioned in their manifests and move with the parity surface; they have
no release tag, because a tag whose only meaning is "CI publishes this" would name nothing.

**npm and PyPI were considered and reversed, not deferred.** This table named all three
registries from the day it was written, and for that whole time none of the three packages
was published anywhere. The parity SDKs exist to hold three language implementations to one
spec, and the consumer of that is the parity harness in this repository. Publishing them
would buy a distribution channel for something nothing outside this repository imports, at
the cost of two more release trains, two more credential sets, and two more versions free
to drift from the Rust one.

`yidam-core` is on crates.io for a reason that has nothing to do with distribution: the
`yidam` CLI depends on it by `{ path, version }`, so crates.io must hold a matching
`yidam-core` before the CLI can publish at all.

**What would reverse it back: an external consumer asking for one.** That condition is
written down because a deferral with no condition attached is how a promise survives the
decision not to keep it — which is what the previous version of this table was.

**Parity surface version.** The parity functions are versioned jointly in
`yidam/prelude/sdks/parity/VERSION`. The authoritative list is the `functions` loop in the
`parity-check` task in `mise.toml`, which fails if any of them has no fixture — this
document deliberately does **not** restate it. It used to, and said "the nine" while the
loop walked ten: a document naming an authoritative source and then copying it is the drift
the loop exists to prevent, one file over. A change to any parity function's contract requires:

1. Bump `yidam/prelude/sdks/parity/VERSION`
2. Update all three SDK implementations in the same PR
3. Update parity fixtures in `yidam/prelude/sdks/parity/fixtures/`
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
// yidam/tests/harness/yidam-harness/src/lib.rs
pub const PROTOCOL_VERSION: &str = "0.3.0";
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

**0.2.0 → 0.3.0.** Q8 added — *edges assert only relationships the domain supports*. Found by
the harness itself: the first committed baseline passed all seven structural checks and was
judged `pass` overall, and the judge reported three unsupportable edges in its summary because
no criterion covered them. Q4 asks whether an edge is a relationship rather than a filing
gesture; nothing asked whether the relationship was true.

**0.1.0 → 0.2.0.** S1–S3 and S5–S7 restated against the instance corpus at `.yidam/corpus/`.
The checks had been written against a flat markdown corpus at the repository root, a layout
[`yidam/prelude/skills/bootstrap.md`](yidam/prelude/skills/bootstrap.md) last produced
twenty-two revisions ago; three of them passed by iterating an empty node list. The snapshot now records the protocol version it was
taken under, and `harness diff` refuses to compare across versions rather than attributing a
changed check to a changed model.

---

## Layer 4 — Tooling

The tooling layer covers the two things a *person* runs rather than a derived repo
inherits: the `yidam` CLI and the editor client.

| Artifact | Manifest | Tag | Registry |
|---|---|---|---|
| `yidam` CLI | `yidam/cli/Cargo.toml` | `cli/v{major}.{minor}.{patch}` | crates.io, GitHub releases, `goedelsoup/homebrew-tap` |
| `goedelsoup.yidam` extension | `yidam/editors/vscode/package.json` | `editor/v{major}.{minor}.{patch}` | VS Code Marketplace, Open VSX |

The CLI's four channels are one artifact reached four ways, and only the first is built:
`.github/workflows/release.yml` cross-compiles the light `reports` build for four targets and
publishes them as release assets. `install.sh`, the Homebrew formula, and `cargo binstall` all
download *those* assets — the tap's formula is rendered from their checksums by the same
workflow (`render-formula.sh`), never maintained by hand. crates.io carries the source,
which is what makes `cargo install` and `cargo binstall` both work.

A hand-edited formula is the failure this arrangement is shaped against: it is a second place
the version lives, it goes stale on the first release nobody remembers to follow, and the
staleness is invisible from here — it shows up as a stranger installing an old binary.

**Two artifacts, one layer — the same shape as Layer 2.** The SDKs are three packages held
together by one jointly-versioned contract (`yidam/prelude/sdks/parity/VERSION`), of which
one is released. This layer is the same arrangement with two artifacts and
a different contract: `format_version`.

That is what makes separate tags safe. A Marketplace release needs a public, semver-shaped
version and its own cadence; a CLI patch should not imply an extension release, and an
extension patch should not imply a CLI one. Neither version is what the two negotiate on.

### The contract between them is `format_version`

```rust
// yidam/cli/src/report.rs
pub const FORMAT_VERSION: &str = "1";
```

Every `yidam <command> --format json` carries it, alongside the CLI's own version and build
commit. A consumer versioned independently of the binary a repository pins reads it first
and **degrades loudly** on an unknown major — says so, and disables verdict features rather
than mis-parsing.

| Bump | What changed |
|---|---|
| No bump | A **new field**. Consumers must ignore what they do not know, so adding one is not a break. |
| Major | A removed field, a changed meaning, a narrowed type — anything a consumer that understood the previous version would mis-read. |

There is no minor or patch: it answers one question, and the answer is yes or no.

**Semver meaning for the CLI and the extension:**

| Bump | What changed |
|---|---|
| Patch | Bug fix; no new command, flag, or report field |
| Minor | New command, new flag, new report field, new editor affordance |
| Major | A command or flag removed; the meaning of an existing verdict changed; `format_version` bumped |

### Pinning in derived repos

Nothing new in `.yidam.toml`. The CLI is already pinned by the `commit` field that records
which yidam a corpus was vendored from — [`[yidam-build]`](mise.yidam.toml) builds from it —
and the editor client resolves a binary rather than bundling one, so a repository never has
two opinions about which yidam governs it.

`format_version` is the compatibility axis, and it is carried in the data rather than
declared in a manifest. A third pin would be a third thing to get out of step.

### The first release is `cli/v0.2.0`, not `cli/v0.1.0`

`yidam/cli/Cargo.toml` declared `0.1.0` from the commit that created it until the day the
first release workflow existed — through every command the CLI now has, the report contract,
and the feature partition. Nothing was ever tagged, so no consumer was misled; but `0.1.0`
had by then named a hundred different binaries, and reusing it for the first one anybody can
actually obtain would make the only version that ever meant something ambiguous.

So the tooling layer starts at `0.2.0`. `0.1.0` is not skipped for superstition — it is
retired because it was spent.

### What this layer is not

Not the vendored prelude — that is Layer 1, and a derived repo carries none of `yidam/cli/`
or `yidam/editors/`. Not the parity SDKs, which the CLI depends on and does not release.

---

## Release process

1. **Decide which layers are affected** by the changeset. Four now, not three — the
   tooling layer is the one an RFC header means when it writes "tooling (`yidam` CLI)".
2. **Update the relevant manifests** (`Cargo.toml` version, `package.json`, `pyproject.toml`,
   `PROTOCOL_VERSION` const, or `yidam/prelude/sdks/parity/VERSION`).
3. **Run `mise run ci`** — all tests must pass.
4. **For SDK changes**, run `mise run parity` — all three SDK parity suites must pass.
   The `ci (parity)` job runs it on every push and pull request.
5. **Tag** the affected layers with [`./release.sh`](release.sh) — `mise run release
   sdk/rust 0.2.0`, `mise run release cli 0.2.1`, and so on. It refuses a version the
   manifest does not declare, a dirty tree, a commit that is not `origin/main`, a tag whose
   workflow is not present at that commit, and — for `cli` — a `yidam-core` that is not yet
   on crates.io or a missing `HOMEBREW_TAP_TOKEN`. Add `--dry-run` to see the checks without
   tagging. The TypeScript and Python SDKs are not tagged; see Layer 2.
6. **Push tags** to origin. CI publishes to registries on matching tag patterns.
   `sdk/rust/v*` must be published before `cli/v*`: the CLI's own publish fails on a
   missing `yidam-core` until it is. `release.sh` enforces that ordering, which is the
   reason it exists — the ordering was documented in a workflow comment and then not
   followed by the person who had written it a few hours earlier.
7. **Dispatch [Install channels](.github/workflows/install-channels.yml)** once the release
   is out. It runs each documented install line in a clean container and asserts the version
   it gets. Deliberately not automatic on a release: crates.io index propagation is a lag
   nobody controls, and a check whose reds are sometimes timing is one people learn to shrug
   at.

### Two credentials the release needs

`CARGO_REGISTRY_TOKEN` publishes both crates. `HOMEBREW_TAP_TOKEN` is a fine-grained PAT
with *Contents: read and write* on `goedelsoup/homebrew-tap` — a second repository, which
this one's `GITHUB_TOKEN` cannot reach. `vars.HOMEBREW_TAP_REPO` overrides the destination
if the tap moves.

Missing either one, the release still publishes everything else and goes red at the one job.
That is on purpose — a skip would leave a channel stale with nothing anywhere red — but red
is not the same as fixed, and on `cli/v0.2.1` the tap served the previous release in
between. `release.sh` now asks about the tap token *before* the tag, which is the only point
at which the answer can still change anything.

If a tap push does fail, no re-tag is needed:
[Tap](.github/workflows/tap.yml) is dispatchable with a release tag, and renders the formula
from that release's own `SHA256SUMS`. It refuses a tag that is not the latest release, since
the tap serves one formula and an older one is a downgrade.

Never bump a layer as a side effect of another layer's release. Each bump is an
intentional signal to downstream consumers.
