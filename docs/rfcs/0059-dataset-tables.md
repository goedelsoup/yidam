# RFC-0059 — A table is a catalog entry that declares its columns

- **Status:** Draft
- **Track:** I42
- **Relates to:**
  - #1504 (tabular evidence has no home; the evidence this answers), parent epic #1513
  - RFC-0050 (the list property type, whose `fields:` grammar this reuses for columns)
  - RFC-0049 (the warehouse contract, weighed as the declaration and declined in §4.1)
  - RFC-0058 (the `signal` REGEN block, which is how prose quotes an aggregate of a table)
  - RFC-0023 (the vault, where bytes too large to track already go)
- **Versioning layers touched:** CLI and prelude. A catalog entry of `type: dataset` gains two
  optional keys, `columns:` and `key:`. `lint` gains three checks. `yidam run` checks a declared
  table it reads before the run and one it writes after. The kit gains a table reader, and
  `yidam run` hands it the declarations in one new file. A markdown link to a catalog entry may
  carry a `#row=` fragment. `prelude/guidelines/directories.md` gains one paragraph. The bump is
  minor, because an old binary ignores both keys and reads the fragment as a heading anchor
  (§"Migration"). No report schema, MCP contract or SDK change.
- **Downstream reference cases:** measured read-only on 2026-11-23 at `origin/main` of every
  derived repository on the measuring machine. The figures come from ohio-campaign-finance-2025-2026
  (e91e01c), allen-county-ohio-affordability (4de4cf3), matt-huffman (7fcec05),
  ohio-education-funding (ab0cdb32), watermark-directory (691b9db1), karl-kerschner (d776819),
  legislation-tracker, hb-96, land-use-activity and allen-county-ohio.
  allen-county-ohio-democratic-party is not checked out there. The issue's figures for its
  precinct results are cited, not re-measured.

## Summary

A corpus holds a claim in a node, a source in the catalog and a computation in a capability. A
table has none of these homes. Today it is a TSV that a calculator reads by path, that no
catalog entry names and that no node can cite. Or it is a class whose nodes are rows, written
from a template. Or it is a markdown table pasted into prose.

This RFC gives a table the home a source already has. A catalog entry of `type: dataset` whose
location is a tracked file may declare the file's columns and its key:

```yaml
---
name: sos-receipts
description: Contributions received by Ohio statewide campaigns, as extracted from the SOS bulk files.
type: dataset
obtained: true
location:
  - kind: file
    value: data/extract/receipts.tsv
key: [filing_id, line]
columns:
  - { name: filing_id,   type: string, required: true }
  - { name: line,        type: number, required: true }
  - { name: committee,   type: ref,    required: true }
  - { name: received,    type: date }
  - { name: amount,      type: number, unit: USD }
  - { name: contributor, type: string }
---
```

`lint` holds the file to the declaration. `yidam run` holds a calculator to it on both sides. A
node cites one row as `[the largest receipt](../../catalog/sos-receipts.md#row=OH-2025-88412,3)`.
A figure computed over the table reaches prose through a calculator's signal and an RFC-0058
block, which already keeps it current.

## Problem

### 1.1 What was measured

Two scripts read each repository's `origin/HEAD` through git, never a working tree. One counted
tracked files ending `.tsv` or `.csv`, their bytes and their rows. The other classified every
class by how alike its nodes are, and counted pipe-table rows in node prose.

| Corpus | Tabular files | Bytes | Rows | Has capabilities |
|---|---|---|---|---|
| ohio-campaign-finance-2025-2026 | 17 | 9.88 MB | 181,000 | yes |
| karl-kerschner | 11 | 6.38 MB | — | no |
| allen-county-ohio-affordability | 94 | 23.1 MB | — | no |
| matt-huffman | 141 | 15.6 MB | — | no |
| ohio-education-funding | 64 | 19.8 MB | — | no |
| watermark-directory (no corpus) | 72 | 60.9 MB | — | no |

### 1.2 The tables a calculator reads are named by nothing

