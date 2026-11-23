# RFC-0058 — A signal quoted in prose is a block that `regen` refreshes

- **Status:** Draft
- **Track:** I41
- **Relates to:**
  - #1503 (computed figures pasted into prose go stale unchecked; the evidence this answers), parent epic #1513
  - RFC-0043 (the inline REGEN block and the `count` generator, whose shape this reuses)
  - RFC-0054 §2.2 (`yidam run` folds its REGEN refresh into the `compute:` commit)
  - #1028 (`.yidam/computed/` read back as signal tables, the reader this cites through)
- **Versioning layers touched:** CLI and prelude. `regen` gains one parameterised generator, `signal`, beside `count`. REGEN blocks are read in node files under `.yidam/corpus/` as well as in tracked markdown. `malformed-regen-block`, `unclaimed` and the two parameterised generators read node files too. `embed` and `export llms` read node prose with REGEN markers removed and bodies kept. `prelude/guidelines/directories.md` gains one paragraph. The REGEN marker format is unchanged, so the template bump is minor. No report schema, MCP contract or SDK change.
- **Downstream reference cases:** measured read-only on 2026-11-23 at `origin/main` of every derived repository on the measuring machine. That is 21 repositories, 20 of them with a corpus. The figures come from traffic-design (8b1529df), ohio-campaign-finance-2025-2026 (e91e01c), land-use-activity (8200c8d), travel-demand (2699f7a), civic-infrastructure (f680bfe) and city-engine (c5236c66). allen-county-ohio-democratic-party and cellular-automata are not checked out there. The issue's figures for them are cited, not re-measured.

## Summary

A calculator writes its answer to `.yidam/computed/`, and then a person copies the number into a
node's prose. Nothing ties the copy to the file. When the calculator runs again, the copy
stays as it was.

The issue proposes a citation form that `lint` resolves and checks for staleness. Most of that
already exists. RFC-0043 built an inline REGEN block that `regen --check` holds to its
generator, and the scaffold's CI runs `regen --check`. RFC-0054 made `yidam run`
refresh stale blocks in the same commit as the recompute. What is missing is a generator that
reads a computed value, and a scan that reaches node prose.

This RFC adds both. A node quotes its own signal as

```markdown
The platoon is stable for τ up to <!-- REGEN: yidam signal idm_ring_delay_platoon_stable_max_s -->0.75<!-- /REGEN --> s.
```

`regen --check` fails when the block no longer matches the file. `yidam run` rewrites the block
in the commit that changed the file. Prose elsewhere names the node as well:
`yidam signal finding/idm-reaction-delay-ring idm_ring_delay_platoon_stable_max_s`.

## Problem

### 1.1 What was measured

Two scripts read each repository's `origin/HEAD` through git, never a working tree. One read
the tree as it stands. The other read the history of `.yidam/computed/`, `.yidam/corpus/` and
every authored `.md` file, oldest commit first.

| Measure | Count |
|---|---|
| Corpora with `.yidam/computed/` | 8 of 20 |
| Computed files | 41 |
| Of which signal tables (`format_version` and `signals:`) | 17, in two corpora |
| Signal table rows | 2,470 |
| REGEN `count` blocks in any corpus | 0 |
| REGEN blocks in any node file | 0 |
| Nodes naming a `.yidam/computed/` path in prose | 16, in five corpora |

### 1.2 Values that travel from a computed file into prose

A value is *carried* when it first appeared in a computed file and later appeared in prose. Only
number tokens with at least four significant digits count, and years are excluded. A value
rounded on its way into prose is not counted, so this is a floor.

| Corpus | Carried | Still in prose | In files | Of them nodes |
|---|---|---|---|---|
| ohio-campaign-finance-2025-2026 | 185 | 182 | 52 | 49 |
| traffic-design | 55 | 51 | 47 | 18 |
| land-use-activity | 14 | 12 | 12 | 8 |
| travel-demand | 3 | 1 | 1 | 0 |
| civic-infrastructure | 2 | 0 | 0 | 0 |
| city-engine | 3 | 0 | 0 | 0 |

