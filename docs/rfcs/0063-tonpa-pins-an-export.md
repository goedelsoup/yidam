# RFC-0063 — tonpa pins a dependency's export, and `derive check` reads it at that pin

- **Status:** Implemented
- **Track:** I46
- **Relates to:**
  - #1508 (a dependency pinned in two places, and a consumer that re-implements `derive check`
    against JSON exports), parent epic #1513
  - #1466 (the pinned path dependency, `rev = "<sha>"`, which this builds on)
  - #1483 (unpacked dependencies are build products, which the unpacked export is too)
  - #1467 (the asks outbox, already shipped as `.yidam/asks/`; see §5)
  - RFC-0045 (`derive check`, whose checks a JSON record is held to)
  - RFC-0018 (`query`, whose `--at` already reads a corpus at a revision)
- **Versioning layers touched:**
  - **tonpa.** A pinned path dependency may declare `export`, a file in the dependency's
    repository. tonpa reads it at `rev`, unpacks it, and locks its hash. The field is
    optional, so every existing `tonpa.toml` and lock still parses. The other direction does not
    hold. `Dependency` and `LockedPackage` ignore keys they do not know, so a CLI from before
    this change drops `export` without a word. It never unpacks the file, and an `install` it
    runs rewrites the lock without `export` or `export_sha256` (§1, "Older CLIs").
  - **Config.** `[derive]` gains `[[derive.records]]` tables, which map span records in a JSON
    file onto `derive check`'s citation. Absent means `derive check` reads exactly what it reads
    today. `DeriveConfig` ignores unknown keys too. So an older CLI reads the table as absent,
    checks no record, and passes. This change makes `[derive]` reject unknown keys, so the next
    key cannot repeat that, and §3.5 says what a consumer does about CLIs already shipped.
  - **CLI.** `yidam query` gains `--dependency <name>`. No command's output changes when the new
    keys are absent.
  - **Reports.** `derive check --format json` gains one optional key on a derivation, `record`,
    the JSON pointer of the record it read (§3.4). It gains `records` at the top, the count of
    records the mappings matched (§3.5). The report schema declares both.
  - **The bump.** It is minor for the CLI. No authored file moves and no migration is owed. An
    upgrade note is owed, because the new keys are what a consumer moves to.
- **Downstream reference case:** city-engine, measured read-only on 2026-11-24 against its six
  sibling corpora at the commits it pins. Nothing was written to any of them.

## Summary

city-engine depends on six corpora through tonpa. It reads one JSON file from each, an engine
pack the corpus computes. Three things are kept by hand around that file:

- the pin, twice, in `corpora.toml` and in `.yidam/tonpa.toml`, with a test holding them equal;
- a copy of each pack in `packs/`, refreshed at each re-pin;
- `check-spans`, 117 lines of Rust that re-implement `derive check` for the packs. It checks
  against each sibling's working tree rather than the pin, because nothing else was at hand.

This RFC makes tonpa hold the pin and the file together (§1). It lets `query` name a dependency
instead of a path and a SHA (§2). And it lets `derive check` read span records from a JSON file
through a declared mapping, against the dependency at its pin (§3). With all three, city-engine
can drop `pack_commit`, the copies, the equality test and the span half of `check-spans`.

## Problem

### 0.1 What was measured

city-engine's six packs hold **1,658** span records between them. Each is an object
`{node, span, tier_span?}`, and each sits at a pointer the pack's producer chose:
`/models/*/parameter_sets/*/parameters/*/source`, `/costs/*/scope`, `/fixtures/*/target/source`
and eight more. Read at each pack's `corpus_commit`:

| what held | records |
|---|---|
| the node resolves, and `span` appears verbatim in the node's file | 1,658 |
| `span` appears inside one of the node's prose strings | 540 |
| the governing quote (`tier_span`, else `span`) appears inside one parsed prose paragraph | 1,608 |
| the governing quote appears inside one blank-line paragraph of the node's **file text** | 1,658 |
| that paragraph carries a standing tag (`[open]`, `[inference]` or `[verified]`) | 1,658 |

Two of those rows decide the design. **1,118 spans quote a typed property**, such as
`value: 0.15`, and not prose. `derive check` today finds a span only in prose strings, so read
as it reads an artifact, two thirds of the packs would fail. And **50 governing quotes carry
YAML's own folding**, a newline and two spaces of indent, so they match the file and not the
parsed string. Every one of the 1,658 sits inside one blank-line paragraph of the file.