ohio-campaign-finance-2025-2026 is the one corpus whose calculators read tables. Its extract
directory holds 9.79 MB in 11 files. The three largest are `receipts.tsv` (4,627,440 bytes,
74,988 rows), `geography.tsv` (2,498,084 bytes, 66,009 rows) and `history.tsv` (1,061,496
bytes, 8,876 rows). Every one is a declared `reads:` glob, and every one is written by an
`extract:` or `refresh:` commit.

None is named by a catalog entry, and none is named in any node's prose. A reader of the corpus
learns that receipts exist from the calculators' code, not from the corpus. Nothing says what
the columns are, which column is the key, or which column names a node.

One defect was found by reading the files beside the prose. The rows of `sources.tsv` sum to
2,032,066, and the decision that cites the total says 2,032,068. Nothing could have caught
that, because the table has no declaration to check against and the prose quotes a figure no
calculator writes.

`results-2026.tsv` is the other shape. It has 194 rows and 12 columns, it is kept by hand
across 2 commits, and the capability `money-vs-result` reads it. A typo in its `votes` column
would reach the calculator as a string.

### 1.3 Records written as nodes

Five classes hold nodes that are rows in all but name.

| Corpus | Class | Nodes | Shape |
|---|---|---|---|
| legislation-tracker | `bill` | 1,562 | 98% open "Generated from the LIS record" |
| hb-96 | `position` | 1,191 | 100% templated prose |
| allen-county-ohio-affordability | `measure` | 1,237 | the same 7 properties, 91% templated |
| land-use-activity | `parameter` | 683 | one node per value |
| land-use-activity | `profile-point` | 208 | one node per point |

ohio-campaign-finance-2025-2026's `contribution` class is the honest case. It has 454 nodes,
449 of them with the same 6 properties, and each has prose a person wrote. A table would hold
the 449 property sets, and a node would remain for each contribution worth an argument.

The prelude says "one concept, one artifact, one relationship" (`GRAPH.md`), and "One concept
per file" (`directories.md`). A row is not a concept, but the corpus offers it no other place.
travel-demand and urban-environment-health each record a decision to model a parameter as a
class, one node per value. traffic-design's `cmf-clearinghouse` keeps a committed projection
of a CSV as nodes.

### 1.4 Tables pasted into prose

| Corpus | Pipe-table rows in node prose | Nodes |
|---|---|---|
| karl-kerschner | 1,493 | 91 |
| hb-96 | 718 | — |
| rural-caucus | 537 | — |

karl-kerschner's largest node is 225 KB, most of it table. rural-caucus pastes contribution
figures that a TSV elsewhere holds. allen-county-ohio prints fixed-width tables in prose and
keeps a `table` property in 33 nodes. Its decision `a-table-a-machine-cannot-read` says "564 of
its nodes print one". None of these is read by a machine, and none is checked.

### 1.5 Why the existing machinery does not reach it

- **The catalog** knows `dataset` as a `type:` label and `file` as a location kind, and that
  is all. No entry carries a schema.
- **`malformed-table`** reads markdown pipe tables in catalog files only.
- **A capability's `reads:`** are path globs. `yidam run` checks out the files they cover and
  records each file's sha256 in the receipt. So a calculator can read any tracked TSV today.
  It cannot tell a TSV whose columns changed from one whose rows did.
- **`$YIDAM_GRAPH`** carries the corpus's classes, nodes, properties and links. It carries no
  catalog, so a calculator cannot learn a table's declaration from it.
- **The kit** has no table reader. Each calculator parses its own TSV.
- **`.yidam/computed/`** reads only `*.yml` signal tables directly under it. A calculator may
  write a TSV there under `writes:`, but nothing reads it back.
- **A link to a catalog entry** cites the whole entry. RFC-0032's reference grammar parses a
  catalog fragment but gives it no meaning.

## Proposal

### 2.1 The declaration

A catalog entry declares a table when all of these hold:

- its `type:` is `dataset`;
- exactly one of its locations is `kind: file`, naming a tracked file inside the repository;
- the file ends `.tsv` or `.csv`;
- it carries `columns:`.

`columns:` is a list in RFC-0050's `fields:` grammar. Each entry has a `name`, a `type`, and
optionally `required`, `unit` and `values`. Four types are accepted: `string`, `number`, `date`
and `ref`. A `ref` cell names a node, in RFC-0032's reference grammar. `values:` closes a
`string` column to a list. `text`, `claim`, `quotation` and `list` are refused, because a cell
is one line and holds no standing of its own.

