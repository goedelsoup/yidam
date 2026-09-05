# Troubleshooting

## Start with `yidam doctor`

It answers, in one screen, the questions that were previously spread across a stderr warning, a
CI step and three reports. It writes nothing and does no network, so it is safe against a
checkout you only mean to inspect — which the rest of the reports are not.

```console
$ yidam doctor
yidam doctor — /home/you/my-domain

  ok    repository   /home/you/my-domain
  fail  provenance   no .yidam.toml
                     → mise run yidam-vendor-update
  ok    binary       this repository pins no binary — nothing can be shadowed
  ok    path         this repository pins no binary — PATH order does not matter
  warn  prelude      no .yidam/.vendor/prelude/ — this repository carries no vendored prelude
                     → mise run yidam-vendor-update
  warn  index        no /home/you/my-domain/.yidam/index
                     → yidam index-build (needs the `index` feature)
  ok    regen        every REGEN block holds what its generator produces
  ok    catalog      no TTL declared — 1 source(s) never expire.
  fail  corpora      not installed: hydrology
                     → mise run tonpa-install
  ok    build        0.5.0 (78544f8) with features: reports, index, export-sqlite, ...

2 failing check(s), 2 warning(s).
```

Every check names its own remedy. **It exits nonzero on what is wrong now**; warnings — no
index, an old pin — are reported and do not affect the exit code unless you pass `--strict`,
which is the reading a CI job wants.

| Check | The question |
|---|---|
| `repository` | Am I in a derived repository? |
| `provenance` | Does this repository record where it came from? |
| `binary` | Is the running binary the one this repository pins? |
| `path` | Is `.yidam/bin` ahead on `PATH`? |
| `prelude` | How stale is the vendored prelude? |
| `index` | Is the index built, and is it current? |
| `regen` | Are the REGEN blocks current? |
| `catalog` | Have any source records aged out? |
| `corpora` | Did the corpora this repository depends on arrive? |
| `build` | Which yidam is this, and what can it do? |

A `skip` is not a pass. Every check after `repository` skips when there is no repository to
check, so a screen of skips means the first line is the problem.

---

## `not a yidam repository`

```
Error: not a yidam repository: /tmp/streamflow is not inside a git repository
  yidam locates a repository with `git rev-parse --show-toplevel` and found none, so it
  fell back to the working directory. Run this from inside a derived repository.
```

yidam finds the corpus by walking up to the git root. Two ways to hit this:

**Nothing is a git repository.** Reading an [example](../examples/README.md) has this shape —
copy it out and initialise it, because running the binary inside the template's own directory
finds *yidam*, which is not a corpus:

```sh
cp -R examples/streamflow /tmp/streamflow
cd /tmp/streamflow && git init -q && git add -A && git commit -qm genesis
yidam graph-check
```

**The git root is not the corpus.** A corpus nested inside a larger repository resolves to the
outer root. Run from the corpus root, or overlay it so it has its own `.yidam/`.

## `unrecognized subcommand`

The command is real; your build does not carry it. Check what you have:

```console
$ yidam --version
yidam 0.5.0 (78544f8) [reports tonpa]
```