The last row is what §3.2's standing-tag check reads. It was measured over every record, not only
those that carry a `tier_span`. The 44 records with no `tier_span` govern by their own `span`, and
each of their paragraphs carries a tag too.

### 0.2 Where the pack lives at the pin

Each pack records `corpus_commit`, the commit it was computed from, and city-engine pins
exactly that. The pack itself is committed one commit later. In all six siblings the next
commit is `compute: engine-pack — 3 files at <pin>`. It touches `.yidam/computed/` and
`.yidam/runs/` only, and its `.yidam/corpus` tree is identical to the pin's. The file the pin
names is an **older** pack. So "read the export at the pin" has to mean the commit that holds
the export, and that commit's corpus is the one the pack was computed from.

### 0.3 What `query` can already do

`yidam query --root ../traffic-design --at 0bd97e7b` answers from git objects at that commit
and never touches the working tree. It has done so since cli v0.4.0. It was verified against
traffic-design at its pin for this RFC. #1508's claim that `query --root` cannot read a
revision is wrong. What is missing is the step from a dependency's name to its path and rev:
`check-spans` would have had to read `tonpa.toml` itself to make it.

## Design

### 1. One pin, and the export beside it

A pinned path dependency may name one file in the dependency's repository as its export:

```toml
[dependencies.traffic-design]
path = "../traffic-design"
rev = "fd586aae…"
export = ".yidam/computed/engine-pack.json"
```

- **Read at `rev`, never from the working tree.** `tonpa add`, `install` and `update` read the
  blob with the same `cat-file` pass that reads the corpus at `rev`. A `rev` whose tree lacks
  the file is refused, naming the path and the commit.
- **Unpacked beside the corpus.** It lands at `.yidam/tonpa/<name>/export/<file name>`, under the
  directory #1483 already treats as a build product.
- **Locked.** The lock entry gains `export` (the declared path) and `export_sha256`. `tonpa
  verify` re-hashes the unpacked file and fails on a mismatch, as it does for a bundle.
- **Only for a pinned path.** On a live path dependency the field is refused, because a working
  tree has no commit to pin. On a fetched bundle it is refused too: the bundle's manifest is the
  right place for an export, and no consumer has asked for one.
- **tonpa does not read the file.** It is bytes with a hash. A JSON pack's own `corpus_commit`
  is the producer's record, and tonpa never compares it to `rev`. §0.2 is why: the two differ by
  one compute commit in every case measured, and each corpus tree is identical to the other.
- **The export must be as new as the corpus at `rev`.** tonpa finds the commit that last wrote
  the export in `rev`'s history (`git log -1 <rev> -- <export>`). It compares that commit's
  `.yidam/corpus` tree with the one at `rev`. If they differ, the corpus changed after the export
  was written, and `add`, `install` and `update` refuse, naming both commits. This reads git
  trees only, so it holds for any export shape. §0.2's six pins pass it: each compute commit's
  corpus tree is the pin's.

`tonpa add ../traffic-design@<sha> --export .yidam/computed/engine-pack.json` writes the table.
With an export declared, `tonpa update traffic-design` does not move `rev` to the sibling's HEAD.
It moves `rev` to the commit on HEAD's history that last wrote the export, and re-reads it there.
That is the corpus's own `compute:` commit. A sibling whose corpus moved past its last export is
refused by the check above, and the fix is to recompute the export there.

**Older CLIs.** A CLI from before this change reads `export` as an unknown key and drops it.
Run against a lock this change wrote, its `install` unpacks no export and rewrites the entry
without `export` or `export_sha256`. Nothing in an older binary can be made to notice. So a
consumer that declares `export` pins its CLI at this release or later (§3.5). A `tonpa verify`
from this release that finds an export declared with no `export_sha256` in the lock fails,
naming `tonpa install`.

### 2. `query --dependency <name>`

`yidam query --dependency traffic-design` reads the named pinned path dependency at its
**locked** `rev`. It is exactly `--root <path> --at <rev>` with both read from
`tonpa.toml` and the lock, and it is implemented as that: no second reader.

- It conflicts with `--root`, `--at`, `--between` and `--across`.
- A live path dependency is refused with the `--root` it should use instead. A fetched
  dependency is refused because it has no git history to read.
- A declared `rev` that differs from the lock is refused, naming `tonpa install`. This is the
  mismatch `tonpa status` already reports.
