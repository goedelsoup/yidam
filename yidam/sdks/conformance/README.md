# Conformance

What a producer that never builds yidam writes against (RFC-0068). Each release packs this
directory, with the report schema, the MCP contract and a manifest, into
`yidam-spec-<version>.tar.gz`.

| Path | What it is | Held by |
|---|---|---|
| `schemas/` | The five repository-independent schemas, byte-identical to what `yidam schema` writes | `tests/gates/spec_asset.rs` |
| `vocabulary.json` | The closed commit vocabulary: each verb and its kind, sorted by kind and then by verb | `tests/gates/spec_asset.rs` |
| `cases/<case>/repo/` | A corpus, with its `.yidam/` tree | `tests/suite/conformance_cases.rs` |
| `cases/<case>/expected.json` | Which checks that corpus fires, on which node, at which severity | `tests/suite/conformance_cases.rs` |
| `stage.toml` | How a case becomes a repository | `tests/suite/conformance_cases.rs` |
| `VERSION` | Bumped when the shape of `expected.json` or `stage.toml` changes | `tests/suite/conformance_cases.rs`, which records the key paths each version ships |

## Running a case

1. Copy `cases/<case>/repo/` into an empty directory.
2. Run `git init -b <branch>` there, and make each of `stage.toml`'s `commits`, with every
   file staged, as its `author`, at its `date`.
3. Run your validator. Compare with `expected.json`:
   - `graph_check.passed`, and the files in `graph_check.nodes` and `graph_check.classes`,
     which are the nodes and class files `graph-check` found an issue in;
   - `lint`, the complete list of findings with no baseline, as `check`, `severity` and
     `node`.

A message, a rationale and a `detail` are not part of a case and may change in any release.
A validator that implements a subset of the checks filters `lint` by `check`.

## Changing a case

Every generated file here is rewritten by `UPDATE_GOLDENS=1` on the gate that holds it.
`expected.json` is written by the gate, not by hand. A case is added as a new directory under
`cases/`, and touches no other case.
