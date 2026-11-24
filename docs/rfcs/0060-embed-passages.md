# RFC-0060 — A node is embedded as passages, and scored by its best one

- **Status:** Implemented
- **Track:** I43
- **Relates to:**
  - #1505 (a node grew to 209,000 characters and nothing warned; the evidence this answers),
    parent epic #1513
  - RFC-0007 (the index layer, and the one-vector-per-node shape this replaces)
  - RFC-0033 (the remote vector index, whose key scheme and 40 KB metadata ceiling §2.4 changes)
  - RFC-0030 (the embed config witness, which gains the splitter's parameters)
- **Versioning layers touched:**
  - **SDKs.** `yidam-core` gains `split_passages`, a parity function with a shared fixture in
    Rust, Python and TypeScript.
  - **The local index.** `corpus.arrow` gains a `passage` column, and `meta.json` gains
    `row_count`.
  - **The witness.** `embed.config.json` gains a `passages` key, and its `format_version`
    becomes `"2"`.
  - **The remote index.** A remote key gains a `#<n>` suffix on every passage after the first.
  - **The MCP contract.** A `retrieve` hit gains `passage`. The contract change is additive.
  - **Lint.** `lint` gains one check.
  - **The prelude.** `prelude/guidelines/directories.md` gains one paragraph.
  - **The bump.** It is minor for the CLI, with an upgrade note (§"Migration").
- **Downstream reference cases:** measured read-only on 2026-11-23 at the checked-out head of
  every derived repository on the measuring machine.
  - The sizes come from file sizes on disk across 18 corpora.
  - The composed text comes from `yidam embed --dry-run`, which writes nothing, on six of them:
    - allen-county-ohio-affordability-2026;
    - ohio-campaign-finance-2025-2026;
    - allen-county-ohio;
    - ohio-136th-assembly-hb-96;
    - ohio-democratic-party-rural-caucus;
    - traffic-design and land-use-activity.

## Summary

`yidam embed` composes one text per node. `yidam index-build` turns that text into one vector.
The model is `all-MiniLM-L6-v2`, and its tokenizer stops at 512 tokens. It stops silently.
Whatever a node says after its first couple of thousand characters never reaches the vector.

That is the median node, not an outlier. On allen-county-ohio-affordability-2026, the composed
text has a median of 2,330 characters and a p90 of 5,101. The vector of a typical node
therefore describes its first paragraph or two. A node that grows past that point grows nothing
that a semantic query can find. The issue's 209,000-character node is the extreme case of a
condition most nodes are already in.

This RFC embeds a node as passages:

- **The split.** `split_passages` cuts a node's composed text into pieces of at most 1,200
  characters. It cuts at line boundaries where it can. Each piece leads with the node's label.
- **The index.** Each passage becomes one row and one vector.
- **The score.** A query scores a node by its best passage, and a reader returns the node once.

A lint check, `node-too-many-passages`, warns only on the outliers. Those are the handful of
nodes that have grown past any size a node should have, either because working artifacts piled
up on an entity or because one account kept absorbing claims of its own.

## Problem

### 1.1 What was measured

**The window.** fastembed's text-embedding pipeline truncates at `DEFAULT_MAX_LENGTH = 512`
tokens. `yidam/cli/src/embedding.rs` sets no other length. WordPiece over this corpus's prose
averages roughly three to four characters a token, because citations, figures and bill numbers
split finely. So the window is about 1,500 to 2,000 characters.

**What the issue calls the window is a different limit.** The ~36,000-character figure in #1505
is `MAX_METADATA_BYTES`, the 40 KB ceiling on the text S3 Vectors stores beside a remote vector
(RFC-0033 §"Metadata"). That limit concerns what a remote hit can show, not what its vector
saw.

**Nodes whose file is over 2 KB, by corpus.** The file size overstates the composed text,
because the composed text drops keys that are not prose. It is still the right order of
magnitude:

| Corpus | Nodes | Over 2 KB | Over 36 KB |
|---|---:|---:|---:|
| allen-county-ohio-affordability-2026 | 1,600 | 90% | 7 |
| ohio-education-funding | — | 97% | 0 |
| WRDA-2026 | — | 96% | 0 |
| matt-huffman | — | 95% | 1 |
| ohio-democratic-party-rural-caucus | 634 | 86% | 2 |
| allen-county-ohio | 724 | 76% | 0 |
| city-engine | 355 | 75% | 2 |
| urban-land-markets | — | 64% | 0 |
| traffic-design | — | 60% | 0 |
| ohio-campaign-finance-2025-2026 | 1,067 | 47% | 0 |
| civic-infrastructure | — | 41% | 0 |
| travel-demand | — | 38% | 0 |
| legislation-tracker | 2,103 | 32% | 0 |
| urban-environment-health | — | 24% | 0 |
| ohio-136th-assembly-hb-96 | 1,736 | 21% | 2 |
| land-use-activity | 1,062 | 12% | 0 |
| ml-models | — | 5% | 0 |
| winter-warmer | — | 4% | 0 |

**The composed text, from `embed --dry-run`.** All sizes are in characters, and they are the
text the model is handed:

| Corpus | Node rows | p50 | p90 | p99 | Max | Cut for remote |
|---|---:|---:|---:|---:|---:|---:|
| allen-county-ohio-affordability-2026 | 1,599 | 2,330 | 5,101 | 18,711 | 143,908 | 4 |
| ohio-democratic-party-rural-caucus | 633 | 2,683 | 11,593 | 24,506 | 63,453 | 4 |
| allen-county-ohio | 724 | 3,498 | 6,914 | 10,999 | 21,076 | 0 |
| traffic-design | 744 | 2,216 | 5,339 | 8,562 | 13,623 | 0 |
| ohio-campaign-finance-2025-2026 | 1,067 | 1,411 | 3,483 | 7,143 | 16,141 | 0 |
| land-use-activity | 1,062 | 757 | 1,795 | 5,340 | 10,783 | 0 |
| ohio-136th-assembly-hb-96 | 1,735 | 708 | 3,761 | 10,609 | 130,352 | 1 |

Catalog sources are composed and embedded too. On rural-caucus their p50 is 4,178 characters
and their max is 90,156.

### 1.2 The window is invisible from both sides

- **The author cannot see it.** No command reports it. The dry-run measures a row against the
  remote ceiling, which is twenty times larger.
- **The reader cannot see it either.** A `retrieve` hit carries the node's whole composed text,
  so the text a reader sees is not the text that was scored. A query that matches paragraph six
  of a node finds that node only if paragraph one happens to be near it too.

### 1.3 What keyword search already sees

`retrieval/mod.rs` falls back to BM25 when there is no index, a space mismatch or no backend.
BM25 reads a node's whole text, so keyword recall is not truncated. The loss is specific to the
vector arm, which is the arm `retrieve` uses whenever an index exists.

### 1.4 Where the outliers come from

The four largest files are:

| Corpus | Node | Size |
|---|---|---:|
| affordability | `mechanism/appraisal-gap-blocks-rehab.yml` | 240,578 |
| city-engine | `engine-choice/meso-walker-holds.yml` | 142,087 |
| hb-96 | `bill-version/as-reported-by-conference-committee.yml` | 139,296 |
| affordability | `mechanism/tax-reduction-factors-shift-burden.yml` | 91,629 |

They are not what #1505 describes. Its 209,000-character node was a central-committee entity
in allen-county-ohio-democratic-party, which is not checked out on the measuring machine. That
node had accumulated plans. The corpus split them out into a `plan` class of its own, and its
decision `plan-class.yml` records why. The four above are arguments that kept growing:

- appraisal-gap holds an `account`, a `counter_account` and a `not_examined` property, which
  together run to some 2,700 lines;
- tax-reduction-factors has the same three properties, which run to some 1,090 lines, about
  810 of them its `account`;
- meso-walker-holds holds a 1,500-line description of seeded measurements;
- the conference report holds a description and an `examined_not_used` record that cover 58
  sections.

Both shapes reach the same place: one node whose vector has to stand for several things it
says. #1505's third bullet asks whether `node-too-long` sees them. It does see `.yml` nodes,
but it measures declared prose lines against a class's `max_lines`, and it does nothing for a
class that declares none. None of these classes declares one.

### 1.5 Why a warning alone does not answer it

A warning at the true window would fire on 50–95% of most corpora's nodes. That is not a
finding, and it would be disabled. A warning at the remote ceiling would fire on about nine
nodes, on three of the seven corpora measured, and would leave the median node's second half unindexed. The
defect is in how the index reads a node, not in how authors write one.

## Proposal

### 2.1 The splitter

`yidam_core::embed::split_passages(label, text, max_chars) -> Vec<String>` is a parity function
beside `compose_embed_text`. It takes the composed text unchanged and works as follows:

1. **No label.** `compose_embed_text` leaves the label out when a node has none or it is
   empty. The splitter is then called with `""`, each passage is its lines alone with no
   leading space, and the steps below read "the label and its space" as nothing.
2. **Short text.** If the whole text is at most `max_chars` characters, it returns the text as
   the only passage. A node under the budget composes exactly as it does today, and its vector
   is unchanged.
3. **The body.** Otherwise it takes the body: the text with its leading `label` and the
   separating space removed. `compose_embed_text` puts the label first.
4. **Packing.** It splits the body on `\n` and packs whole lines greedily into passages. Each
   passage is `"{label} {lines joined by \n}"`, and it stays at most `max_chars`, label
   included.
5. **A long line.** A line that alone exceeds the room left after the label is cut at the last
   whitespace before that room. If there is no whitespace, it is cut at the character boundary.
   The remainder carries on into the next passage.
6. **Related.** `compose_embed_text` joins its parts with a space, not a newline, so the
   `Related: …` sentence ends the body's last line rather than starting a line of its own. The
   splitter first cuts the body at its last occurrence of `" Related: "` and treats that
   sentence, without the leading space, as a line of its own, so it packs whole like any other
   line. It finds the sentence only when the node has links; a node without them has no such
   cut.
7. **A long label.** A label that leaves less than half of `max_chars` for the body is cut to
   its first `max_chars / 2` characters, at the last whitespace before that point, in every
   passage of that node. A passage therefore always has room for body text, and the packing
   always makes progress.

Every passage leads with the label, so a passage cut from paragraph six still says which node
it is part of. "Characters" means Unicode scalar values in all three SDKs. TypeScript counts by
code point, not by UTF-16 unit, and a fixture holds a non-BMP character to make the three
agree.

**The budget is 1,200 characters.** At three characters a token, the densest ratio measured
above, that is 400 tokens. It leaves room for the label and the tokenizer's special tokens
inside 512. The implementing PR measures the budget with the model's own tokenizer over every
passage of the seven corpora in §1.1. It lowers the constant if any passage exceeds 512 tokens,
and it reports the figure in the PR.

**No overlap and no sentence splitting.** Lines are the boundaries `compose_embed_text` already
emits. A field's block scalar keeps its own line breaks. Sentence detection would be the one
part of the splitter that differs by language and library, which is the hardest thing to hold
to parity. Overlap is §6's first question.

### 2.2 The index

- **Rows.** `yidam embed` keeps writing one record per node, with the whole composed text.
  `index-build` splits each record and writes one row per passage.
- **Columns.** `COLUMNS` becomes `[path, class, label, passage, text, properties, vector]`.
  `passage` is a non-null `UInt32` ordinal from 0, and `text` is that passage's text.
- **`meta.json`.** `node_count` becomes the number of distinct paths, which is what its readers
  (`status`, `doctor`, `web`, the VS Code tree) already say it is. A new `row_count` holds the
  rows. The reports schema gains `row_count` additively.

**Cost.** The rows grow to at least the composed total divided by the budget:

- affordability goes from 1,599 node rows to at least 4,220;
- campaign-finance goes from 1,067 to at least 1,620.

Those are floors. Greedy packing of whole lines leaves part of a passage empty, and every
passage repeats the label. So the real counts are higher by an amount only the splitter can
tell. The implementing PR runs it over both corpora and reports the counts.

At 384 dimensions a row is 1.5 KB of vector. The largest corpus's `corpus.arrow` grows by at least
3.9 MB of vectors, and `index-build` time grows with the row count. The implementing PR reports both.

### 2.3 Scoring a node by its best passage

Every reader that returns hits groups rows by `path`, keeps each path's best-scoring row, and
cuts to `k` after grouping.

- **Local search** (`retrieval/vector.rs`) already scores every row, so the grouping is exact.
- **Remote search** over-fetches eight times `k` today only when a residual filter is set.
  `top_k_request` asks for exactly `k` rows otherwise, and `k` rows can be fewer than `k`
  nodes. So `top_k_request` over-fetches by `OVERFETCH` on every query against a passage
  index. `wants_more` now counts distinct paths, not rows, and keeps paging until it has `k`
  paths or reaches `MAX_TOP_K`.
- **`retrieve` (MCP), `query --anchor`, `export web`'s `main.js`.** Each returns one entry per
  node. A hit's `text` is the best passage, which is the text that was scored. A hit gains
  `passage`, its ordinal. The `retrieve` contract change is additive.
- **`export sqlite`.** It writes the `passage` column. A `vec0` query over it returns rows, and
  the export's README says to group by `path`.

`Hit.text` now shows the passage that matched, not the node's opening. A reader that wants the
whole node reads it through `find_node`, as it does today.

### 2.4 The remote index

A passage's key is `"{corpus}/{path}"` for passage 0 and `"{corpus}/{path}#{n}"` for passage
`n ≥ 1`. `split_key` and `path_from_key` strip a trailing `#{digits}`.

`vector_key` does not refuse `#` today. From this release it refuses a node path containing
one, naming the path, so a suffix can never be confused with a path. No file under any of the
18 corpora measured has `#` in its name.

`plan_mirror` diffs on keys, as it does now. A node that shrinks deletes its high-numbered
passage keys, and a node that grows puts new ones. A passage carries a new metadata key,
`passage`, with its ordinal.

A passage is at most 1,200 characters, so `text_truncated` never fires for a passage. The
40 KB metadata ceiling stops binding text, and the dry-run's "would have their text cut" line
is replaced by the passage counts.

### 2.5 The witness

`embed.config.json` gains:

```json
"passages": { "splitter": "lines", "max_chars": 1200 }
```

Its `format_version` becomes `"2"`. `space_disagreement` compares `passages` as it compares the
model. An index built with another budget is a different space for this purpose, because the
same query lands on different rows. A reader of this release refuses a `format_version` it does
not know, naming the version. That check is new; no reader today branches on the version.

### 2.6 `node-too-many-passages`

This lint check is a Warning on a node whose composed text splits into more than 32 passages.
That is about 38,000 characters, the order of the remote ceiling the dry-run already reports.
The dry-run counts rows over that ceiling. On the seven corpora of §1.1 that gives 9 rows:

- 4 on affordability;
- 4 on rural-caucus;
- 1 on hb-96.

By file size, city-engine and matt-huffman add up to three more. That is few enough to read
every finding. The implementing PR counts the findings with the check itself. The message names:

- the node and its class;
- its passage count and its character count;
- the advice of §2.7.

The check does not depend on a class declaring `max_lines:`. It answers #1505's third bullet
without changing `node-too-long`, which keeps measuring declared prose lines against a declared
ceiling.

The threshold is a constant. Whether a class may raise it is §6's third question.

### 2.7 The prelude

`directories.md` gains one paragraph after its description of a node.

The paragraph says this. A plan, a brief or a decision log about an entity is a node of its
own, in a class of its own, linking to the entity it concerns. The entity node states what is
true of the entity, and the artifact node states what was done about it. This is the pattern
allen-county-ohio-democratic-party reached with its `plan` class. An account that has grown to
argue several claims is several claim nodes, each linked from the one that cites it. Either
kind of growth makes the outlier `node-too-many-passages` reports.

The paragraph is on a read route, so both `READ_CEILING` entries rise by its exact length.

### 2.8 Tests

- **Parity.** `split_passages` gets fixtures under `yidam/sdks/parity/fixtures/split_passages/`
  and runners in all three SDKs, registered in `mise.toml`'s parity `functions=` line. The
  fixtures cover:
  - a text under the budget, returned whole;
  - lines packed at the exact boundary;
  - a long line cut at whitespace;
  - a long line with no whitespace;
  - a label longer than half the budget, cut;
  - a label longer than the whole budget, cut;
  - a node with no label;
  - a `Related: …` sentence after a full last line;
  - a non-BMP character;
  - an empty body.
- **Index build.** A record of three passages writes three rows with ordinals 0, 1 and 2, and
  the schema test reads the new `COLUMNS`.
- **Search.** Two passages of one node both score above a third node. Search returns the node
  once, with the better passage's text and ordinal, and still returns `k` distinct nodes.
- **Remote.** A query with no residual filter, whose first `k` rows are passages of fewer
  than `k` nodes, still returns `k` nodes. A key with `#3` round-trips to its path. `plan_mirror` deletes passage keys a
  shrinking node no longer has.
- **The witness.** An index with `format_version: "3"` is refused by name. One with another
  `max_chars` is a space mismatch.
- **Lint.** `node-too-many-passages` fires on a node of 33 passages and stays silent on one of
  32. Its eight registrations each fail the build when skipped.
- **The issue's test.** A node whose only mention of a phrase is past character 10,000 is the
  top hit for a query on that phrase. On `origin/main` it is not.

## What this does not touch

- **The model.** `all-MiniLM-L6-v2` stays the default. Changing the vector space of every
  corpus is a separate decision (§4.2).
- **`yidam embed`'s records.** Each node still has one record, with the whole composed text.
  The split happens at index time, so `embeddings/` does not churn.
- **`compose_embed_text`.** It is unchanged, as are its 12 fixtures.
- **BM25.** It keeps scoring whole nodes.
- **`node-too-long`.** Its meaning, its severity and its dependence on `max_lines:` are
  unchanged.
- **Nodes.** No node is split, moved or rewritten. §2.7 is guidance.

## Migration & compatibility

**The index is derived.** Every corpus rebuilds it with `yidam index-build` on the pin that
reads it. A new binary that reads an old local index finds no `passage` column. It reads every
row as passage 0, which is what the row is, so an old index keeps working until it is rebuilt.
`doctor` reports an index whose witness has `format_version: "1"` as stale and names
`index-build`.

**A remote index shared across pins is the one place an old binary misreads.**

- **The keys.** A consumer on an older pin, querying a producer's remote index after the
  producer rebuilds, sees `path#3` keys. `find_node` cannot resolve them, so they become
  `foreign_reference` hits.
- **Duplicates.** Several passages of one node can fill its top `k`.

It does not misread silently, because the producer's witness changes. A consumer on this
release refuses an unknown version. A consumer on an older one compares `passages` as an
unknown key and ignores it, which is the misreading the upgrade note warns about. The upgrade
note in `.changes/upgrading/` says to move both pins together. §6's second question asks
whether that warrants a major bump instead.

**The MCP contract.** `retrieve` gains `passage`, and `text` now means the matched passage. The
contract version bumps once, additively.

## Alternatives considered

### 4.1 A check only

This is #1505's own proposal: warn when a node outgrows the window. At the true window it fires
on most nodes (§1.5). At the remote ceiling it leaves the median node's second half where it is
now, outside the vector. It is kept for the outliers as §2.6, and declined as the whole answer.

### 4.2 A long-context model

Models such as `nomic-embed-text-v1.5` and `bge-m3` read 8,192 tokens. Adopting one would:

- change the vector space of every corpus at once;
- raise the dimension from 384 to 768 for nomic, or to 1,024 for bge-m3;
- slow `index-build` by the square of the window in attention.

It also still averages a node's topics into one point. A 30,000-character node about five
things has a vector near none of them. Passages keep the small model and give each topic its
own vector. The model choice stays open per corpus through `embed.config.json`.

### 4.3 Warn when the tokenizer truncates

`index-build` could count tokens and report truncated rows. That makes the loss visible, and it
is what §2.1's budget check does once, in the implementing PR. As a standing report it would
repeat §1.5's noise on every build.

### 4.4 Mean-pooling a node's passages into one vector

This keeps one row per node and changes no reader. It is the same topic-averaging as §4.2,
reached by a different route. The best-passage score is the reason to split at all.

### 4.5 Splitting by tokens

The splitter would then need the tokenizer in all three SDKs. Its parity would depend on a
vocabulary file, and the model's tokenizer would become part of the contract. A character
budget is checked against the tokenizer once, in §2.1, and holds for this model. A model with a
different tokenizer states its own budget in its witness.

## Expected effect, and the falsifier

**Expected.**

- The issue's test (§2.8) passes on every corpus: a phrase past the window finds its node.
- On allen-county-ohio-affordability-2026, the implementing PR draws 200 nodes over 3,000
  characters. For each one it takes a sentence from past character 2,500 as the query. It then
  measures how often the node is a top-5 hit before and after.
- The "after" rate is expected to exceed the "before" rate by at least 30 points.

**The falsifier.**

- **The measurement.** If that measurement moves the top-5 rate by less than 10 points, the
  passages bought rows and build time and returned nothing that the vector did not already
  find. Then the passage split is reverted in the same release line, and only §2.6 stays.
- **Ninety days after the release.** Count `node-too-many-passages` findings across the local
  corpora. If no finding was resolved by moving an artifact to its own node, §2.7's paragraph
  is not being read. It is then rewritten or removed.

## Open questions

1. **Overlap.** Should a passage repeat the last line of the one before it? Overlap helps a
   phrase that straddles a boundary, and it costs rows. The falsifier's measurement can be run
   with and without one line of overlap before the implementing PR merges.
2. **Major or minor.** §"Migration" names one misreading across pins on a shared remote index.
   If a known consumer reads a remote index on a pin it does not move with the producer, the
   bump should be major.
3. **A class-level threshold.** Should a class be able to raise the threshold of §2.6, the way
   `max_lines:` sets `node-too-long`'s? A class of bill versions might legitimately be long. A
   class key costs a schema change and its registrations, so it waits for a corpus that asks.
