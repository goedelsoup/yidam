# RFC-0068 — A release ships the spec a producer writes against

- **Status:** Implemented
- **Track:** I51
- **Relates to:**
  - #1565 (publish the node and ontology schema and conformance cases as a release asset),
    parent epic #1513
  - #1510 and RFC-0064, which this was split from. RFC-0064 lets a corpus be produced from
    outside yidam, and states that a producer "still reads the Rust parser as the spec".
  - #1501 and RFC-0056 (the calculator kit), the other half of the reach theme
  - RFC-0016 (the report envelope) and RFC-0065 (a pin names its release)
- **Versioning layers touched:**
  - **Release.** Each `cli/v*` release gains one asset, `yidam-spec-<version>.tar.gz`, with
    its `.sha256`. It is listed in `SHA256SUMS` and attested like the binaries (§5).
  - **Tree.** A new `yidam/sdks/conformance/` holds the committed schemas, the vocabulary
    and the conformance cases (§2, §3, §4). The MCP contract and the report schema stay
    where they are and are copied into the asset.
  - **CLI.** None. No command, flag or output changes. `yidam schema` writes what it writes
    now, and the committed copies are gated against it.
  - **Reports.** None. The cases key on fields the envelope already has.
  - **Prelude.** None. Nothing `yidam-vendor-update` copies changes.
  - **Docs.** A page section says what the asset holds, how to fetch it for a pin, and how a
    pin with no release reads the same files from source (§6).
  - **The bump.** Minor for the CLI, since the release gains an asset. No authored file
    moves, so no upgrade note is owed.
- **Downstream reference cases:**
  - watermark-directory. Its CI workflow was read through the GitHub API on 2026-11-24, at
    its last commit of 2026-11-18. Nothing was cloned or fetched.

## Summary

A producer that writes a corpus from outside yidam has nothing versioned to write against.
The node and ontology shapes are defined by the Rust parser. `yidam schema` writes JSON
schemas into a repository, but no release carries them. The MCP contract and the commit
vocabulary are reachable only from a source checkout.

This RFC makes each release carry one more asset, `yidam-spec-<version>.tar.gz`:

- the repository-independent JSON schemas and the report envelope schema (§2);
- conformance cases: small corpora with the `graph-check` and `lint` results they must
  produce (§3);
- the MCP contract, its cases and corpora, and the closed commit vocabulary (§4);
- a `manifest.json` naming every version the files are stamped with (§1).

Every file in it is committed and gated in this repository, so the release only packs a
directory (§5). A pin with no release reads the same files from a checkout (§6).

## Problem

### 0.1 What was measured

watermark-directory's `ci.yml` has a job named `corpus`. Its step "The vendored MCP contract
matches the pin" does three comparisons:

- `diff -u` of the pin's `yidam/sdks/parity/mcp/tools.json` against its vendored
  `src/watermark/agent/mcp_contract.json`;
- `diff -ru` of the pin's `yidam/sdks/parity/mcp/cases` against its vendored
  `tests/fixtures/yidam-mcp/cases`;
- an `ast.parse` of the pin's `yidam/sdks/python/yidam_core/git.py`. It pulls out
  `EPISTEMIC_VERBS` and `OPERATIONAL_VERBS` and compares them with its vendored
  `commit_vocabulary.json`.

To make those three comparisons, the job blobless-clones yidam and checks out the pinned
commit on every run. A comment in the step says why the verbs are compared by content: *"the
vendored form is JSON and the source is a Python module"*.

The same corpus projects its nodes from Python (RFC-0064). Nothing it can download says what
a node or a `.ont.yml` may contain.

### 0.2 What already exists

- **The schemas are generated, and five of them are repository-independent.**
  `cmd/schema.rs::all()` returns `corpus-node.json`, `corpus-ontology.json`,
  `corpus-universal.json`, `catalog-entry.json` and `authorship.json`. `class_schemas` adds
  one per class. Those depend on a repository's ontology and are not in scope.
- **The report envelope has a schema.** `yidam/sdks/parity/fixtures/reports/report.schema.json`
  describes what every `--format json` emits. Its `id` field is described as the stable
  identifier a consumer keys on.
- **The MCP contract is versioned.** `yidam/sdks/parity/mcp/` holds `tools.json`, `cases/`,
  `corpora.json`, the fixture corpora, `VERSION` and `CONTRACT_SHA`.