`key:` is optional. It names one column, or a list of them, whose values together are unique
across rows. Every key column is implicitly `required`. Key values are compared as the raw text
of their cells, and a row is cited by that text, so `3` and `3.0` are two keys whatever their
column's type.

**Read only on a dataset.** `columns:` and `key:` are read from an entry whose `type:` is
`dataset` and from no other file. `Frontmatter` is shared by every kind of markdown file, and a
derived corpus may already use either word for its own purposes. So both are typed as
`serde_yaml::Value`, as `status:` is. A value of another shape then cannot fail the rest of the
header, and the keys are interpreted only on the entries this RFC defines.

The first line of the file is its header. The header must list the declared columns in the
declared order. An extra column is a finding, so a column added by an extract is noticed and
declared.

**Why the catalog.** A table here is evidence the corpus draws on, which is what a catalog entry
is. An entry already has a name that nodes link to, a description, a `file` location and an
`obtained:` flag. RFC-0051 gave the `file` location its pins. A table needs only its columns
added. §4.2 gives the alternative of a new directory.

**Why no stored digest.** The issue asks for a digest. A digest stored in the entry would need a
commit to the entry each time the file changes, and ohio-campaign-finance-2025-2026 refreshes
its extracts in commits of their own. The receipt already records the sha256 of every file a
calculator read. The digest is derived where it is used, and the entry declares only what does
not change on refresh.

### 2.2 What `lint` checks

Three checks are added. Each reads the file from the working tree, as the other checks do, and
asks git only whether the file is tracked.

- **`table-declaration-invalid`** (Error), on a `dataset` entry that carries `columns:`. It
  reports a `columns:` that does not parse, a refused type, a duplicate column name, a `key:`
  naming no declared column, no `file` location or more than one, a file that is missing or
  untracked, and a file that does not end `.tsv` or `.csv`. So an entry that carries `columns:`
  either declares a table or is reported. A malformed declaration is not used to check the
  file.
- **`table-shape`** (Error), on the table file. It reports a header that does not match, a row
  with the wrong number of cells, a missing required cell, a cell that does not parse as its
  type, a value outside `values:`, and a repeated key. It names the file, the line and the
  column. After 20 findings in one file it reports a count instead of each one.
- **`table-ref-unresolved`** (Warn), on the table file. It reports a `ref` cell naming a node
  the corpus does not hold. It is a warning because an extract may run ahead of the corpus, and
  the cell is still a fact about the source.

The cell grammar is the property grammar applied to text. A property's number must be an
unquoted YAML number, and every cell is text. So a `number` cell is read by `numeric_value`,
the parser a property's number and `query`'s ordering already share. A `date` cell takes the
precisions a property's date does. The empty cell is the absence of a value.
TSV is read as plain tab-separated text with no quoting, and a tab or newline in a cell is
a shape finding. CSV is read as RFC 4180. One reader serves `lint`, `run` and the row
citation, so a file `lint` passes is a file the other two read the same way.

**Cost.** ohio-campaign-finance-2025-2026's 17 files hold about 181,000 rows. Reading them is a
single pass with no allocation per cell beyond the key set. The implementation is measured
against that corpus, and the PR states the time `lint` gains.

### 2.3 What `yidam run` checks

Before a capability runs, every declared table its `reads:` cover is checked as `lint` checks
it. A table that fails stops the run before the calculator starts, naming the table and the
first findings. A calculator should not compute from a file its own corpus would reject.

After the calculator exits, every declared table its `writes:` cover is checked the same way.
A written table that fails fails the run, and nothing is committed. The `extract:` and
`refresh:` commits that wrote ohio-campaign-finance-2025-2026's tables are this case.

A table that is read but not declared is read as it is today. The check is earned by declaring,
not imposed on every TSV.

### 2.4 The kit's table reader

`yidam run` writes the declarations of the tables a capability reads to one JSON file, and
names it in `$YIDAM_TABLES`. Each entry carries the path, the catalog name, the key and the
columns. The file is written beside `$YIDAM_GRAPH`, and the receipt hashes it with the other
inputs.