Every node line holding a carried value is in a YAML block scalar (`|` or `>`). None is in a
plain or quoted scalar.

### 1.3 A signal is mostly quoted where its row is

A signal table row is keyed by `node:`. The second measure asked whether the node a row is
about quotes the row's values, spelled exactly as the file spells them.

| Corpus | Numeric signals | Quoted in their own node | Nodes |
|---|---|---|---|
| traffic-design | 208 | 119 | 26 |
| ohio-campaign-finance-2025-2026 | 14,111 | 284 | 144 |

So 403 signal values sit verbatim in the prose of the node they describe. traffic-design quotes
57% of its signals that way. Its nodes also say where the figure came from, in words: "the
delay block of `.yidam/computed/idm-ring.yml`". The citation the issue asks for is already
being written by hand, as prose.

### 1.4 Where the drift is

Locally, the copies have not drifted yet. Of the 246 carried values still in prose, none is
absent from today's computed files. In the history, exactly one commit replaced a carried value
in prose with another. That was ohio-campaign-finance's f371606: "revise: OHROC is paid $8.5M by
70 members, not $5.4M by 67". traffic-design's computed files changed in 56 commits, but those
changes added values and did not move the ones already quoted.

The drift is in the corpus the issue describes. allen-county-ohio-democratic-party quotes K4–K8
ranks and figures in 29 nodes, 17 of them as markdown tables. Each recompute is followed by a
revise sweep, such as "23 nodes quote the K4–K8 figures the 2026 general moved". The issue
counts 18 such subjects. The local corpora show the paste at scale. That corpus shows what the
paste costs when the inputs move.

### 1.5 Why the existing machinery does not reach it

RFC-0043's `count` block is the right shape. A figure sits mid-sentence, the generator writes
it and `regen --check` gates it. Three things stop a corpus using it for a computed value.

- **No generator reads `.yidam/computed/`.** `count` answers a query over nodes. A signal is not
  a node property, by `directories.md`'s own rule, so no query reaches it.
- **REGEN is read only in markdown.** `count` and `unclaimed` both skip any file that does not
  end in `.md`. The 246 carried values are mostly in node files, where a block would be ignored.
- **A block in a node would leak.** Node prose goes into embedding text and the llms export as
  written. A marker there would be indexed as if it were prose.

## Proposal

### 2.1 The block

```markdown
<!-- REGEN: yidam signal <name> -->0.75<!-- /REGEN -->
<!-- REGEN: yidam signal <node-ref> <name> -->0.75<!-- /REGEN -->
```

The first form is valid only in a node file, and it cites that node's own row. The second form
is valid anywhere REGEN is read. `<node-ref>` is a reference in RFC-0032's grammar, read by
`parse_reference`, so `finding/idm-reaction-delay-ring` and the full `yidam://` form both work.

The value is resolved through `Signals::load`, the reader `embed` already uses. Its rules come
with it, unchanged:

- a signal name is corpus-wide, and two files declaring one name are refused;
- a row naming a node the corpus does not hold is reported;
- a file whose `format_version` this binary does not know is reported and not read.

**Why a signal name, and not a path into a file.** A path addresses the file's layout, such as
`delay.platoon[2].stable_max_s`. A calculator that reorders a list moves every such path
without changing a value. A signal name is declared by the calculator, collision-checked across
the corpus, and already the key `embed` indexes by. §4.1 gives the alternative in full.

### 2.2 What the block holds

The body is the value, written as follows.

- An integer is written in decimal.
- A float is written the way `serde_json` writes it, which is the shortest form that reads back
  to the same number: `0.75`, `7.0`, `569.5`.
- A string is written as it is.
- A boolean is written as `true` or `false`.
- A null, list or map is refused. None of them is a figure.

