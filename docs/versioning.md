# Versioning and releases

yidam has **four independent versioning layers**. They share a monorepo but release on separate
trains with separate semantics, because they have genuinely different lifetimes: what a derived
repository *inherits* and what a person *runs* do not move together, and forcing one version on
both would make every CLI patch imply that every corpus is out of date.

| Layer | What it covers | Tag |
|---|---|---|
| 1 — Template | Everything a derived repo inherits at bootstrap: directory layout, prelude documents, REGEN marker format, scaffolding, `BOOTSTRAP.md` | `v{x.y.z}` |
| 2 — SDKs | The prelude model implemented in Rust, TypeScript, Python | `sdk/rust/v{x.y.z}` |
| 3 — Bootstrap protocol | The bootstrap sequence and the harness that scores it | `bootstrap/v{x.y.z}` |
| 4 — Tooling | The `yidam` CLI and the editor client | `cli/v{x.y.z}`, `editor/v{x.y.z}` |

**One namespace, four layers.** Because all four release from one repository, they routinely tag
the same commit — and a question phrased as "what is the latest release?" answers for whichever
layer happened to tag last. That has broken real things: at one commit carrying `cli/v0.4.0`,
`sdk/rust/v0.3.0` and `editor/v0.1.0` at once, a bare `git describe` answered `editor/v0.1.0`,
so every repository cloned there recorded a VS Code extension's version as the version of its
template layer. Any code asking "which tag?" must name the layer it means.