That build has no `index`, so `yidam index-build` is not there. The feature table is in
[Installation](installation.md#check-which-build-you-have); released binaries carry the default set.

This failure mode is why `tonpa` is a default feature despite costing an HTTP stack: it was the
one whose absence broke an *instruction* rather than removing a capability, and inside a script
with output redirected `unrecognized subcommand` is indistinguishable from success.

## The gate fails and the finding is not mine

`yidam lint` asks *did this change make the corpus less clean?*, not *is the corpus clean?* If
you have inherited debt, adopt the ratchet rather than fixing everything first:

```sh
yidam lint --init-baseline   # write .yidam/lint-baseline.yml if absent; safe to re-run
```

Existing findings become the baseline; new ones fail. After a genuine cleanup pass, re-bless —
**a baselined finding that is later repaired also fails the build**, because the entry no longer
describes reality and a stale ratchet has stopped ratcheting:

```sh
yidam lint --bless
```

`yidam lint --explain` prints each check's rationale beside its findings, which is usually
faster than guessing what a check is for.

## A REGEN block is stale and CI is red

In a derived repository a stale `<!-- REGEN: … -->` block is a failing build. Refresh them all:

```sh
mise run regen        # or: yidam regen
yidam regen --check   # what CI runs — reports staleness, writes nothing
```

**Careful:** `regen`, `status`, `open-questions` and the other index commands *rewrite files*.
Twenty-three commands do; `yidam --help` marks each with a `*`. Against a checkout you only mean
to read, `yidam doctor` is the one that is guaranteed not to touch anything.

## The editor shows nothing, or disagrees with CI

**Nothing at all.** The extension activates only on a workspace containing `.yidam.toml` or
`.yidam/`. Opening the yidam template repository itself activates nothing — it has neither, by
design.

**Nothing rendered, but it activated.** It resolves a binary and never downloads or builds one.
Run `yidam: Show binary and contract status` to see which one answered. If the binary speaks a
report contract the extension does not understand, verdict features are disabled and the status
bar says so.

**Verdicts disagree with CI.** Almost always the wrong binary. The extension prefers the
repository's own build (`.yidam/bin`) over `PATH` precisely because a machine-wide binary is one
per machine while the pin is one per repository. `yidam doctor` answers this directly — the
`binary` and `path` checks exist for it.

**Findings look too mild.** Baseline membership outranks check severity: inherited debt renders
as a Hint however severe the check is. `yidam.lint.showBaselined` controls whether they appear
at all.

## `serve --mcp` returns `degraded`

The binary has no `index` feature, so `retrieve` is doing keyword search rather than semantic
search. It says so on every call rather than returning keyword results as though they were
embeddings. Every other MCP tool is unaffected — both transports are in the light default build.

To get semantic retrieval you need a build with `--features index` (protoc 31 at build time) and
a built index:

```sh
yidam embed
yidam index-build
```

[Connecting an agent](mcp-server.md) covers what `degraded` and `origin` mean on the wire.

## The index is stale

`yidam index-status` reports freshness against the corpus; `doctor`'s `index` check summarises
it. Rebuild after corpus changes:

```sh
yidam embed && yidam index-build
```

If a *provider* is the question rather than the index — whether some other embedding runtime
reproduces this index — `yidam index-verify --provider <cmd>` checks it against the index's
reproducibility contract.

## Catalog sources have aged out

`catalog-expired` names each one. TTLs are opt-in in both forms, and absent means nothing
expires: set `[catalog] ttl_days` in `.yidam/config.toml` for a corpus whose sources age alike,
or `ttl_days:` on the individual entry, which is the primary form because a gauge record and a
statute do not age at the same rate. See [Configuration](configuration.md#catalog-ttl_days).

## A corpus a dependency declares is not there

```
fail  corpora      not installed: hydrology
                   → mise run tonpa-install
```

`mise install` fetches the corpora `.yidam/tonpa.toml` declares, through a `postinstall` hook.
**A green `mise install` is not proof they arrived**: mise logs a failing postinstall hook as a
warning and exits 0 anyway, so a corpus that could not be fetched leaves one line in a log that
scrolls past. This check is what still says so afterwards.

Run `mise run tonpa-install` on its own — invoked directly it returns a real exit code — and
read what it says about the dependency it could not get. A URL that has rotted is the common
cause; the remedy is a `tonpa.toml` edit, not a re-run.

Three other readings of the same check:

| Line | What it means |
|---|---|
| `does not match tonpa.lock: <name>` | The bundle on disk is not the one this repository pinned. Re-run the install: it re-fetches at the locked hash. |
| `declared but never pinned: <name>` | A `warn`. Normal between `tonpa add` and the first install — the dependency has no lock entry, so nothing can verify it. |
| `N path` | Path dependencies, read from a sibling checkout. Nothing to fetch, and never graded. |

It answers offline. It compares what is unpacked under `.yidam/tonpa/` against `tonpa.lock`,
which is the whole correctness story for a fetched corpus — so it works on a plane, and on a
binary built without the `tonpa` feature, which can read a corpus but not fetch one.

---

## The pin is old, or the prelude is stale

`doctor`'s `prelude` check reports the age of the pinned commit — the *commit's* date, not the
date you last ran the vendor step. To adopt a newer template:

```sh
mise run yidam-vendor-update           # re-vendor and re-pin
YIDAM_REF=v0.2.0 mise run yidam-vendor-update   # target a tag or branch
```

This re-vendors `.yidam/.vendor/prelude/` and rewrites `.yidam.toml`. It does not touch domain
content. Upgrading the *binary* is a separate act on a separate release train — see
[Versioning and releases](versioning.md).

## Still stuck

- `yidam <command> --help` is generated from the same source as the behaviour, so it cannot
  disagree with the binary you have.
- [Open an issue](https://github.com/goedelsoup/yidam/issues) with the output of
  `yidam doctor` and `yidam --version` — between them they answer most of the first round of
  questions.