Three modifiers may follow the name, in any order:

- `places=N` writes a number with exactly N decimals;
- `percent` multiplies by 100 and appends `%`;
- `grouped` separates thousands in the integer part with `,`.

So `yidam signal share_of_receipts places=1 percent` writes `18.2%` for `0.18234`.

A modifier on a value that is not a number is refused. RFC-0043 left grouping open for `count`.
This RFC gives `signal` the modifiers because a computed float rarely has the precision prose
wants. Whether `count` takes the same three is §6's first question.

### 2.3 Where REGEN is read

The tracked set REGEN is read over gains node files: tracked `*.yml` under `.yidam/corpus/`.
Ontology files are excluded. Only inline blocks are read there. A block in a node has its
open tag, body and close tag on one line.

Four readers widen together, so that none of them sees a block another misses:

- `count` and `signal` refresh blocks there;
- `unclaimed` reports a block there whose command no generator writes;
- `malformed-regen-block` reports a broken block there.

The non-parameterised generators are unchanged. Each writes a named block in a named document,
and none of those documents is a node.

**Writing into YAML.** A block is valid YAML text only inside a block scalar or a quoted scalar.
In a plain scalar, `REGEN: ` is a mapping indicator and the node fails to parse, which `lint`
already reports. In a quoted scalar a value holding `"`, `'` or `\` would change the scalar's
meaning. So a value is written into a node file only when it is one line and holds none of
those three characters. Every number qualifies. A string that does not is refused, with the
file and the reason. §1.2 measured every carried value in a block scalar, where only the
newline matters.

### 2.4 Staleness, resolution and the recompute

**Staleness is `regen --check`.** A block whose body differs from what the generator would write
is stale, and `regen --check` names it with the remedy it gives every other block. The scaffold's
CI already runs that command. A stale signal fails CI the way a stale count does.

**A block that does not resolve fails the run.** An unknown name, a node with no row, or a
refused file fails the same way `count` fails on a query that does not typecheck. The run names
the file, the block and the reason, then exits 1. It visits every block first. It never writes
a guess, for RFC-0043's reason: a figure that could not be read has not been computed.

**The recompute carries its prose.** RFC-0054 §2.2 has `yidam run` refresh stale blocks into the
`compute:` commit it authors. With `signal` registered, a recompute that moves a value rewrites
every block quoting it, in the same commit. That is the transclusion the issue asked for as an
option. A person who recomputes outside `yidam run` uses `yidam regen --fold`.

### 2.5 Readers that are not markdown

A rendered node hides an HTML comment, so the block shows only its value. Two readers take node
prose as text, and both will strip the markers and keep the body:

- `embed`, so an embedding reads `0.75 s`, not the open tag;
- `export llms`, so the export reads as prose.

The stripping is one function over `parse_markers`. A marker with no matching close tag is left
as it is, for `malformed-regen-block` to report.

### 2.6 Why `lint` gains nothing

The issue puts the check in `lint`. `regen --check` already owns the question, and it has a
remedy: `yidam regen` rewrites the block. A `lint` check for the same staleness would be a
second gate with no remedy of its own. `lint` reads blocks only for their shape, through
`malformed-regen-block`, and §2.3 widens that check to node files.

### 2.7 The prelude

`prelude/guidelines/directories.md` gains a paragraph after its signal-table rules. It says that
prose quoting a signal uses the block, with one example. The read-route word ceilings move by
the paragraph's word count.

### 2.8 Tests

- A node with an own-row block is refreshed. The same block in a markdown file is refused,
  because a markdown file has no row of its own.
- A node-ref block resolves both reference spellings.
- `places`, `percent` and `grouped` each write their documented form. Each is refused on a
  string.
- An unknown name, a node with no row and a colliding name each fail the run. Each failure names
  its block, and the other blocks are still refreshed.
- A string value holding `"` is refused in a node file and written in a markdown file.
- `regen --check` reports a stale block in a node file. `unclaimed` reports `yidam signl x`
  there. `malformed-regen-block` reports a block with no close tag there.
