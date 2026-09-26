# RFC-0040 — A numeric property type

- **Status:** Draft
- **Track:** G10
- **Relates to:**
  - RFC-0018 (the query surface, whose ordering operators were deferred over the absence of exactly this type and then shipped `date`-only — this RFC is the "one condition in `check_pred`" its open question said would have to change)
  - RFC-0016 (the ontology contract, whose `property_type_violation` gains one arm and whose compiled class schema gains one annotation)
  - RFC-0038 and RFC-0039 (the rules/evidence split and the occasion-scoped read — `GRAPH.md` gains one sentence and the ceilings move by that sentence)
- **Versioning layers touched:** SDK parity 0.12.0 → 0.13.0 (`OntologyProperty.unit`, the `number` arm in all three compilers). `yidam-core` 0.7.0 → 0.8.0, because `unit` is a new public field on a struct a caller can construct exhaustively, which `cargo semver-checks` reads as breaking. MCP contract 0.24.0 → 0.25.0 (an ordering is licensed on `number`, and what it does there is stated). CLI surface: `property-type` reports a quoted number; `unordered-property`'s message names two types instead of one. No migration.
- **Downstream reference case:** `examples/streamflow`'s `reach.length_km`, declared `string` and holding `"~24"` and `"~6"` — the property RFC-0018 used as its example of the trap, in the corpus that ships with the template.

## Summary

