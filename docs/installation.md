# Installing yidam

The `yidam` binary is the whole toolchain. There is no daemon, no service, and nothing to
configure before the first run — a derived repository records which yidam governs it in its
own `.yidam.toml`, and the binary reads that.

Four channels serve the same artifact. Each one is exercised weekly by
[`install-channels.yml`](https://github.com/goedelsoup/yidam/blob/main/.github/workflows/install-channels.yml),
which runs the lines below verbatim in a container holding only the tools the line claims to
need and asserts that `yidam --version` answers with the latest release. A channel that stops
working turns that job red, so a line documented here is a line somebody checked this week.

## The script

```sh
curl -fsSL https://raw.githubusercontent.com/goedelsoup/yidam/main/install.sh | sh
```

Resolves the newest `cli/v*` release for your platform, downloads the tarball **and its
`.sha256`**, verifies it, and installs to `~/.local/bin`. If neither `shasum` nor `sha256sum`
is present it refuses the install rather than trusting an unverified download — that is a
deliberate failure, not a fallback. It warns if the install directory is not on your `PATH`.

| Variable | Effect |
|---|---|
| `YIDAM_BIN_DIR` | Install somewhere other than `~/.local/bin` |
| `YIDAM_VERSION` | Install a specific tag instead of the latest — e.g. `cli/v0.4.0` |
| `YIDAM_REPO` | Resolve releases from a fork instead of `goedelsoup/yidam` |

## Homebrew

```sh
brew install goedelsoup/tap/yidam
```

macOS and Linux. The formula is rendered from the release's own checksums by the release
workflow, so the tap cannot lag a published version, and `brew upgrade` carries yidam along
with everything else.

## cargo-binstall

```sh
cargo binstall yidam
```

Fetches the same prebuilt artifact the script does — it does not compile. Useful when cargo is
already on hand and you would rather not add another package manager.

## From source

```sh
cargo install --git https://github.com/goedelsoup/yidam --tag cli/v0.6.0 --locked yidam
```

The default build needs **only a Rust toolchain**: no protoc, no system C library, no ML
runtime. `--locked` builds from the lock file the tag committed, which is what makes this the
same binary the release was built from; without it cargo re-resolves every dependency and you
get something that merely resembles it.

`yidam` is also on [crates.io](https://crates.io/crates/yidam), so `cargo install yidam` works
— but it re-resolves dependencies the same way, and pinning the tag is the reproducible form.

**MSRV is Rust 1.85**, deliberately below the 1.88 toolchain this repository pins, so a derived
repo on an older toolchain can still install the CLI.

## Verifying

```console
$ yidam --version
yidam 0.5.0 (78544f8) [reports index export-sqlite export-graph tonpa]
```

Three facts, and the third is the one that matters: the version, the commit it was built from,
and **the features compiled into it**. A command that is absent from a build is absent because
of that list, so it is printed on every `--version` rather than left to be discovered when a
subcommand reports `unrecognized subcommand`.

Inside a derived repository, [`yidam doctor`](troubleshooting.md) is the fuller check — it
answers whether the running binary is the one the repository pins, whether `.yidam/bin` is
ahead on `PATH`, and how stale the vendored prelude is.

## Which build you have

The binary is partitioned by cargo feature so the common case stays cheap to install. Released
artifacts — the script, the tap, binstall — carry the **default** set.

| Feature | Adds | Build cost |
|---|---|---|
| `reports` *(default)* | Every pure-Rust command: reports, gates, queries, `clone`, `overlay`, `serve --mcp`, `serve --lsp`, export to graphml/llms | None |
| `tonpa` *(default)* | Bundle dependency manager — `tonpa add`, `verify`, `update` | reqwest (rustls) + tokio; vendored C, no system library |
| `index` | `index-build`, and upgrades `serve --mcp`'s `retrieve` from keyword to semantic | fastembed (ONNX) + LanceDB; **needs protoc 31 at build time** |
| `export-sqlite` | `export --format sqlite` | Bundled SQLite + sqlite-vec, compiled from C |
| `export-graph` | `export --format rdf` | Pure Rust |
| `full` | All of the above | |

Two things follow from that table that are easy to get backwards.

**Deriving a repository does not need `full`.** `clone`, `overlay`, `tonpa`, both `serve`
transports, every report and every gate are in the light set. A default binary serves MCP; what
it does not serve is *semantic* retrieval, and it says so on every call rather than silently
returning keyword results as though they were embeddings.

**`tonpa` is a default even though it costs an HTTP stack**, because it is the only feature
whose absence broke an instruction rather than removing a capability. Without it
`yidam tonpa add …` answered `unrecognized subcommand`, and inside a script with output
redirected that is indistinguishable from success.

To build a heavier set from source:

```sh
cargo install --git https://github.com/goedelsoup/yidam --tag cli/v0.6.0 --locked \
  --features full yidam
```

`--features full` requires protoc 31 and a C toolchain on the build machine. That is the
maintainer's build — see [Contributing](contributing.md), where `mise install` provisions all
of it.

## Upgrading

Whichever channel installed it:

```sh
brew upgrade yidam                # tap
cargo binstall yidam              # binstall — re-run, it replaces
curl -fsSL …/install.sh | sh      # script — re-run, it overwrites
```

A derived repository pins a template commit, not a CLI version, so upgrading the binary does
not by itself change what any repository inherits. `mise run yidam-vendor-update` is the
separate act that adopts a newer prelude; see [Versioning and releases](versioning.md) for why
those are different layers with different lifetimes.

## Uninstalling

```sh
rm ~/.local/bin/yidam             # script (or "$YIDAM_BIN_DIR/yidam")
brew uninstall yidam              # tap
cargo uninstall yidam             # cargo install / binstall
```

Nothing is installed outside that binary. A derived repository's `.yidam/` is its own content
and is unaffected.