- **The vocabulary is certified.** `yidam vocabulary --format json` reports each verb and its
  kind. Membership comes from the lists that are proven against the Dafny spec, and GRAPH.md
  supplies only the prose.
- **Fixture corpora are already committed.** `parity/mcp/corpus/` and
  `parity/fixtures/reports/basic/` are corpora with a `.yidam/` tree. `basic/stage.toml`
  records how a runner turns one into a git repository.
- **The release packs per-target artifacts.** `release.yml` builds a `.tar.gz` and an
  `.mcpb` per target. The publish job downloads them into `dist/`, writes `SHA256SUMS`,
  attests `dist/*.tar.gz` and `dist/*.mcpb`, and creates the release.

### 0.3 What does not exist

- No conformance case says what `graph-check` and `lint` must report for a given corpus.
  The goldens under `parity/fixtures/reports/` freeze whole reports for one corpus. They are
  the CLI's regression tests, not a spec a second implementation can meet.
- No release asset is platform-independent. Everything in `dist/` is a binary.

## Design

### 1. The asset

One tarball per release, `yidam-spec-<version>.tar.gz`, where `<version>` is the CLI version
the tag names. It unpacks to one directory, `yidam-spec-<version>/`:

```text
yidam-spec-<version>/
  manifest.json
  schemas/
    corpus-node.json
    corpus-ontology.json
    corpus-universal.json
    catalog-entry.json
    authorship.json
    report.schema.json
  conformance/
    README.md
    VERSION
    stage.toml
    cases/<case>/repo/…          a corpus, with its .yidam/ tree
    cases/<case>/expected.json
  mcp/
    tools.json
    VERSION
    CONTRACT_SHA
    corpora.json
    cases/…
    corpus*/…
  vocabulary.json
```

`manifest.json` is the one file a consumer reads first:

```json
{
  "format_version": "1",
  "cli": "0.27.0",
  "commit": "<the tagged commit>",
  "versions": {
    "report": "1",
    "mcp_contract": "0.35.0",
    "contract_sha": "<CONTRACT_SHA>",
    "vocabulary": "1",
    "conformance": "1"
  },
  "files": {
    "schemas/corpus-node.json": "yidam/sdks/conformance/schemas/corpus-node.json",
    "mcp/tools.json": "yidam/sdks/parity/mcp/tools.json"
  }
}
```

`files` maps every path in the asset to the path it was copied from. That map is what lets
a pin with no release read the same files from a checkout (§6). It is complete: a file in
the tarball with no entry fails the packaging gate (§5).

### 2. The schemas are committed, and drift fails a gate

`yidam/sdks/conformance/schemas/` holds the five files `schema::all()` returns. Each is
byte-identical to what `yidam schema` writes into `.yidam/schemas/`, `$comment` marker
included. A consumer can diff its own `.yidam/schemas/` against the asset and see only
the per-class files.

A gate under `tests/gates/` renders `schema::all()` and compares it with the committed
files, byte for byte. A missing file and a file `all()` does not name both fail. As with the
other goldens, `UPDATE_GOLDENS=1` rewrites them. So a change to a node's shape is a diff in
this directory, in the same PR, and a reviewer sees it.

`report.schema.json` is not moved. It is copied from `parity/fixtures/reports/`, which is
where its own gates read it.

The per-class schemas are not shipped. They are compiled from a repository's `.ont.yml`, and
`compile_class_schema` is already a parity function with fixtures.

### 3. Conformance cases

`yidam/sdks/conformance/cases/<case>/` holds a corpus under `repo/` and the result it must
produce in `expected.json`:

```json
{
  "graph_check": {
    "passed": false,
    "nodes": [".yidam/corpus/concept/traversal.yml"],
    "classes": []
  },
  "lint": [
    { "check": "orphan-out", "severity": "error", "node": ".yidam/corpus/concept/traversal.yml" }
  ]
}
```

**What is fixed is which checks fire, on which node, at which severity.** A message, a
rationale and a `detail` are free to change. The report envelope already says a consumer
keys on `id` and the prose may change. A case that froze the prose would break a consumer's
comparison on every reworded rationale.

