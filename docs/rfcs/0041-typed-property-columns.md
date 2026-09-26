# RFC-0041 — Typed property columns, and a predicate on a ranked answer

- **Status:** Draft
- **Track:** G11
- **Relates to:**
  - RFC-0018 (the query surface, whose filter grammar this RFC reuses verbatim rather than restating — `where` is the text inside a step's `[...]`)
  - RFC-0040 (the numeric property type, whose closing open question — *whether `retrieve` should filter on a number* — this is the answer to)
  - RFC-0033 (the remote vector index, which this RFC deliberately leaves without the column, and says why)
- **Versioning layers touched:** MCP contract 0.25.0 → 0.26.0 (`retrieve` gains `where`; `rejected` gains `query`'s predicate codes plus two of its own; `absence` gains `predicate-unsatisfied`). Index format: `corpus.arrow` gains a nullable `properties` column and `meta.json` a `columns` list; an index without the column still decodes. No SDK parity change: no SDK reads the index. No migration file: the repair is a re-embed and a re-index, and the tool says so.
- **Downstream reference case:** the 84 `tenure` nodes of a county-history corpus, each with `began` and an `ended` that is absent while the holding is open. `query 'tenure[began<=1893,ended>?1893]'` answers *who held office in 1893*; before this RFC, nothing could answer *of those, which are most about the courthouse*.

## Summary

The index carries a row's class and text and nothing else a filter can read, so `retrieve`
can narrow by class and by nothing else (#1029). This RFC adds **one column, `properties`**:
a JSON object of the row's declared `date` and `number` values, written by `index-build`
for every property whose declared type has an order. It adds **one argument, `where`**, to
`retrieve`: the query language's filter, verbatim, parsed by the query parser, typechecked by
the query checker and evaluated by the query evaluator, applied before the `k` cut. The two
surfaces then answer one predicate identically, and a case over each of the two typed
fixtures holds them to it.

## Problem

`query` can ask *which tenures were open in 1893* and return every one. `retrieve` can ask
*which nodes are most about the courthouse* and return the five nearest. Neither can ask the
conjunction, and the conjunction is the question an agent has when it knows the period and
not the node: the ranked five *of the open ones*. Today the agent has two bad options. Ask
`retrieve` with a large `k` and filter the answer by reading each node, which spends the
tokens the index exists to save. Or ask `query` and rank its answer by hand, which is a
second ranking beside the one the index already computes.

The index cannot help, because it holds nothing the predicate reads. `index-build` writes
`path`, `class`, `label`, `text` and `vector`. A `tenure`'s `began` is in the YAML and in
nothing the search touches. RFC-0040 said in its last line that this was #1029's question
and gave it a numeric type to ask it about; #1028 made the same point from the other side,
that a calculator's answer reaches the embedding record and then stops.

Two designs were possible and one was refused. A `where` could take its own small grammar —
a `field:op:value` triple, say — and evaluate it its own way over the column. That is a
second predicate language for the same corpus, and the two would disagree the first time one
of them met a partial date. RFC-0018 spent its longest section on what `1893 < 1893-06-01`
means; a second evaluator would have to spend it again or get it wrong.

## Proposal

### Index-carried is implied by type, not declared

Every property declared `date` or `number` is written into the index. There is no
`indexed: true` flag on the declaration and none is proposed. The ordered types are the
ones a predicate can be asked of, and a property a predicate can be asked of is one the
index should be able to answer for; a flag would let a corpus declare a `date` and then be
told `undeclared-property` by `retrieve` for a property `query` orders happily. `string`
and `text` properties are not carried, because nothing here filters on them and the text
column already carries the prose.

### One column, holding a JSON object

`corpus.arrow` gains a nullable Utf8 column `properties`, after `text` and before `vector`,
holding a JSON object of the row's carried values keyed by property name — `{"began":
"1893"}` — and null for a row with none: a source record, or a node whose class declares no
ordered property. A key is **absent** for an absent property, never null, because absence is
what the `>?` affix asks about and a null would be a second spelling of it. A number is
written as a JSON number and a date as the string the corpus wrote, so the evaluator reads
each the way `query` reads the YAML.

One column rather than one per property, for two reasons. The schema of the index would
otherwise be a function of the ontology, and an index built against one ontology could not
be read against another without a migration of the Arrow file — where a JSON column is
read by the same decoder whatever the corpus declares. And the browser shell decodes the
same file, and a decoder that learns one nullable string column learns it once.

`meta.json` gains `columns`, the ordered list the file was written with. A consumer that
wants to know what it may filter on reads that rather than the schema, which is the sentence
`status`'s `computed_signals` field has been waiting to point at.

### `where` is the filter grammar, verbatim

`retrieve --where 'began<=1893,ended>?1893'` takes the text inside a step's `[...]`, with or
without the brackets. It is parsed by the query parser's filter rule, exposed for the
purpose; there is no second grammar and no second table of operators. Everything RFC-0018
and RFC-0040 defined holds unchanged: commas are **and**; the six comparison operators and
the two affixes mean what they mean there; an ordering on a `string` is `unordered-property`;
`length_km>ten` is `unsatisfiable-predicate`.

It is typechecked by the query checker over one synthetic step — the caller's `class` or
`*`, no anchor, the parsed filter — and the checker's verdict is the answer. So a `where`
with no `class` narrows the way `*` does: to the classes that declare each named property
with an ordered type, which is `query`'s rule rather than a new one. A `where` that
typechecks where the query would not, or the reverse, is the surface disagreeing with
itself, and building the step rather than restating the loop is what makes that impossible.

### One evaluator, three readers

The query evaluator's predicate test is split in two. The half that reads a node's YAML and
the half that compares scalar values against a predicate under a declared type are separate
functions, and the second is the one `retrieve` calls. The vector arm hands it the values
out of the row's JSON column; the keyword arm hands it the values out of the node the server
loaded, which is the node `query` reads. Three readers into one comparison. The precision
rule for dates, the numeric rule for numbers, the absence rule for the affix — each is
written once and cannot drift.

### Applied before the `k` cut, as the residual

The search takes a pushable filter and a residual, and a `where` is exactly that pair: its
class narrowing is pushed, its comparison is the residual. The residual runs before the top
`k` are taken, so `k` counts rows the caller asked for. A `where` admitting three of a
hundred rows with `k: 5` returns three rows, not five with two struck out.

### Absence is absence, and there is no `as-of`

`ended>1893` never matches a row without `ended`; `ended>?1893` does. The canonical interval
is two comparisons and an affix, and the two conformance cases over `corpus-dated/` pin both
readings. An `as-of` argument that expanded to the interval was considered and refused: it
would be a third spelling of a question the grammar already asks, and every corpus's open
interval is not `ended` — one is `until`, one is `closed`, one is `repealed`.

### A predicate that admits nothing is evidence

`predicate-unsatisfied` joins `retrieve`'s absence codes. The `where` parsed and typechecked,
the class half admitted `instances` candidates, and the predicate refused every one. It is
reported only when at least one candidate was evaluated. That is the one empty answer here an
agent should read as a fact about the values rather than as a search that missed — *no
holding began before 1880* — and it is not `no-term-match`, whose claim is about words and
whose repair is to rephrase.

### An index without the column is rejected, not answered

`where` against an index that predates the column is `where-unindexed`, and its message
names `yidam embed && yidam index-build`. The alternative — evaluating the predicate over
rows whose `properties` are all null — would return nothing and report it as
`predicate-unsatisfied`, which is the wrong fact stated confidently. The index's schema is
read once at load, and the flag it sets is the difference between *nothing matched* and
*nothing could*.

### The remote index does not carry it

`index-push` does not forward the column, and a test asserts it does not. A server backed
by a remote index rejects `where` as `where-remote` and points at `query`. The 2 KB
filterable-metadata ceiling RFC-0033 measured against is the reason: a row's properties
would compete with its class and label for the half of the metadata a filter can read, and a
predicate that only sometimes fits is a predicate that only sometimes works. Keyword search
answers a `where` in every build, so the light default build and an indexed local one agree
about every row.

## What this does not touch

- **Signals as columns.** A computed signal (#1028) reaches the embedding record and stops
  there still. It is a number with a method and a receipt, and whether it is a *property*
  a predicate may name is a question about the ontology, not about the index. This RFC gives
  it a column to join when that is decided.
- **`string` and `text` in the index.** Not carried; not filterable. `~` on a `string` is a
  `query` question.
- **The SDKs.** No SDK decodes the index or calls `retrieve` with a `where`; parity is
  untouched.

## Migration & compatibility

An index built before this RFC decodes as before, with every row's `properties` as `None`,
and `retrieve` without a `where` is unchanged over it. A `where` against it is rejected
with the repair named. There is no migration file: `yidam embed && yidam index-build`
rewrites the records and the index, and every corpus that wants the argument runs the two
commands it already runs.

The embedding record gains a `properties` object, absent where empty — the convention
`signals` set — so a corpus with no ordered property writes the records it wrote before,
byte for byte. The SQLite export gains a `properties` column beside `text`. The web shell's
decoder reads the column when it is there.

The MCP contract bumps to 0.26.0. `retrieve` gains an optional `where`; a client built
against 0.25.0 never sends one and sees the response it saw.

## Alternatives considered

- **A grammar of `retrieve`'s own.** Refused above: a second predicate language for one
  corpus, and the first partial date splits them.
- **One Arrow column per property.** Refused above: the index schema becomes a function of
  the ontology, and the browser decoder learns every corpus separately.
- **An `indexed:` flag on the declaration.** Refused above: it lets `query` and `retrieve`
  disagree about which properties exist.
- **An `as-of` argument.** Refused above: a third spelling, and no corpus spells its open
  interval the same way.
- **Evaluating over a column-less index and reporting `predicate-unsatisfied`.** Refused
  above: the wrong fact, stated as a fact.

## Open questions

- **Whether a signal is a property.** If #1028's signal tables are ever declared on the
  class, they join the column by the rule above and nothing here changes. If they are not,
  a predicate over them needs a name for what it is filtering on, and this RFC does not
  propose one.
- **Whether a remote index should carry a bounded subset.** A corpus with two `date`
  properties and no `number` is well inside the ceiling. The refusal is total today because
  a sometimes-working filter is worse than none; a per-corpus measurement at push time could
  make it a decision rather than a rule.