The kit gains `table(path)` in Python and Rust. It reads the file with the same grammar `lint`
uses and returns rows keyed by column name. A `number` cell is a float and an empty cell is
`None`. Every other cell is a string. A path with no declaration in `$YIDAM_TABLES` is refused
with the reason, so a calculator does not silently read an undeclared file as typed.

The gluon arm is unchanged. Its `Corpus` value carries no tables, and adding them is §6's third
question.

### 2.5 Citing a row

A node cites one row by a markdown link to the entry with a `#row=` fragment:

```markdown
[the largest receipt](../../catalog/sos-receipts.md#row=OH-2025-88412,3)
```

The fragment holds the key's values in `key:` order, joined by `,`, each percent-encoded. A `,`
inside a value is written `%2C`, though common encoders leave it bare. The fragment is split on
`,` before any value is decoded. A
link with a `#row=` fragment is a citation of the entry, for `catalog-unobtained-but-cited` and
the other checks that ask which nodes cite an entry. `broken-prose-link` resolves it further:

- a `#row=` on an entry that declares no table, or no `key:`, is reported;
- a key with the wrong number of values is reported;
- a key no row holds is reported.

All three are Error, under `broken-prose-link`, because a row link that does not resolve is a
broken link. No new check is added for them.

Only a markdown link carries the fragment. An edge's `source:` and a quotation's `of:` name an
entry by stem or path, and an old binary reads a fragment there as part of the path. §6's first
question takes them up.

**Aggregates are not cited.** A total, a share or a median is computed. A calculator that reads
the table writes it as a signal, and prose quotes it through RFC-0058's block. `regen --check`
then holds the prose to the signal the last run wrote, not to the table. A table refreshed since
that run is one of the capability's inputs, so `doctor` reports the capability stale until it
runs again. §1.2's two-vote discrepancy is the case that path catches,
and a citation grammar for aggregates would be a query language with no calculator behind it.

### 2.6 The prelude

`prelude/guidelines/directories.md` gains a paragraph after its catalog rules. It says that a
tabular file a corpus draws on is declared by a `dataset` entry, that records alike enough to
template are rows, not nodes, and that a figure over a table reaches prose through a signal. The
read-route word ceilings move by the paragraph's word count.

### 2.7 Tests

- A declaration with each refused type, a duplicate column, a `key:` naming no column, an
  untracked file, two `file` locations and a `.xlsx` file each report
  `table-declaration-invalid`, and the file is not checked.
- A catalog entry, a skill and an agent that carry `key: primary` or `columns: 3` outside a
  `dataset` entry keep every other field of their header, and report nothing.
- A header out of order, a short row, a missing required cell, a non-number in a `number`
  column, a value outside `values:` and a repeated composite key each report `table-shape`
  with file, line and column. A file with 25 bad rows reports 20 and a count. A `number` cell
  reading `24` passes. Keys `3` and `3.0` are two keys.
- A `ref` cell naming no node reports `table-ref-unresolved` at Warn.
- TSV and CSV read the same rows from the same data, and a quoted CSV cell holding a comma is
  one cell.
- `yidam run` refuses to start on a declared read that fails, and commits nothing when a
  declared write fails. An undeclared TSV is read as before.
- The kit's `table()` returns typed rows in both languages, and refuses an undeclared path.
- A `#row=` link resolves, and the wrong arity, an unknown key, and a table with no `key:` each
  report `broken-prose-link`. The link counts as a citation of the entry. A key value holding
  a comma resolves when written `%2C`.
- The registration gates for three new checks pass.

## What this does not touch

- **The warehouse.** A declared table is not exported by RFC-0049's contract. Whether it should
  be is §6's second question.
- **Tables too large to track.** A table lives in git, as the coverage clock's TSV does. Bytes
  that belong in the vault stay there, and a calculator still cannot read them.
- **Records written as nodes.** No class is converted, and no command converts one. §1.3's
  classes are evidence for the prelude paragraph, not a migration.
- **Markdown tables in prose.** They stay prose. A corpus that moves one into a file does so by
  hand.