There is no numeric property type, so no corpus has one, and an ordering refuses everything
but a date (#1030). This RFC adds **one type, `number`**, and states the three rules that
make it usable: a number is a YAML number, unquoted; a unit is a fact about the column and
not the cell; and comparison is exact, with `=` numeric so the three operators stay
trichotomous. The ordering operators are then defined on `date` and `number` and refused
everywhere else, which is RFC-0018's refusal with one more type on the licensed side.

## Problem

RFC-0018 deferred `<`, `<=`, `>` and `>=` because the declared types were `string`, `text`,
`date`, `ref` and `claim`, with no numeric among them, and an ordering that fell back to
comparing text would be correct on `date` and a trap on the rest — `length_km<9` would rank
`10` before `9` and say nothing about having done so. #725 shipped the operators on `date`
and kept the refusal, and both RFC-0018 sentences that mention a numeric type say the same
thing: *if one is ever declared, this is the one rule that has to change.*

The cost of not declaring one is measurable in the template's own example. `reach.length_km`
is declared `string` and holds `"~24"`, and the sentence in `docs/cli-reference.md` that
demonstrates the refusal uses it: `yidam query 'reach[length_km<9]'` comes back
`unordered-property`. The corpus knows the answer and cannot be asked. Across the derived
corpora, every gage discharge, every dollar figure, every count is either a `string` the
gate cannot read or a coined type the gate leaves alone — and a coined type is refused an
ordering too, because nothing knows what its order is.

The computed-signal work (#1028) made this sharper: a calculator's answer now reaches the
embedding record, but a calculator's answer is a number, and the ontology has no word for one.

## Proposal

### One type, `number`

Not `integer` and `number`. A corpus that has coined `integer` keeps it and the gate keeps
leaving it alone; a second numeric type would be a distinction the query language does not
make and the compiled schema would have to. JSON Schema's own `number` admits both, and the
compiled class schema says exactly that: `{"type": "number"}`.

### A number is a YAML number, unquoted

`length_km: 24`, never `length_km: "24"`. The `string` arm of `property-type` already says
"is a number, not text — quote it" for `parameter: 00060`; this is its mirror. A `number`
field holding a quoted value is reported "is text, not a number — unquote it", one holding
prose is reported "is not a number", and `.inf` and `.nan` are refused because neither is a
quantity a corpus measured. The compiled schema refuses a quoted number too, so a gate that
accepted one would be looser than the schema it exists to be no stricter than.

One parser, `numeric_value`, serves the gate and the query, for the reason `iso_date_parts`
serves both on dates: a second parser is a corpus where `lint` accepts a value that `query`
will not order, which is the quietest possible split.

### A unit is a fact about the column, not the cell

```yaml
properties:
  - name: length_km
    type: number
    unit: km
    description: Approximate channel length.
```

`unit:` is an optional field on the declaration beside `type: number`. The instances carry
bare numbers. No `unit:` means dimensionless — a count, an index, a ratio. A unit on any
other type is carried and ignored.

The alternative, a unit in the cell (`24 km`, or `{value: 24, unit: km}`), was rejected on
three grounds. It makes every instance repeat a fact the class already knows. It makes the
comparison rule depend on unit conversion, which is a library and not a query operator. And
it makes the compiled schema either a string pattern or an object, neither of which a
validator can order. The declaration is where a fact about every value of a property lives —
`description`, `required`, `prose` and `retrievable` are all there for that reason.

The compiled class schema publishes the unit as `x-yidam-unit`, an annotation like
`x-yidam-edges`: an editor can show it, a reader can cite it, and no validator treats it as a
constraint. A computed signal (#1028) already carries its unit in its `method:` block, so a
calculator's output and a declared property agree about where a unit is written.

### Comparison is exact, and `=` is numeric

There is no precision rule for numbers, and the RFC says so rather than leaving it to be
inferred from the date rule. `1893` denotes an interval — every day of that year — which is
why an ordering on dates drops to the precision the two sides share and `=` matches at the
precision the query wrote. `7` and `7.0` denote the same point. So:

- `<`, `<=`, `>`, `>=` compare the two values as numbers. `10 > 9` holds.
- `=` compares numerically. `length_km=7.0` matches a stored `7`. Without this, a value
  could be in none of `<`, `=` and `>`, which is the one property a caller reasons about an
  ordering by.
- `!=` is `=`'s complement on the same reading.
- `~` stays textual on every type, as it is everywhere.
- A stored value that is not a number orders against nothing and equals nothing, as a
  malformed date does: `property-type` reports it and does not gate, so a query has to
  survive meeting one, and guessing an answer for it would be the undercount's louder twin.

The operand is typed through the same parser. `reach[length_km<nine]` is
`unsatisfiable-predicate` with the gate's own sentence, "`nine` is not a number" — but not
"unquote it", which is the right sentence for a corpus file and nonsense for a query.

### The executor dispatches on the declaration, never on the value

`check_pred` already reads the declared type to license the operator. It now carries what it
read into the executor, which dispatches `=` and the orderings on it: numerically on a
`number`, by date on a `date`, and as text otherwise. The stored value is never sniffed —
`7` is a legal `string`, and a `string` holding digits keeps comparing as text. A `*` query
narrows an ordering to the classes that declare an ordered type, exactly as it narrowed to
`date` before.

### What the fixture decides

`corpus-measured/` joins the MCP conformance corpora: three reaches measuring 10, 9 and 7
on a `number` with `unit: km`. It exists because a lexical implementation is **invisible**
over `corpus-dated/` — a fixed-width date orders the same as text and as a value — and
visible here: `length_km>9` must return the 10, and `length_km=7.0` must return the 7. A
server comparing text answers zero rows to both.

## What this does not touch

- **Index columns.** A `number` is not indexed and `retrieve` does not filter on it. #1029
  is the place that decides whether a numeric column belongs in the index, and this RFC
  gives it something to index.
- **Unit conversion.** `km` and `mi` are two columns. A query does not convert, and a
  comparison across two properties with different units is not a comparison this surface
  offers.
- **`migrate retype`.** Retyping a `string` to `number` refuses when an instance holds
  `"24"`, because the gate refuses it — a quoted number is text. Unquoting mechanically is a
  follow-up; today the corpus edits the instances first.
- **Coined types.** `integer`, `discharge`, `river-mile-range` compile to `true` and are
  refused an ordering, as before. Nothing about coining changed.

## Migration & compatibility

No corpus has a `number` property, so no corpus changes. A class file with `unit:` on a
property is refused by the class-file schema of every CLI before this one
(`additionalProperties: false`), which is the right refusal: it would compile the unit to
nothing and validate a quoted number as fine.

`examples/streamflow` retypes `reach.length_km` to `number` with `unit: km`, and its two
instances write `24` and `6`. The description already said "approximate", which is where the
tilde was carrying its meaning.

The MCP contract bumps to 0.25.0. The rejection code `unordered-property` is unchanged;
its message names `date` and `number`. A client built against 0.24.0 never asks to order a
number, so nothing it does today behaves differently.

## Alternatives considered

- **`integer` and `number` as two types.** Rejected above: a distinction the query language
  cannot express and the schema would have to enforce, for no reader that wants it.
- **A precision rule for numbers**, on the model of dates — `7` matching `7.0` through
  `7.4`. Rejected: `7` is not an interval, and a corpus that means one writes it as one.
- **Ordering coined types by falling back to numeric parsing where the value parses.** The
  trap RFC-0018 refused, one layer down: it would order `river-mile-range` values that
  happen to be bare numbers and refuse the ones that are not, on the same property.
- **Unit in the value.** Rejected above, on three grounds.

## Open questions

- **Whether a `number` should carry a range.** `min:` and `max:` on the declaration would
  compile to JSON Schema `minimum` and `maximum` and let `property-type` report a negative
  length. Lean: not yet. No corpus has asked, and the gate's rule is *no stricter than the
  schema*, which a range would extend on both sides at once.
- **Whether `retrieve` should filter on a number.** This is #1029's question, and it has a
  column to ask it about now.