- **It needs the sibling repository.** `--at` reads git objects, and the materialised copy
  under `.yidam/tonpa/<name>/corpus` has none. So a checkout that holds that copy but not the
  sibling, a CI runner among them, cannot run `query --dependency`. It is refused there with
  the missing path. This is unlike §3.3, which reads the copy. The two still read one corpus:
  `rev` is a full SHA, and tonpa wrote the copy from that commit's tree and locked its digest.

### 3. `derive check` reads span records through a declared mapping

#### 3.1 The declaration

```toml
[[derive.records]]
export = "traffic-design"   # a dependency's export; or `paths = [...]` for files in this repo
records = [
  "/models/*/parameter_sets/*/parameters/*/source",
  "/models/*/formulation",
  "/fixtures/*/target/source",
]
node = "0/node"
span = "0/span"
governing = "0/tier_span"   # optional; defaults to `span`
tier = "1/tier"             # optional
```

- **`export` or `paths`, not both.** `export` names a dependency, and every record cites that
  dependency's corpus at its locked `rev`. `paths` takes the `[derive] paths` grammar over this
  repository's own JSON files, and every record cites this repository's corpus.
- **`records`** are JSON pointers (RFC 6901), with `*` as a segment matching every member of an
  array or object. A pointer that matches nothing in a file is a finding, `derive-record-unmatched`
  at Error. `derive check` fails only on an Error, so a renamed key in the producer fails the
  check rather than emptying it.