**`graph_check` is matched by file.** Its JSON names each issue in prose, such as
`"orphan node: no outgoing links"`, and has no stable identifier per issue. So the case
records whether it passed and which files had issues. `nodes` lists the paths in
`nodes_with_issues`, and `classes` lists the paths in `classes_with_issues`, which holds an
`.ont.yml` that did not parse. Either list being non-empty fails the check. The `lint` list
names each defect by check id. Each of `graph-check`'s issues has a lint check that reports
the same defect: `missing-class`, `unknown-class`, `missing-label`, `orphan-out`,
`dangling-edge`, which takes both a broken link and a link entry with no `target:`, and
`malformed-yaml`, which takes a node or a class file that did not parse. Giving
`graph-check` its own ids is a separate change to its report, and is not needed here.

**`graph-check` does not read `outgoing: optional`.** It reports every node with no
outgoing link, whatever its class declares (`cmd/corpus.rs`, the `links.is_empty()`
branch). Only lint's `orphan-out` reads the declaration. A case records the two
disagreeing, as they do. Making `graph-check` read it is a change to its output and is not
made here.

**`lint` is the complete list for the corpus at that release.** It is run with no baseline.
A consumer that implements a subset of the checks filters the list by `check`. A check that
fires on no case does not appear, and that is fine.

**A case becomes a repository the way `basic/` does.** The gate copies `repo/` to a
temporary directory and makes one commit with a fixed date and the `establish:` verb, per a
`stage.toml` in `conformance/`. Then it runs `graph-check --format json` and
`lint --format json`, projects both onto the shape above, and compares. `UPDATE_GOLDENS=1`
rewrites `expected.json`. The recipe ships in the asset, so a consumer's runner stages the
corpus the same way.

**The starting set covers the node and ontology shapes, not every check.**

| Case | What it holds | What it must produce |
|---|---|---|
| `minimal` | one class, two nodes that each link to the other | passes, no lint errors |
| `orphan` | a node with no outgoing link | fails on that node; `orphan-out` |
| `optional-outgoing` | the same node, its class `outgoing: optional` | `graph-check` fails on that node; no `orphan-out` |
| `broken-link` | a link to a node that does not exist | fails on that node |
| `missing-label` | a node with no `label` | fails on that node |
| `unknown-class` | a node whose `class:` names no `.ont.yml` | fails on that node |
| `missing-required-property` | a `required: true` property omitted | its lint error |
| `claim-standings` | bare, bracketed and qualified tags on a `type: claim` property | no tag finding |

The table names what each case is for. The check ids in each `expected.json` are written by
the gate, not by hand, so the RFC does not restate them. A case is added without touching
any other case.

### 4. The MCP contract and the vocabulary ship in the same asset

`mcp/` is a copy of `yidam/sdks/parity/mcp/`, which is already versioned and gated by
`mcp_contract_digest`. Nothing about it changes here.

`vocabulary.json` is committed at `yidam/sdks/conformance/vocabulary.json`:

```json
{
  "format_version": "1",
  "verbs": [
    { "verb": "establish", "kind": "epistemic" },
    { "verb": "extract", "kind": "operational" }
  ]
}
```

It holds the verbs and their kinds only. It has none of the build stamp, root or GRAPH.md
prose that `yidam vocabulary --format json` adds, because those vary by machine and would
make the file differ on every run. A gate compares it with the certified lists, sorted by
kind and then by verb. watermark-directory's vendored `commit_vocabulary.json` already holds
`verb` and `kind` per entry, so its comparison becomes a JSON-to-JSON one.

### 5. The release packs a directory

`scripts/spec-bundle.sh <version> <commit> <out-dir>` assembles the tree in §1 from the
committed sources, writes `manifest.json` and packs it. Each `versions` value is read from
one committed file, not from a binary:

| Key | Read from |
|---|---|
| `report` | the `const` of `version` in `parity/fixtures/reports/report.schema.json` |
| `mcp_contract` | `parity/mcp/VERSION` |
| `contract_sha` | `parity/mcp/CONTRACT_SHA` |
| `vocabulary` | `format_version` in `conformance/vocabulary.json` |
| `conformance` | `conformance/VERSION`, new here, bumped when the shape of `expected.json` or `stage.toml` changes |

So it runs once, on any runner, with no build.

In `release.yml`, the publish job runs it after `download-artifact` and before
`Collect checksums`. The new tarball and its `.sha256` are then in `dist/` with everything
else: in `SHA256SUMS`, in the `gh release create` upload, and in the attestation step,
whose `dist/*.tar.gz` subject already matches it. No subject line is added.

