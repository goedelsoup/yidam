# RFC-0003 — Feature-gated builds and a publishable reports-only binary

- **Status:** Accepted
- **Track:** I3
- **Relates to:** RFC-0001 (the report contract), RFC-0004 (drift detection), RFC-0007 (the Python index layer)
- **Versioning layers touched:** template (the CLI ships in the template tree; its build + install story is template-owned)
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

The four reports that keep a corpus honest — `graph-check`, `lint`, `corpus-index`,
`open-questions` — are pure Rust: they walk a git tree, parse YAML, and emit markdown.
But they live in the same crate as `fastembed`, `lancedb`, `arrow`, a from-source SQLite,
and a build that requires `protoc`. There is no feature gate and the binary is on no
registry, so the only way to obtain a text report is to build the entire native ML stack.
That cost is exactly why BOSC did not build the binary at all and re-implemented the reports
in Python instead. This RFC partitions the crate into cargo features so `cargo build
--no-default-features --features reports` yields a tiny, `protoc`-free, SQLite-free binary,
and proposes publishing that binary so a consumer's CI can run the *real* reports without a
Rust build. It removes the reason a downstream repo re-implements at all.

## Problem

**The reports carry the weight of the whole native stack.** `yidam/cli/Cargo.toml`
declares one undivided dependency set: `fastembed = "4"` (`Cargo.toml:29`),
`lancedb = "0.13"` (`:30`), `arrow-array/-schema/-ipc = "52"` (`:32-34`), `tokio` (`:35`),
`reqwest` (`:37`), `rusqlite = { version = "0.32", features = ["bundled"] }` (`:46`) — the
`bundled` feature **compiles SQLite from C source** — `sqlite-vec = "0.1.9"` (`:47`), and
`oxrdf`/`oxttl` (`:44-45`). There is no `[features]` table. Every one of these is pulled for
every `cargo build`, including a build a consumer wants only for `graph-check`.