- **`.yidam/computed/`.** A computed TSV is not read back. A calculator that produces a table
  for others to read writes it to a tracked path a `dataset` entry declares.
- **The MCP surface.** `serve` does not expose rows.

## Migration & compatibility

**An old binary reading a new corpus.** `Frontmatter` does not deny unknown fields, so
`columns:` and `key:` are ignored. An old `lint` splits a link at `#` and drops a fragment that
is not a `#L` line range as a heading anchor. A `#row=` link still resolves to the entry, and
still counts as citing it. An old `yidam run` sets no `$YIDAM_TABLES`, and the kit's `table()`
refuses with a message naming the variable. A calculator written against the new kit fails
loudly on an old pin. Nothing is stranded silently, so the bump is minor.

**A new binary reading an old corpus.** Both keys are read only on a `dataset` entry, and are
typed so that a value of another shape cannot fail the header (§2.1). A `dataset` entry that
already carries `columns:` for another purpose is reported by `table-declaration-invalid`. That
is the one finding an old corpus can gain, and the implementing PR counts such entries across
the local corpora before it merges. Otherwise no check fires and no run is checked until a
corpus declares.

**Adopting it.** ohio-campaign-finance-2025-2026 writes 17 entries, one per tabular file. The
first `lint` reports what the files already hold. §1.2's discrepancy is found by the first
calculator that writes the total as a signal, once the prose quotes that signal.

## Alternatives considered

### 4.1 RFC-0049's warehouse contract as the declaration

The issue asks whether the warehouse contract can declare a table. Its `ColumnType` has four
members, `String`, `Float`, `Date` and `Timestamp`, derived from `.ont.yml`. Its feeds are SQL
fragments. Both describe a projection out of the corpus, not a file inside it. A `Date` there
is a date and a precision column, and a `ref` has no type of its own. Using it would give a
table a second type vocabulary beside the one properties use. The property grammar was chosen,
so a column means what a property of the same type means.

### 4.2 A `.yidam/tables/` directory

A new top-level kind would give tables a directory of their own. It would need its own
frontmatter, its own lint registrations and its own link resolution, all copies of the
catalog's. RFC-0051 declined a registry class for the same reason. A table here is evidence,
and evidence has a home.

### 4.3 A stored digest in the entry

`artifacts:` already holds a sha256 per artifact. Requiring one for a table would add a
bookkeeping commit to every refresh, and would duplicate what the receipt records. A stale
digest would also be a new finding with no meaning beyond "someone forgot". Declined in §2.1.

### 4.4 A row as a node

The present practice. It gives every row a file, a commit history and a place for prose. It also
gives 1,562 bills 1,562 files of templated text, which `embed` indexes and `lint` reads as
argument. A row worth an argument keeps a node, and the node can cite its row.

### 4.5 Aggregate citations, such as `#sum=amount`

A citation that computes would put a query language in a link. It would have no receipt, no
capability and no place to state its method. A signal has all three.

## Expected effect, and the falsifier

**Expected.** ohio-campaign-finance-2025-2026 declares its 17 tabular files, and `lint` reports
the defects they hold. At least one corpus without capabilities today declares a table and
cites a row from prose. A record class in §1.3 is moved to a table by its corpus's own decision.

**The falsifier.** Ninety days after the release, count `dataset` entries with `columns:`
across the local corpora, and `#row=` links in node prose. If ohio-campaign-finance-2025-2026
is the only corpus with a declaration, and no node anywhere cites a row, the declaration served
one calculator and the citation served nobody. Then the citation is removed in the next major,
and the declaration is kept only if its checks found a defect.

## Open questions

1. **A row as an edge's source.** An edge resting on one row would write `source:
   sos-receipts#row=…`. An old binary reads that as a path and reports it unresolved, so it
   needs a major bump or a separate key. Is the edge case common enough to pay for?
2. **Exporting tables.** Should RFC-0049's contract export a declared table, typed by its
   columns, beside the corpus tables it exports now?
3. **The gluon arm.** Should gluon's `Corpus` carry declared tables, or is a table a Python and
   Rust calculator's input only?
4. **A size ceiling.** watermark-directory tracks 60.9 MB of tabular files. Should `lint` report
   a declared table above some size, and point it at the vault?
