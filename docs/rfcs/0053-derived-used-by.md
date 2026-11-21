# RFC-0053 — `used-by` is derived from the citations, not stored beside them

- **Status:** Implemented
- **Track:** I38
- **Relates to:**
  - #829 (absent versus `used-by: []`, the distinction this RFC makes unnecessary)
  - #172 (three implementations of "does this node cite that source", since reduced to one)
  - #1499 (fold derived updates into the commit that caused them; this RFC removes one kind instead of folding it)
  - #1498 (the evidence this answers), parent epic #1513
- **Versioning layers touched:** template (`guidelines/directories.md` stops describing a hand-kept list) / tooling (one `migrate` operation, one lint check replaced, `catalog-reconcile` retired from the workflow, `propose`'s withdrawal stops editing the catalog). Report contract stays at 1: two `catalog-audit` fields keep their shape and go quiet. No MCP contract change. No SDK code change, but the parity fixture `reports/basic` loses its `used-by` list, so its `catalog-audit`, `catalog-reconcile`, `lint` and `rename` goldens are regenerated (§2.7).
- **Downstream reference cases:** ohio-education-funding, measured read-only on 2026-11-21 at `origin/main`, and allen-county-ohio-democratic-party, whose counts are quoted from #1498 because no clone of it is on the measuring machine.

## Summary

A catalog entry may declare `used-by:`, a list of the corpus nodes that draw on it. The same
fact is already recorded once, as the citations those nodes carry. The list is a copy, and
`catalog-used-by-drift` exists because the copy goes stale. `catalog-reconcile` exists to bring
it back. Each repair is a commit.

In ohio-education-funding, 36 commits are `reconcile: <entry> used-by against the citations`,
19 of them on one day. 35 were written by the tool itself. In allen-county-ohio-democratic-party,
#1498 counts 169 `reconcile:` commits in seven days, 165 of them for `used-by`, touching 475
catalog files.

This RFC stops storing the list. Every reader that wants it already computes it from the
citations, and `catalog-audit` already reports it as `cited_by`. A `yidam migrate used-by`
operation deletes the stored lists once, in one `migrate:` commit. After that there is nothing
to drift and nothing to reconcile.

## Problem

### 1.1 Where the commits come from

A citation is a link, an edge `source:`, or a quotation's `of:` that resolves to a file under
`.yidam/catalog/`. When a knowledge commit adds a node citing three entries, three `used-by`
lists are now wrong. `catalog-reconcile` repairs them one entry at a time, and each repair is
its own `reconcile:` commit, as an operational commit should be.

ohio-education-funding shows the shape directly. Three consecutive reconciles on `origin/main`
each add the same newly written node, `cfap-required-percentage.yml`, to a different entry:
`ohio-revised-code`, `ofcc-equity-list` and `lsc-fcc-greenbook`. One unit of understanding
produced three commits of bookkeeping.

| Corpus | `reconcile:` commits for `used-by` | Shape |
|---|---|---|
| ohio-education-funding | 36 of 1,316 commits, 19 on 2026-10-22 | 74 catalog files touched; 72 of 73 entries declare a list |
| allen-county-ohio-democratic-party | 165 of 169 `reconcile:` commits in one week (#1498) | 475 catalog files touched |

The cost is not only the count. Each commit is a diff a reviewer reads past, in a log whose
value is that every commit says what changed in the understanding. A reconcile changes nothing
in the understanding. The RFC-0052 argument for receipts applies here unchanged.

### 1.2 Why the drift check cannot gate

`catalog-used-by-drift` is Warn, so `yidam lint` exits 0 when it fires.
`catalog-reconcile --dry-run` also exits 0 when it would rewrite something. ohio-education-funding
wrote a `[graph-lint]` guard in `mise.guards.toml` to make the disagreement fail its gate. A
derived repository had to build enforcement for a list that only exists to be enforced.

Raising the check to Error would make every knowledge commit fail until its reconciles landed.
That moves the ceremony earlier without removing it.

### 1.3 Nobody reads the stored list except yidam

A survey of the template on 2026-11-21 found the field read or written in these places, and
nowhere else:

| Where | What it does with the stored list |
|---|---|
| `parse.rs`, `corpus/source.rs`, `corpus/load.rs` | read it into `Source.used_by` |
| `catalog-used-by-drift` | compare it to the citations |
| `catalog-audit` | report it verbatim as `used_by`, beside the computed `cited_by` and `drift` |
| `catalog-reconcile` | rewrite it to the citations |
| `propose` (a `withdraw:` proposal) | delete the withdrawn node from every list, by a line filter |
| `schema` | declare it in the emitted frontmatter schema |
| vscode | show a tooltip line when `drift` is non-empty |

No MCP tool's input or response carries it. No SDK, in Rust, Python or TypeScript, reads it. The
web editor mentions it only in comments. The warehouse derives an entry's citers from link
targets, not from the list. Downstream, ohio-education-funding's web build reads `used-by` into
`Source.usedBy`, only so a test can hold it against the `citedBy` it computes from prose. So the
copy serves no reader that could not compute the original.

### 1.4 The absent-versus-empty distinction is a cost of storing it

#829 found that `used-by: []` was a silent opt-out of the drift check, and the fix made the
empty list a declaration while leaving an absent key unchecked. That distinction is now three
paragraphs of `guidelines/directories.md` and a doc comment on every function that touches
the field. It exists only because a stored copy can be in three states. A derived list has one.

## Proposal

### 2.1 The citations are the only record

An entry does not list its citers. The answer to "what draws on this entry" is the set of
corpus instances whose citations resolve to it, which is what `catalog-audit` already reports
as `cited_by`, and what `catalog-uncited` and `catalog-unobtained-but-cited` already read.

**No new implementation of citation is written.** #172 found three predicates for "does this
node cite that source", and they were reduced to `linked_paths`. Three walkers call it today:
lint's `citations`, over nodes; the audit's `draws_on`, which also reads corpus READMEs and class
definitions and counts them as `elsewhere`; and `serve`, which computes each node's cited entries
for the server. `cited_by` is the instances half of `draws_on`, the same set the drift check
compares against. This RFC reads those sets and adds nothing beside them.

### 2.2 `yidam migrate used-by`

A new `migrate` operation, beside the eleven that exist, `locations` and `pins` among them. It
deletes the `used-by:` block from every catalog entry that declares one, by a line edit, as every
migrate rewrite is. It deletes `used-by: []` too.

It edits no other file in the catalog or the corpus. For each entry, the plan reports what the
deleted list said that the citations do not, from the same `used_by_drift` function the gate uses
today. The plan compares the list with everything `draws_on` finds for the entry, the instances
in `cited_by` and the READMEs and class definitions in `elsewhere`. The gate compared instances
only, because its question was whether the list matched `cited_by`. The plan's question is
whether a citation is missing, and a README that links the entry is not missing one.

- **Claimed but not citing.** The list named a file that carries no citation to the entry.
  Either the list was stale, or the file should cite the entry and does not. The migration
  cannot tell which. It names the file and leaves the decision to the author, who can add the
  link in a knowledge commit.
- **Citing but not claimed.** Nothing is lost. The citation is the record.

The deleted lines are the only thing the migration loses, so it keeps the part of them that
matters. The record every migration writes under `.yidam/migrations/` lists each entry's
"claimed but not citing" names. The `migrate:` commit carries them as a file, so an author who
skips the dry run can still find them.

`--dry-run` prints the plan and writes nothing. The operation is idempotent: run against a
catalog with no lists, it reports nothing to do.

### 2.3 One lint check replaced

`catalog-used-by-drift` retires. In its place, `catalog-used-by-declared` (Warn) reports an
entry that still declares a `used-by:` key, whatever its value. The message names the fix,
`yidam migrate used-by`, as `catalog-location-malformed` names `yidam migrate locations`.

It is Warn and not Error so that upgrading to this release does not fail a derived repository's
gate before it has run the migration. It is a lint check and not a parse failure for the same
reason: the entry's other fields are still read.

A derived repository whose lint baseline records `catalog-used-by-drift` findings will see them
disappear. That is a ratchet moving in the allowed direction.

### 2.4 `catalog-reconcile` retires from the workflow

`catalog-reconcile` is a built-in workflow step today, run after `catalog-fetch` and
`catalog-extract` in every workflow. It leaves `BUILTINS`. The cluster overlay that names it,
`streamflow-on-push`, drops its step and landing.

The command stays for one minor release, because derived guards call it.
ohio-education-funding's `[graph-lint]` runs `catalog-reconcile --dry-run --format json` and
fails when the report's `reconciled` names an entry. In this release the command writes nothing,
with or without `--dry-run`, and exits 0 as it does today. Its `reconciled` is always empty. When
an entry still declares a list, it prints a note naming `yidam migrate used-by`, as
`catalog-used-by-declared` does, and changes neither the exit code nor the report. A guard or
script that calls it keeps passing, whether or not the catalog is migrated. The release after
removes it.

### 2.5 `propose` stops editing the catalog on withdrawal

A `withdraw:` proposal today runs `drop_used_by` over every catalog entry, deleting the
withdrawn node's line from each list. With no lists, the withdrawal touches only the node.
`drop_used_by` and the `catalogs()` feed for it are deleted. The withdrawal's citations go with
the node, so `cited_by` is correct without a write.

`rename` already lists `used-by` references in `unhandled`. That case disappears.

### 2.6 The report contract does not change

`catalog-audit`'s `used_by` is a required array and `drift` is nullable. From this release,
`drift` is null on every row, migrated or not. A drift nothing will repair is not worth
reporting, and the drift check that read it is gone. `used_by` still echoes a list an
unmigrated entry declares; after migration it is `[]` on every row. Both shapes are valid at
report contract 1. A consumer written against contract 1 keeps working, and the vscode tooltip
goes silent in every repository, because `drift` is null.

The schema's descriptions of both fields say they are retained for compatibility: `drift` is
always null, and `used_by` is empty after `migrate used-by`. The next report contract bump removes them. `catalog-reconcile`'s own
report keeps its shape for the release it survives, with `reconciled` always empty.

`cited_by` is the field a consumer reads. Its description already says it is computed and
authoritative.

### 2.7 What the template says

`guidelines/directories.md` replaces its `used-by` bullet with one sentence: an entry does not
list what cites it; `yidam catalog-audit` reports that from the citations. The two paragraphs on
absent versus `[]` go. The example frontmatter at the top of the section loses its `used-by:`
key. `schema` stops declaring the key.

The five example corpora under `examples/` and the two test fixtures that declare a list are
migrated in the implementing PR. The `reports/basic` fixture's `stage-discharge` entry exists to
show drift in both directions; its goldens change to show the entry with no list.

## What this does not touch

- **The citations.** How a node cites an entry, and what counts as a citation, are unchanged.
- **`catalog-uncited` and `catalog-unobtained-but-cited`.** They already read the citations.
- **The warehouse.** It never read the list.
- **#1499's other ceremony.** `regen:` and `scaffold:` commits still follow their causing
  commit. This RFC removes one kind of derived commit by removing the stored value; #1499 folds
  the kinds that have to stay.

## Migration & compatibility

One `migrate:` commit per derived repository: `yidam migrate --dry-run used-by` to read the
plan, then `yidam migrate used-by`. The upgrade note says to read the "claimed but not citing"
names before committing, because each is either a stale entry or a missing citation.

A repository that upgrades and does not migrate keeps working. Its lists are read by nothing
but `catalog-used-by-declared`, which warns, and `catalog-audit`'s `used_by`, which echoes them.
The workflow no longer commits reconciles, and a direct call to `catalog-reconcile` writes
nothing and exits 0 (§2.4).

A repository pinned to an older yidam that reads a migrated catalog sees entries with no
`used-by:` key. Since #829 that is the unchecked state, so the old `catalog-used-by-drift`
reports nothing and the old `catalog-reconcile` leaves the entries alone. Older readers need no
change.

A guard cannot have relied on `catalog-reconcile --dry-run`'s exit code, which is 0 whether or
not it would rewrite (§1.2). ohio-education-funding's guard reads the report's `reconciled`
instead, and failed when it named an entry. After this release `reconciled` is always empty, so
the guard passes, migrated or not. A guard that wants to fail on an unmigrated catalog reads
`catalog-used-by-declared` from `yidam lint --format json`.

## Alternatives considered

**A generated region.** #1498 offers this for consumers that read the YAML directly: keep the
list, but as a REGEN-style region refreshed in the commit that caused it. The survey in §1.3
found no such consumer. A region would also still change every time a citation does, and
without #1499's folding each refresh would be its own `regen:` commit, which is the churn this
RFC removes under another verb. If a reader of the raw YAML turns up, a region can be added
then, generated from `cited_by`, without changing anything this RFC decides.

**Raise `catalog-used-by-drift` to Error.** Discussed in §1.2. It moves the reconcile into every
knowledge commit and keeps a copy whose only job is to agree with its source.

**Fold reconciles into the causing commit (#1499's approach).** It would turn 36 commits into
zero extra commits, but every knowledge commit would still rewrite one catalog file per entry
it cites. The diff a reviewer reads would carry lines that say nothing new.

**Keep the list for an entry's own author to annotate.** An author might want to say which
nodes an entry was obtained for, ahead of any citation. That is intent, not usage, and a list
that mixes the two cannot be checked against either. A node that should cite an entry and does
not is a missing citation, and §2.2's plan names those.

## Open questions

1. **Should `catalog-audit` take an entry argument?** Today it reports every entry. Asking
   "what cites this one" is a filter over its JSON. A positional `[entry]`, as
   `catalog-reconcile` has, would make the derived list a one-word query.
2. **Is one minor release long enough for `catalog-reconcile`?** ohio-education-funding's guard
   is the only caller found, and its owner can change it in the same upgrade.