**There is a second versioning document, and the split is deliberate.** This page is what a
reader consults: what the four layers are, how a derived repository pins and adopts one, and
how a release is cut. [`VERSIONING.md`](https://github.com/goedelsoup/yidam/blob/main/VERSIONING.md)
at the repository root is the maintainer's reference — the semver table for every layer, the
release history behind each protocol bump, and the decision records for what is *not* a layer.
It stays at the root rather than moving here for two reasons that are not style: it is part of
the template root `clone` withholds and bootstrap deletes, because a derived repository does not
release yidam's layers; and `versioning_layers.rs` and `release_script.rs` parse it at that path
to check that what it promises is what the manifests, constants and workflows actually do. Every
path and every tag prefix either document names is held against the tree, in both directions —
these two may go out of step on prose, never on a fact.

---

## Layer 1 — Template

Tagged `v{major}.{minor}.{patch}` on the monorepo root — bare, with no prefix, which is what
distinguishes this layer from the three that prefix theirs.

| Bump | What changed |
|---|---|
| Patch | Typo or documentation fix in the prelude |
| Minor | New prelude document, new skill, new optional REGEN section |
| Major | Directory layout change, REGEN marker format change, constitutional revision |

### How a derived repo pins it

Every derived repository carries a `.yidam.toml` at its root, written by `clone` or `overlay`:

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "88edd17f4c2a1b09e3d5f7a8c6b4e2d1a9f0c3b5"
template  = "v0.1.0"
committed = "2026-08-27"
```

`commit` is the field that makes the pin resolvable — it is what the re-vendor procedure and CI
check out. `template` is the template-layer tag at that commit, or `"untagged"`. `committed` is
that commit's author date, which is how `yidam doctor` reports the prelude's age.

There is one optional table beside it, and it is the repository's rather than yidam's:

```toml
[build]
features = ["export-sqlite"]
```

**Features this repository's binary must have beyond the released default set.** One
declaration, read by all three paths that can produce a binary: `yidam-build-source` passes it
to `cargo install --features`, `yidam-build` checks the release it just downloaded and compiles
instead if it is short, and `yidam-vendor-update` withholds the `[tools]` entry so `mise
install` cannot make the swap either.

It exists because the download channel used to make that swap silently. A repository
compiling with `--features export-graph` whose pin reached a `cli/v*` tag would have had that
binary replaced by the released one, and `export --format rdf` would start answering `needs
--features export-graph` — which is a *decline*, not a failure, so a test written to skip when
the capability is absent goes on passing while asserting nothing. It was caught by a derived
repository reading the change before adopting it, not by anything here (#532).

`yidam-vendor-update` preserves this table across the re-vendor that rewrites everything else
in the file.

[Configuration](configuration.md#yidamtoml) has the field-by-field detail.

### How a derived repo adopts a newer one

```sh
mise run yidam-vendor-update                     # re-vendor at the pinned origin's HEAD
YIDAM_REF=v0.2.0 mise run yidam-vendor-update    # target a tag or branch
mise run yidam-vendor-status                     # has the origin moved?
```

This re-vendors `.yidam/.vendor/prelude/` and rewrites `.yidam.toml`. Domain content is
untouched.

**Prelude errata propagate by re-vendor, not by freezing.** A derived repository is never frozen
at its birth prelude: a typo or vocabulary fix made upstream is a patch bump, and it reaches the
repository on the next template bump it adopts.

## Layer 2 — SDKs

Three packages under `yidam/prelude/sdks/`, held together by one jointly-versioned parity
contract (`yidam/prelude/sdks/parity/VERSION`).

| Package | Manifest | Registry |
|---|---|---|
| `yidam-core` (Rust) | `yidam/prelude/sdks/rust/Cargo.toml` | crates.io |
| `@yidam/core` | `yidam/prelude/sdks/typescript/package.json` | not published |
| `yidam-core` (Python) | `yidam/prelude/sdks/python/pyproject.toml` | not published |

Only the Rust package is released, tagged `sdk/rust/v{x.y.z}`. The other two are versioned in
their manifests and move with the parity surface; they carry no release tag, because a tag whose
only meaning is "CI publishes this" would name nothing.

**npm and PyPI were considered and reversed, not deferred.** This table names registries the
project *delivers to*, never ones it intends to — a registry named in a versioning document is
read as a promise, and `cargo binstall yidam` once stood in the README for a whole release cycle
while `yidam` did not exist on crates.io.

## Layer 3 — Bootstrap protocol

Tagged `bootstrap/v{x.y.z}`. It covers the bootstrap sequence itself and the harness that scores
a run against the [rubric](quality-rubric.md) — a change here changes what a *good* bootstrap
means, which is why it is not folded into the template layer.

## Layer 4 — Tooling

The three things a *person* runs rather than a derived repo inherits.

| Artifact | Manifest | Tag | Delivered to |
|---|---|---|---|
| `yidam` CLI | `yidam/cli/Cargo.toml` | `cli/v{x.y.z}` | crates.io, GitHub releases, `goedelsoup/homebrew-tap` |
| `goedelsoup.yidam-vscode` | `yidam/editors/vscode/package.json` | `editor/v{x.y.z}` | Open VSX, GitHub releases |
| `@goedelsoup/yidam-edit` | `yidam/editors/web/package.json` | `edit/v{x.y.z}` | npm |

**The VS Code Marketplace is not in that table, and its absence is the point.** The publisher
needs an Azure DevOps organisation that has not been created, so the Marketplace step notices the
missing credential and skips rather than failing a release for a channel nothing claims. Open VSX
does not serve VS Code proper — it serves VSCodium, Cursor, Windsurf, Gitpod and code-server —
so a VS Code user installs the `.vsix` from the GitHub release, which is why that asset is
attached before either registry is tried. Restoring the Marketplace means restoring three things
together, and a test refuses any subset: the table row, the publish path, and the channel check.

**The CLI's five channels are one artifact reached five ways**, and only the first is built. The
release workflow cross-compiles the light `reports` build for four targets and publishes them as
release assets; `install.sh`, the Homebrew formula, mise's `github:` backend and `cargo binstall`
all download *those* assets. The tap's formula is rendered from their checksums by the same
workflow, never maintained by hand — a hand-edited formula is a second place the version lives,
it goes stale on the first release nobody remembers to follow, and the staleness is invisible
from here. It shows up as a stranger installing an old binary.

mise is where one release list holding four layers becomes a caller's problem. Its declaration
must carry `version_prefix = "cli/v"`, or `latest` resolves whichever layer was tagged most
recently — and an `editor/v*` release ships only a `.vsix`, so the install does not degrade, it
fails. See [installation](installation.md#mise).

**`@goedelsoup/yidam-edit` publishes no GitHub release at all**, because npm is its only channel
and the package *is* the artifact. That makes it the one layer whose version cannot be read off
the release list — the channel check resolves it from the `edit/v*` tag instead. `edit/v*` and
`editor/v*` are four characters apart and disjoint as patterns, so neither tag starts the
other's workflow; a test holds that, because a workflow firing on the wrong tag publishes
nothing and looks exactly like one that has not finished.

### The contract between the CLI and the editor clients is `format_version`

Not any one of their versions. Every `yidam <command> --format json` carries `format_version`
alongside the CLI's own version and build commit, and a consumer versioned independently of the
binary a repository pins reads that first. It is what makes separate tags safe: a CLI patch
should not imply an editor release, and an editor patch should not imply a CLI one.

It has three consumers now — the CLI declares it, and both editor clients name the major they
will parse. A client that meets a major it does not know says so and disables verdict features
rather than guessing at an envelope it cannot read.

## Release process

1. **Decide which layers the changeset affects.**
2. **Update the relevant manifests** — `Cargo.toml`, `package.json`, `pyproject.toml`, the
   protocol-version constant, or `yidam/prelude/sdks/parity/VERSION`.
3. **`mise run ci`** — everything must pass.
4. **For SDK changes, `mise run parity`** — all three suites.
5. **Tag with `./release.sh`**, e.g. `mise run release cli 0.5.1`. It refuses a version the
   manifest does not declare, a dirty tree, a commit that is not `origin/main`, a tag whose
   workflow is not present at that commit, and — for `cli` — a `yidam-core` not yet on crates.io
   or a missing tap token. `--dry-run` shows the checks without tagging.
6. **Push tags.** `sdk/rust/v*` must be published before `cli/v*`, because the CLI's own publish
   fails on a missing `yidam-core`. `release.sh` enforces that ordering, which is the reason it
   exists: the ordering was documented in a workflow comment and then not followed by the person
   who had written it hours earlier.
7. **Dispatch [Install channels](https://github.com/goedelsoup/yidam/actions/workflows/install-channels.yml)**
   once the release is out. Deliberately manual: crates.io index propagation is a lag nobody
   controls, and a check whose reds are sometimes timing is one people learn to shrug at.

### Credentials

`CARGO_REGISTRY_TOKEN` publishes both crates. `HOMEBREW_TAP_TOKEN` is a fine-grained PAT with
*Contents: read and write* on `goedelsoup/homebrew-tap` — a second repository, which this one's
`GITHUB_TOKEN` cannot reach.

Missing either, the release publishes everything else and goes red at the one job. That is on
purpose — a skip would leave a channel stale with nothing anywhere red — but red is not the same
as fixed, and on one release the tap served the previous version in between. `release.sh` now
asks about the tap token *before* the tag, which is the only point at which the answer can still
change anything.

---

The semver table for each layer, the release history behind every protocol bump, and the
arguments for what is deliberately *not* a layer are in
[VERSIONING.md](https://github.com/goedelsoup/yidam/blob/main/VERSIONING.md) at the repository
root — see the note under the layer table above for why it lives there rather than here.
