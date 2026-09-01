# Versioning

Yidam has four independent versioning layers. They share a monorepo but are released
on separate trains with separate semantics. Understand the layers before bumping anything.

---

## Layer 1 — Template

The template layer covers everything a derived repo inherits at bootstrap time:
directory layout, prelude documents, REGEN marker format, `mise.toml` shape, the
`samudaya/` and `sangha/` scaffolding, and the `BOOTSTRAP.md` entry prompt.

**Tags:** `v{major}.{minor}.{patch}` on the monorepo root (e.g. `v0.1.0`).

**Pinning in derived repos.** Every derived repo carries a `.yidam.toml` at its root, written
by `clone`/`overlay` and rewritten by `mise run yidam-vendor-update`. These four fields, and
no others — `src/provenance.rs::render` is what emits them:

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "88edd17f4c2a1b09e3d5f7a8c6b4e2d1a9f0c3b5"
template  = "v0.1.0"    # this layer's tag at that commit, or "untagged"
committed = "2026-08-27"
```

`commit` is the resolvable pin — it is what the re-vendor procedure and CI check out.
`template` carries the tag verbatim, `v` prefix included, and is matched against
`v[0-9]*` so that a commit carrying several layers' tags cannot answer for this one.
`committed` is the *pinned commit's* author date, not the date the repo last re-vendored.

**There is no `bootstrap` field.** This document described one for several releases; nothing
wrote it and nothing read it. The protocol version a repo's snapshots are valid for is
recorded by the harness, not by this file — see Layer 3.

`mise run yidam-vendor-status` reads `.yidam.toml` and reports drift against the origin.
`mise run yidam-vendor-update` re-vendors `.yidam/.vendor/prelude/` at the origin's current
commit and re-pins the file, leaving domain-owned content (`corpus/`, `agents/`, `crates/`)
untouched; `YIDAM_REF=v0.2.0` targets a specific tag or branch.

**Semver meaning for this layer:**

| Bump | What changed |
|---|---|
| Patch | Typo or documentation fix in prelude |
| Minor | New prelude document, new skill, new optional REGEN section |
| Major | Directory layout change, REGEN marker format change, constitutional revision |

**Prelude errata propagate by re-vendor, not by freezing.** A typo or vocabulary fix to the
prelude is a **patch** bump (above). Derived repos adopt it by re-vendoring the prelude at the new
tag — `YIDAM_REF=v0.2.0 mise run yidam-vendor-update` applies the forward change to the inherited
prelude while leaving domain content untouched. A derived repo is never frozen at its birth prelude:
a correction made upstream reaches it on the next template bump it adopts.

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
pub const PROTOCOL_VERSION: &str = "0.4.0";
```

Every result snapshot records the protocol version it was taken under. Regression
comparisons are only valid between snapshots taken at the same protocol version;
the harness rejects cross-version diffs with an explicit error rather than silently
producing misleading output.

**Tags:** `bootstrap/v{major}.{minor}.{patch}` (e.g. `bootstrap/v0.1.0`).

**How a repo records it.** Not in `.yidam.toml` — that file has no `bootstrap` field, and
this document described one for several releases that nothing wrote and nothing read. The
protocol version travels with the artifact it qualifies: **every result snapshot records the
`PROTOCOL_VERSION` it was taken under**, which is what lets the harness refuse a cross-version
diff. Re-baselining after a bump is a harness run whose new snapshots carry the new version.

**Semver meaning for this layer:**

| Bump | What changed |
|---|---|
| Patch | Rubric clarification; new optional scenario field |
| Minor | New structural check (S-check) added; existing passing repos still pass |
| Major | Existing S-check removed or changed; genesis commit requirements changed; snapshot format changed |

**0.3.0 → 0.4.0.** S4 changed meaning: it accepts the verbs step 8's commit-sequence block
actually names — `establish:` and `implement:` from step 7, `regen:` from step 8.5 — where it
had accepted only `consume:` and `vendor:` after the root. A major by the table above: an
existing S-check changed, and it changed so that a history which used to fail now passes.
A run that followed the bootstrap skill exactly could not pass S4, and had not been able to
for as long as step 7 has prescribed those commits. It went unseen because the only baseline
the harness holds produced none of the three — its scenario approved no implied edges, folded
its stubs into the genesis commit, and predates step 8.5. Reported by a derived repository,
which hit it on its first bootstrap. The verb list is now pinned to the skill's own block in
both directions rather than maintained by remembering.

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
| `goedelsoup.yidam-vscode` extension | `yidam/editors/vscode/package.json` | `editor/v{major}.{minor}.{patch}` | Open VSX, GitHub releases |

**The VS Code Marketplace is not in that row, and its absence is the point.** The `goedelsoup`
publisher needs an Azure DevOps organisation that has not been created (#314), so `VSCE_PAT`
does not exist and `editor.yml`'s Marketplace step notices and skips rather than failing a
release for a channel nothing claims.

This table names registries this project *delivers to*, never ones it intends to. That rule
was set when Layer 2 stopped naming npm and PyPI, and it holds here for the same reason: a
registry named in a versioning document is read as a promise, and #232 is what a promise
nobody can keep costs — `cargo binstall yidam` stood in the README for a release cycle while
`yidam` did not exist on crates.io.

Restoring the Marketplace means restoring three things together, and a test refuses any
subset: the row above, the publish path in `editor.yml`, and the channel check in
`install-channels.yml`. **Open VSX does not serve VS Code proper** — it serves VSCodium,
Cursor, Windsurf, Gitpod and code-server — so until then a VS Code user installs the `.vsix`
from the GitHub release, which is why that asset is attached before either registry is tried.

The CLI's five channels are one artifact reached five ways, and only the first is built:
`.github/workflows/release.yml` cross-compiles the light `reports` build for four targets and
publishes them as release assets. `install.sh`, the Homebrew formula, mise's `github:` backend
and `cargo binstall` all download *those* assets — the tap's formula is rendered from their
checksums by the same workflow (`render-formula.sh`), never maintained by hand. crates.io
carries the source, which is what makes `cargo install` and `cargo binstall` both work.

**mise is where one release list holding four layers becomes a caller's problem.** Its
declaration must carry `version_prefix = "cli/v"`, or `latest` resolves whichever layer was
tagged most recently — and when that is `editor/v*` the install does not degrade, it fails,
because that release ships only a `.vsix`. That is the same defect `releases/latest` caused in
`install.sh` and `tap.yml`, arriving through a third party's resolver instead of ours.

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

**Raising the minimum Rust version is a minor bump, at least.**

`rust-version` is a promise to anyone building from source, and `yidam-core` is on crates.io
where the promise is load-bearing: a consumer on an older toolchain stops being able to
resolve the crate at all. Cargo enforces it, so raising it does not break a compile — it
makes the version invisible to that consumer, which is the same outcome arriving quietly.

Both crates moved 1.85 → 1.88.0 in #463. They read 1.85 while every gate built 1.88.0 and
nothing compiled the floor, so it was an unverified claim rather than a lower one; the pin is
now the floor and every build verifies it. The next raise is a decision with a cost, and this
is where the cost is written down.

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