**The build needs `protoc` and a newer toolchain than the crate admits.** `lancedb` pulls
`lance-encoding`, whose `prost-build` build script needs the Protocol Buffers compiler:
`mise.toml:17-18` pins `protoc = "31"` and names the reason ("required by lance-encoding's
build script (prost-build), a transitive dependency of the yidam CLI via lancedb"). The crate
declares `rust-version = "1.85"` (`Cargo.toml:8`) but the toolchain is pinned at `1.88.0`
(`mise.toml:15`). So a consumer reproducing a report build must install `protoc 31`, a C
compiler for SQLite, and Rust 1.88 — none of which a text report touches.

**What the reports actually need is a small subset.** `cmd/corpus.rs`
(`graph_check`, `corpus_index`, `open_questions`) and `cmd/lint.rs` import nothing heavier
than `crate::`, `serde`, `walkdir`, and `pulldown-cmark` — no `use` of `fastembed`,
`lancedb`, `arrow`, `rusqlite`, or `oxrdf` appears in either file, nor in `cmd/status.rs`,
`cmd/phases.rs`, `cmd/diff.rs`, or `cmd/export_graphml.rs`. `load_domain_model`
(`model.rs`) pre-renders every report over a `DomainModel` with no heavy import at all. The
native stack is confined to four modules: `cmd/index_build.rs` (`fastembed`, `lancedb`,
`arrow`, `Cargo.toml:2-10`), `cmd/serve/mod.rs` (`fastembed`, `:15`), `cmd/export_sqlite.rs`
(`rusqlite`, `:2`), and `cmd/export_rdf.rs` (`oxrdf`, `:2`). The dependency line runs cleanly
between the reports and the index — but the crate does not cut there.

**The binary is unpublished.** `VERSIONING.md:44-48` lists three packages on registries —
`yidam-core` on crates.io, `@yidam/core` on npm, `yidam-core` on PyPI — and the `yidam`
binary on none of them. The documented install is `cargo install --path yidam/cli`
(`mise.toml:33-35`). BOSC's own `.yidam.toml` records the consequence in its comment: *"Not
published to crates.io; built from source with `cargo install --path yidam/cli`"* and
*"the REAL `yidam` binary … for a cross-check, when it is installed."* It never is: BOSC
replicated `graph-check`/`corpus-index`/`open-questions`/`lint` in Python
(`watermark.site.corpus_mirror`) precisely so the reports "regenerate offline, with no Rust
toolchain." A gate + a registry entry would have made re-implementation unnecessary.

## Proposal

### A cargo feature partition

Introduce a `[features]` table in `yidam/cli/Cargo.toml` and move the heavy dependencies to
`optional = true`, activated by feature:

| Feature | Default | Commands it enables | Dependencies it adds |
|---|---|---|---|
| `reports` | **yes** | `graph-check`, `lint`, `corpus-index`, `open-questions`, `status`, `index-status`, `diff`, `phases`, `backfill`, `decisions-log`, `catalog-audit`, `{agents,skills,crates,packages}-index`, `bundle-status`, `samudaya-audit`, `clone`, `overlay`, `embed`, `export --format {bundle,web,graphml,llms}` | none beyond the light base: `walkdir`, `pulldown-cmark`, `serde`/`serde_yaml`/`serde_json`, `flate2`+`tar`, `toml`, `sha2`, `yidam-core` |
| `vector-read` | no | semantic `retrieve` and an anchored `query`, over an index built elsewhere | `fastembed 4`, `arrow-* 52` → ONNX runtime, **no `protoc`** |
| `index` | no | `vector-read`, plus `index-build` | `+ lancedb 0.13`, `tokio`, `futures` → **requires `protoc 31` at build** |
| `export-sqlite` | no | `export --format sqlite` | `rusqlite 0.32 {bundled}` (**from-source SQLite**), `sqlite-vec` |
| `export-graph` | **yes** (since #532) | `export --format rdf` | `oxrdf`, `oxttl` (pure Rust; no `protoc`, no C) |

Three placements are load-bearing and worth stating explicitly:

- **`embed` is a report, not an index step.** `yidam embed` (`main.rs:170`) extracts embedding
  *text* to `.yidam/embeddings/`; it imports nothing heavy. Only `index-build` vectorizes
  (via `fastembed`). So `embed` stays in `reports` and only the LanceDB build crosses into
  `index`.
- **`graphml` stays in `reports`.** `cmd/export_graphml.rs` hand-writes XML and has no `oxrdf`
  import; only the `rdf` format needs `oxrdf`/`oxttl`. `export-graph` therefore gates *only*
  the `rdf` arm — GraphML export is dep-free and ships by default.
- **`index-status` and `status` stay in `reports`.** They only `read_to_string` the index's
  `meta.json` (`cmd/status.rs:58-59`); reporting on an index needs no embedding runtime.
- **Reading an index and building one are different features** (added by #442, after RFC-0023
  gave an index a way to travel between machines). `lancedb` is named in exactly one file,
  `cmd/index_build.rs`, and it is what requires protoc; decoding `corpus.arrow` and embedding a
  query need only `fastembed` and `arrow-*`. Measured on this repository's lockfile: 197
  packages for the default build, 387 with `vector-read`, 715 with `index`.

  The split matters because the *released* binary is the light default, so the machine that can
  use an index is almost never the machine that can build one. `resolve_model` moved out of the
  gated `cmd/index_build.rs` into `src/embedding.rs` to make it possible — the same move
  `sha256_hex` made out of `cmd/tonpa/`, and for the same reason: **the feature buys the build,
  and naming a model is not building one.**

  **`vector-read` is not a released artifact, and that is a decision.** It carries an ONNX
  runtime — a native dependency this release matrix has never cross-compiled for aarch64 — and
  publishing it would double the artifacts per platform, which every install channel then has
  to choose between on a `releases/latest` that already answers for four versioning layers. The
  channel is `cargo install --features vector-read`: a Rust toolchain and nothing else, which
  is a long way from the protoc-and-CMake it replaces. What would reverse it is somebody
  wanting semantic retrieval on a machine with no toolchain.

`cargo build --no-default-features --features reports` then produces a binary with no
`protoc` requirement, no C compiler for SQLite, and no `fastembed`/`lancedb`/`arrow` in the
graph — buildable with a stock stable Rust toolchain.

### Command dispatch and the async seam

Two mechanical issues follow from gating and both have clean answers.

**Keep the subcommands visible; gate only the body.** If a `Command` variant in `main.rs` is
`#[cfg(feature = "index")]`-removed, `yidam index-build` degrades to clap's "unrecognized
subcommand" and the command *vanishes* from `--help` — a confusing failure. Instead the clap
enum stays complete in all builds and only the **dispatch arm** is gated, falling through to a
precise error:

```rust
Command::IndexBuild { model } => {
    #[cfg(feature = "index")]
    { yidam::index_build(model).await }
    #[cfg(not(feature = "index"))]
    { let _ = model; anyhow::bail!(
        "`index-build` needs the `index` feature — reinstall with \
         `cargo install yidam --features index` (pulls fastembed/lancedb; requires protoc)") }
}
```

So absent capabilities announce themselves and name the fix, instead of disappearing.
`export --list` (`cmd/export.rs:59-72`) is amended to mark each format `compiled` /
`needs --features export-sqlite` so `--list` tells the truth about *this* build.

**Confine `tokio` to `index`.** `main` is `#[tokio::main]` today (`main.rs:132-133`), but the
only async commands are `index-build` (`cmd/index_build.rs:44`) and the `tonpa` subtree
(`cmd/tonpa/mod.rs:44`, network fetch via `reqwest`, `cmd/tonpa/install.rs:13`). Under
`reports` there is no runtime and no async work. Make `main` a plain `fn main() -> Result<()>`
and have the `index`-gated arms build a `tokio::runtime::Runtime` locally and `block_on` the
future. `tokio`, `futures`, and `reqwest` then live entirely inside the optional features.

### Publishing the reports binary

Ship the reports-capable binary two ways, so no consumer needs a Rust build:

1. **crates.io** — publish `yidam` so `cargo install yidam` installs the default (`reports`)
   tool. This requires publishing the `yidam-core` path dependency first: `Cargo.toml:43`
   pins it as `{ path = "../prelude/sdks/rust" }`, and a crates.io release cannot carry a
   `path` dep — it must become a `version` dep against the published `yidam-core`
   (already slated for crates.io in `VERSIONING.md:46`). Add a `full` convenience feature
   (`full = ["reports", "index", "export-sqlite", "export-graph"]`) so
   `cargo install yidam --features full` reproduces today's monolith.
2. **Prebuilt release artifacts** — attach `reports` (and `full`) binaries for the common
   targets to GitHub Releases and index them for `cargo-binstall`, so a consumer's CI runs
   `cargo binstall yidam` (or downloads a tarball) and needs **no Rust toolchain, no `protoc`,
   no C compiler** at all. Register the binary as a fourth row in the `VERSIONING.md:44-48`
   table with its own tag train (proposed `cli/v{x.y.z}`, distinct from `sdk/rust/*`).

### What this unlocks for RFC-0001 and RFC-0004

A light, installable binary converts two currently-unenforced parity claims into executed
checks, for free:

- **RFC-0001 (executed parity).** BOSC's Python replicas assert faithfulness only in
  docstrings that cite Rust symbols; no test runs the binary. With `cargo binstall yidam` in
  CI, BOSC can run the *real* `yidam graph-check` / `lint` / `open-questions` over its
  committed `.yidam/corpus/` mirror as a drift guard, and diff the output against its Python
  reports. Docstring-parity becomes executed-parity — and the live `open` divergence noted
  in RFC-0001/0006 (BOSC keying `open` off a field the real `has_open_claim` never sees)
  surfaces the day it lands instead of silently.
- **RFC-0004 (an enforceable pin).** `.yidam.toml` already pins `cli = "0.1.0"` and
  `cli_ref = 8f7ada99…` "so drift is visible," but nothing installs that ref. A published,
  `protoc`-free binary makes `cargo binstall yidam@<cli>` a real CI step, so `yidam sync` can
  verify the pin against a binary it can actually obtain rather than one no consumer builds.

## Migration & compatibility

- **`default = ["reports"]` is a deliberate, small break.** `cargo install --path yidam/cli`
  today gives the full tool; after this change the default is the light reports binary and
  `index-build` / `serve --mcp` / sqlite/rdf export require `--features index` etc. This is
  the point — the common case should be cheap — but it must be called out in the release
  notes and the mismatched `rust-version`/`1.88` pin reconciled while the manifest is open.
- **yidam's own dev loop is preserved via `full`.** Update `tasks.yidam-build`
  (`mise.toml:33-35`) to `cargo install --path yidam/cli --features full`. All REGEN tasks
  (`status`, `corpus-index`, … `mise.toml:45-96`) are in `reports` and back a light install
  unchanged; `embed-parity` (`mise.toml:190-197`) tests `index` and must pass
  `--features index`.
- **CI must test more than the default.** `mise.toml:251` runs `cargo test --manifest-path
  yidam/cli/Cargo.toml`, which after this change compiles only `reports` and silently drops
  every `index`/`sqlite`/`rdf` test. CI must run `--all-features` (or a
  `{reports, full}` matrix) so the gated code stays exercised.
- **`serve --mcp` staying behind `index` is acceptable.** Semantic `retrieve`
  (`cmd/serve/mod.rs:15`) needs `fastembed`; a reports-only consumer that wants
  `graph-check`/`lint` does not need an MCP server. RFC-0006's "degrade to keyword, don't
  re-embed" doctrine leaves room to later offer a keyword-only `serve` inside `reports` with
  the semantic path behind `index`, but that is out of scope here.

## Alternatives considered

- **A separate `yidam-reports` crate/binary.** Add a second `[[bin]]` over the same `lib`,
  compiled with only the light deps. Rejected as the primary mechanism: it means two install
  stories, two names to publish, and — the real cost — two report entry points that can drift
  from each other *inside yidam*, the very failure this RFC set exists to prevent. Feature
  flags keep one binary and one source of truth.
- **A WASM build of the reports.** Compelling for zero-install use, but the reports *walk a
  git checkout* (`walk_corpus_instances`, filesystem-relative broken-link resolution); a WASM
  sandbox has no repo to walk. It solves a different problem (in-browser preview) and does not
  give CI a way to run the real reports over a working tree.
- **Stay monolithic, ship prebuilt binaries only.** Skips the feature work and just attaches
  a built binary to Releases. It helps, but the artifact still bundles SQLite/lancedb/fastembed
  (large, slow release builds, surprise model-weight downloads at runtime), and
  `cargo install yidam` still drags in `protoc`. Feature flags *and* prebuilt binaries is
  strictly better than either alone.

## Open questions

- **Internal drift between reports and index.** Splitting yidam risks the reports path and the
  index path diverging *inside* yidam if only the default is exercised — e.g. `index-status`'s
  reader (`cmd/status.rs:58-59`) assuming a `meta.json` shape that a later `index-build`
  change breaks, with no CI catching it. The `--all-features` CI matrix above is the intended
  guard; the shared `DomainModel` seam (`model.rs`) is the invariant to protect.
- **Arrow/lancedb version-lock rot.** `Cargo.toml:31` warns "Arrow versions must match
  lancedb's transitive dependency; update together." If `index` is off by default and rarely
  built, that lock decays unnoticed until someone opts in. Who owns the periodic bump, and
  should the `full` build run on every CI pass to keep it honest?
- **crates.io naming and ordering.** Is the name `yidam` available; does the release require
  the `yidam-core` path→version conversion to land first (its own `sdk/rust/*` train); and
  does publishing the binary want a distinct `cli/v*` tag or a shared monorepo tag?
- **`protoc` on the release machine.** Even prebuilt `full` binaries need `protoc 31` to
  *build* in the release workflow. That is acceptable on the maintainer's CI — the point is
  the *consumer* never needs it — but the release pipeline must provision it explicitly.
- **`tonpa` placement.** Bundle-dependency management (`reqwest`/`tokio`, network) is neither
  a report nor an index build. Fold it into `index`, give it its own `tonpa` feature, or leave
  it in a default that then re-acquires `reqwest`? It is the one command that does not fit the
  three-way cut cleanly.