- `yidam run` folds a moved signal into its `compute:` commit, and the node's block holds the
  new value.
- `embed` and `export llms` emit the body without the markers.
- The `PARAMETERISED` and `GENERATORS` guards list `signal`, and
  `every_generator_in_the_crate_is_listed` passes with its call site.

## What this does not touch

**Report files.** 24 of the 41 computed files have no signal table, and nothing here cites into
them. A figure in a report becomes citable when the calculator also writes it to a signal row.
land-use-activity's 12 carried values are that case.

**Tables in prose.** allen-county-ohio-democratic-party quotes figures as markdown tables. A
block per cell works, and a block that writes a whole table is a different generator. That is
§6's second question.

**The REGEN marker format.** Inline blocks have existed since RFC-0043. This RFC only reads them
in more files.

## Migration & compatibility

**A derived repository does nothing.** No document changes, and no generator writes anything new
until a block names `signal`. No node file holds a REGEN block today (§1.1), so widening the scan
changes no output.

**Why the bump is minor.** RFC-0043's bump was major because an old reader destroyed the new
form. Here an old binary does not scan node files, so it ignores a block there. In markdown, an
old binary reports a `signal` block as `unclaimed`, and `regen --check` fails on a stale pin. That
is a pin too old to read a block, said plainly, and it writes nothing.

**Adopting it.** A corpus replaces a quoted value with a block whose body is the same value. The
first `regen --check` passes, because nothing moved. §1.3's 403 own-row quotes are the
candidates. An agent can find them the way the measurement did.

## Alternatives considered

### 4.1 `yidam computed <file>#<path>`

A citation by file and key path reaches report files as well as signal tables. It was declined
for three reasons:

- a path addresses layout, so reordering a list breaks citations without moving a value;
- two calculators can write the same figure under different paths, and nothing would say they
  disagree;
- it gives a report's values a public address that its calculator never declared.

A signal row costs the calculator one line per figure, and the kit's writer already has the
function.

### 4.2 The issue's `[signal:<capability>/<key>#<field>]`

This form needs a second scanner, a second staleness gate and a second refresh path, beside the
three REGEN already has. It also names the capability, which `Signals` already resolves, and it
carries no value. Checking a quoted value for staleness then needs the value written beside it,
in a form the scanner must also parse.

### 4.3 A `lint` check that diffs prose against computed files

That is what §1.2's script does, and it can only guess. A number in prose matching a number in
a file is a coincidence as often as a quote. A declared citation needs no guess.

## Expected effect, and the falsifier

A corpus that adopts the block stops writing revise sweeps after a recompute. The `compute:`
commit carries every moved value, and `regen --check` fails on any copy left unquoted by a
block.

**The falsifier is allen-county-ohio-democratic-party's next recompute.** Its K4–K8 figures in
the 29 nodes, moved into blocks, should change in the `compute:` commit, and no revise commit
should follow for those figures. If its tables cannot be written cell by cell, or its figures
are not signals but report values, this RFC answered the local corpora and not the one the
issue describes.

## Open questions

1. **Modifiers on `count`.** `places`, `percent` and `grouped` are written for `signal`.
   RFC-0043's open question asks the same of `count`, with `roadmap.rs`'s `grouped()` as
   evidence. Sharing one modifier parser is cheap. Whether `,` is the author's separator or a
   corpus declaration is still unanswered.
2. **A block that writes a table.** A row per node and a column per signal is the shape
   allen-county-ohio-democratic-party writes by hand 17 times. It would be a block-form
   generator, and it would need a way to name the rows.
3. **A proposer.** An opt-in Info check could flag a node quoting its own signal verbatim, as
   `refusal-undeclared` flags a refusal. §1.3 says it would find 403 candidates in two corpora.
   It is left out until a corpus asks for it.