- **`node`, `span`, `governing` and `tier`** are relative JSON pointers (`0/node` is the record's
  own `node`; `1/tier` is its parent's `tier`). A price-index value whose node sits two levels up
  is `node = "2/node"`. A record where `node` or `span` does not resolve to a string is
  `derive-record-unmatched` too, at Error, because it is a citation that names nothing.
- A producer that drops a section for one release is a re-pin the consumer makes. The consumer
  removes the pointer in the same commit that moves `rev`.

#### 3.2 What a record is held to

A record is a citation, and it gets the findings an artifact's citation gets, in the same ids:

| check | finding |
|---|---|
| `node` names no node in the cited corpus | `derive-unresolved` |
| `span` does not appear verbatim in the node's file | `derive-span-drift` |
| the governing quote does not appear verbatim in the node's file | `derive-span-drift` |
| the governing quote crosses a blank-line paragraph of the file | `derive-span-crosses-paragraph` |
| the governing paragraph carries no standing tag | `derive-tag-drift` |
| `tier` is declared and the governing paragraph does not carry it | `derive-tag-drift` |

Two of these read differently from an artifact's citation, and §0.1 is the reason for both.

- **A record's span is matched against the node's file text, not its prose strings.** 1,118 of
  1,658 records quote a typed property. A prose-only match would fail them, and a pack quotes a
  property precisely because the value lives there.
- **The paragraph is a blank-line block of the file text.** It is not one of the parsed
  string's paragraphs. All 1,658 governing quotes hold under it, and 50 would fail under the
  parsed rule only because they quote YAML's folding.

`check-spans`' 600-byte reach between span and governing quote is not carried over. A governing
quote is accepted anywhere in the node's file, as long as it sits in one paragraph. The reach was
the consumer's own guess at "governs", and the paragraph is what `derive check` already uses.

A record has no `claim` and no `reach`. So the reach, refusal and answer checks do not apply to
it, and `derive-no-reach` is not reported for a JSON file. A consumer that wants them writes an
artifact.

#### 3.3 The cited corpus at its pin

For `export = "<name>"`, every record resolves against `.yidam/tonpa/<name>/corpus`. tonpa wrote
that tree from `rev` and locked its digest. `derive check` refuses to run a mapping whose
dependency is not installed at its declared `rev`, naming `tonpa install`. It never reads the
sibling's working tree.

#### 3.4 The report

A JSON file contributes one derivation per record. Its `path` is the file and its new optional
`record` is the matched pointer with the wildcards filled, such as
`/models/3/parameter_sets/0/parameters/7/source`. `claim` and `reach` are null, because a record
has neither. `tier` is filled as it is for an artifact: the standing the governing paragraph
carries, and null only when no paragraph could be located. Where the mapping declares `tier`,
that is the value `derive-tag-drift` compared it to. Text output names the file and the pointer
on each finding.

#### 3.5 A CLI that predates the mapping

`DeriveConfig` has never rejected an unknown key. So a CLI from before this change reads
`[[derive.records]]` as nothing, checks no record, and passes. That cannot be fixed in a binary
already shipped, and two changes keep it from mattering.

- **`[derive]` rejects unknown keys from this release.** A later key then fails an older CLI's
  parse instead of passing it. The `Dependency` and `LockedPackage` tables stay lenient, for the
  reason #1466 gives about `rev`: a lock an older CLI cannot read takes every fetched dependency
  down with it.
- **The report counts what it read.** `derive check --format json` carries `records`, the number
  of records the mappings matched. A consumer's CI holds it above zero. An older CLI emits no such
  key, so that assertion fails on it, which is the point.

A consumer pins its CLI at this release before it drops its own checker. Migration step 1 says so.

### 4. What city-engine can retire

With §1 to §3, and nothing else of its own:

- `pack_commit` in `corpora.toml`, and `tonpa_pins_each_corpus_at_its_pack_commit`;
- the copies in `packs/`: the engine reads `.yidam/tonpa/<name>/export/engine-pack.json`;
- the span half of `check-spans`: eleven or so `[[derive.records]]` pointers per pack shape.

Two things stay its own. The `gap_kind = "unexported"` check reads `gameplay-defaults.toml` and
`conformance/harness.toml`, which are TOML and the engine's own, not a dependency's. And a CI
checkout still has no siblings, so `tonpa install` and `derive check` over the exports stay in
`engine-checks`, as `check-spans` is today.

### 5. #1467 and the open-questions drafts

#1508 asks whether #1467 covers city-engine's 55 drafts in `docs/open-questions/` and its six
`*-outstanding.md` files. It does: `.yidam/asks/<package>/<slug>.md` records a question to a
cited corpus with its pin, and `due` reports it. city-engine is pinned at cli 0.25.0 and has not
moved its drafts there. That is the consumer's migration, not a missing feature, and this RFC
does not change the asks format.

## What this does not do

- **It does not interpret an export.** tonpa hashes the file. Only a declared mapping reads
  inside it, and only for spans.
- **It does not add exports to fetched bundles.** A bundle's manifest would carry one. No
  consumer has asked.
- **It does not give records a reach.** A record is a citation and not an argument.
- **It does not change how an artifact's citation is matched.** `.yml` and `.md` artifacts keep
  RFC-0045's prose-paragraph rule. §3.2's file-text rule applies to records only.
- **It does not carry over `check-spans`' price-index scope and base checks as special cases.**
  They are records like any other, with `node = "2/node"` or wherever the shape puts the node.

## Alternatives considered

- **Export plus provenance from the file.** tonpa could read the pack's `corpus_commit` and check
  that its corpus tree matches the one at `rev`. That couples tonpa to one JSON shape. §1 gets the
  same guarantee from git instead: the commit that last wrote the export must have the corpus tree
  at `rev`. §3.3 alone would not give it. It shows the quoted spans still hold at `rev`, not that
  the export was computed from that corpus.
- **Find every object holding `node` and `span`.** This needs no pointers, and it found all 1,658
  records in this measurement. But it cannot tell a span record from an object that merely has
  those keys, and it gives a renamed key no way to be noticed. Declared pointers fail loudly
  (`derive-record-unmatched`).
- **Require the producer to emit `cites:`.** Packs would carry derive's own citation shape and
  need no mapping. But every producer would change, and the pack format belongs to its producer.
- **Match a record's span against prose strings, as for an artifact.** §0.1: 1,118 failures on
  packs that are correct.

## Migration

None is owed. A consumer adopting this:

1. pins its CLI at this release or later, since an older one ignores both new keys (§3.5);
2. adds `export` to each pinned dependency and re-pins to the corpus's `compute:` commit
   (`tonpa update`);
3. declares `[[derive.records]]` for each pack shape, and holds the report's `records` above zero;
4. points its reader at `.yidam/tonpa/<name>/export/`;
5. drops its own pin, copies and span checker.

## Falsifier

This RFC is wrong if a consumer's span records are found that the file-text paragraph rule
accepts and a person reading the node would say are not governed by the quoted tag. The check is
the 1,658 records above. Every governing quote sits in one paragraph, and in 1,457 of the 1,469
records that declare a tier, the governing quote carries that tier. The other 12 are records
where the measurement script's guess at the tier's location was wrong, and the implementation
re-measures them through the declared `tier` pointer before it ships.

## Unresolved

- Whether a dependency should declare more than one export. city-engine reads one file from each
  corpus, so `export` is one path. A list can come later without breaking the key.