It is the first `.tar.gz` on a `cli/v*` release that holds no binary. Every installer
selects its asset by target. `install.sh` and `render-formula.sh` build the name
`yidam-<version>-<target>.tar.gz` and match it whole. mise's `github:` backend picks the
asset whose name carries the running platform's OS and architecture, and
`yidam-spec-<version>.tar.gz` carries neither. The `install-channels` workflow's mise job
installs the latest release weekly and on dispatch. It is dispatched once after the first
release with the asset, so that release proves mise still picks the binary.

A gate runs the script into a temporary directory. It fails when:

- a file in the tarball has no `files` entry, or an entry names no file;
- a `versions` value differs from the committed file the table above names for it;
- a committed file under `yidam/sdks/conformance/` is missing from the tarball.

The script and gate never call the network.

### 6. How a consumer reads it

A pin that names its release (RFC-0065's `release` key) fetches:

```text
https://github.com/goedelsoup/yidam/releases/download/cli/v<release>/yidam-spec-<release>.tar.gz
```

It verifies the `.sha256`, and optionally the attestation with `gh attestation verify`. Then
it diffs its vendored copies against `mcp/`, `vocabulary.json` and `schemas/`, and runs its
own validator over `conformance/cases/`.

A pin with no release, such as an untagged commit, has no asset to fetch. It reads the same
files at the source paths `manifest.json` lists, which are the same at every commit since
this lands. So it keeps one way to compare, either from the asset or from a checkout.

For watermark-directory, the "vendored MCP contract matches the pin" step becomes one
download and three diffs. The blobless clone stays only for the step that measures how far
the pin has drifted, which needs history.

## What this does not do

- **It does not ship per-class schemas.** They belong to a repository's ontology. A producer
  compiles them with `compile_class_schema` or reads them from `yidam schema`.
- **It does not freeze report prose.** The cases fix check ids, nodes and severities.
- **It does not give `graph-check` per-issue ids.** That is a report change of its own.
- **It does not ship the parity fixtures.** `parity/fixtures/` is a function-level suite for
  the SDKs. Whether a producer wants `parse_instance` and its fixtures in the asset is open
  (Unresolved).
- **It does not add a CLI command.** The asset is packed from committed files. Nothing is
  generated at release time.
- **It does not change watermark-directory.** It gives that repository something to fetch.
  Switching to it is that repository's change.

## Alternatives considered

- **One asset per kind.** Schemas, cases, contract and vocabulary as four downloads. A
  consumer fetching only the contract saves a few kilobytes, and has four URLs and four
  version stamps to keep in step. One tarball with a manifest is one of each.
- **Generate at release time with a `yidam spec --out` command.** It would commit nothing
  new. It is a new command and its registrations, the release would have to build a binary
  before packing, and a pin with no release would get nothing.
- **Freeze whole JSON reports as the expectation.** This is stricter, and it is what the
  `basic/` goldens do for the CLI's own sake. For a second implementation it turns every
  rationale edit into a broken comparison, and the envelope already tells consumers not to
  key on prose.
- **Schemas and cases only, as the issue's minimum.** The MCP contract and the verbs would
  stay source-only, and the one consumer measured would still clone for two of its three
  comparisons.

## Migration

None for an authored corpus. No file under `.yidam/` moves, and no command changes output.
A consumer that compares against a source checkout keeps working, because the source paths
do not move (§2 copies `report.schema.json` and `parity/mcp/` and does not move them).

## Falsifier

This is wrong if a consumer that switches to the asset still needs the checkout for anything
other than history. The test is watermark-directory's contract step. After the switch, it
should read only the tarball.

It is also wrong if the cases churn. A case's `expected.json` should change only when a
check is added, removed, or changes which node it fires on or at what severity. If ordinary
message edits rewrite cases, §3 has fixed the wrong fields.

## Unresolved

- **Whether the parity fixtures ship.** A producer writing nodes in Python is close to an SDK.
  `parse_instance` and `compile_class_schema` fixtures would let it check its writer at the
  function level. They are versioned by `parity/VERSION`, so adding them later is one more
  `files` entry and one more `versions` key.
- **A stable latest URL.** `releases/latest/download/yidam-spec.tar.gz` would need an
  unversioned name. A consumer pinned to a release does not need it, so it is left out.
