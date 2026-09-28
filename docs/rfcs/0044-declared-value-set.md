# RFC-0044 — A declared value set

- **Status:** Draft
- **Track:** G12
- **Relates to:**
  - RFC-0040 (the numeric type, whose `unit:` is the precedent for a fact about the column declared beside `type:` and carried through unchanged on every other type)
  - RFC-0016 (the ontology contract, whose `property-type` check gains a second predicate and whose compiled class schema gains one constraint)
  - RFC-0038 and RFC-0039 (the rules/evidence split and the occasion-scoped read — `GRAPH.md` gains one sentence and the ceilings move by that sentence)
- **Versioning layers touched:** SDK parity 0.14.0 → 0.15.0 (`OntologyProperty.values`, the `enum` arm in all three compilers). `yidam-core` 0.9.0 → 0.10.0, because `values` is a new public field on a struct a caller can construct exhaustively, which `cargo semver-checks` reads as breaking. MCP contract unchanged: no operator, rejection code or message moves. CLI surface: `property-type` reports a `string` outside its declared set; `migrate retype` into `string` is held to the set. No migration.
- **Downstream reference case:** `allen-county-ohio`'s `site.status`, whose description reads *"extant | demolished | ruin | repurposed | in operation"* and whose instances hold `"demolished in 1971"`, and `matt-huffman`'s `tenure.how_ended`, declared `resigned | term-limited | lost | ...` in prose and holding `resigned to enter Congress`.

## Summary

A class cannot declare a closed set of values, so 40 properties across seven corpora spell one
in their description, where nothing reads it, and 33 instances hold a value outside the set
their own class documents (#1052). This RFC adds **one field, `values:`**, beside `type:` on
a `string` property, and states the three rules that make it usable: declaring the set
closes it; the match is exact, as written; and the field binds `string` alone. The gate
`property-type` reports a value outside the set, and the compiled schema carries the set as
`enum`, so the two cannot disagree about what a value may be.

## Problem

A closed set is the most common shape a `string` property takes in the derived corpora — a
status, a kind, a mode, a how-it-ended — and the ontology has no word for one. So a class
writes it where it can, in the description:

```yaml
- name: status
  type: string
  description: extant | demolished | ruin | repurposed | in operation
```

The description is prose. `lint` does not read it, the compiled schema does not carry it,
and an editor completing the field has nothing to offer. What the corpora show is the
consequence: measured across seven of them, 40 properties document a set this way and 33
instances have drifted off it. The drift is of two kinds, and both are the finding this RFC
is for. One is a **widening nobody recorded** — `abandoned` where the set says `demolished`
or `ruin` — which the author may well have meant, and the class should now say. The other is
**prose in a token field** — `"demolished in 1971"`, `resigned to enter Congress` — where the
year and the reason belong in another property or the detail, and the token has stopped
being a token. Neither is reported today, and a query for `site[status=demolished]` silently
misses both.

RFC-0040 already put a fact about a column beside `type:`. A value set is the same kind of
fact, and the description is where it was living because the declaration had no room for it.

## Proposal

### One field, `values:`, beside `type:`

```yaml
- name: status
  type: string
  values: [extant, demolished, ruin, repurposed, in operation]
  description: Whether the site still stands.
```

A list of strings, on the declaration. The instances carry the token, as before. The
description goes back to being a description.

Spelled as a sibling of `type:` rather than as a type of its own — `type: [extant, ...]` or
`type: enum` with the set elsewhere — because it is a fact about a `string`, and every reader
that already dispatches on `type` keeps working. `unit:` is the precedent, and so is
`edge_policy`: a fact about a declaration is declared on it, in its own key.

### Declaring the set closes it

There is no `closed: true` beside it, and no `open:` marker. A class that writes the set has
asked for the gate, as a class writing `edge_policy: exhaustive` has. The alternative, an
open list that documents the common values without binding them, was rejected because it is
the description again: a list nothing enforces is what the 40 properties already have.
A class that wants an unbounded `string` declares no `values:`, which is every class today,
so nothing changes for a corpus that does not ask.

An empty list is absent. `values: []` binds nothing rather than refusing everything, because
a closed set of nothing is not a set any class means.

### The match is exact, as written

`in operation` matches `in operation` and nothing else — not `In operation`, not
`in-operation`, not a trimmed or folded spelling. JSON Schema's `enum` is exact, and the
gate's rule is *no stricter than the schema*, which cuts both ways: a gate that admitted
`In operation` would be looser than the schema it exists to be no stricter than, and a
corpus would learn the difference from a validator instead of from `lint`. A class that
wants both spellings declares both.

The set is published in declaration order, unsorted and untrimmed, so the three compilers
produce one document.

### The field binds `string` alone

`text` is prose and has no set. `ref` is a path. `date` and `number` are values, not tokens.
`claim` has its own set, the six evidence tokens. A coined type has said the gate does not
know its shape, and a set on it would be the gate half-knowing. On every one of these
`values:` is carried and ignored, as `unit:` is off `number` — carried, so a class-file
schema of this CLI does not refuse it; ignored, so a retype away from `string` does not
silently keep binding.

That last clause is what `migrate retype` reads. A retype *into* `string` on a property that
declares a set is the moment the set starts to bind, and an instance outside it would be a
migration into a failing build — the refusal `retype` already makes for a value the new type
cannot hold. It reports each instance, names the two repairs, and writes nothing.

### The gate and the schema say one thing

`property-type` gains the predicate rather than a check of its own, because it is the same
finding: a value the declaration does not admit. The type is tested first, then the set. A
bare `7` under a `string` is reported once, for its shape, and not a second time for not
being one of `extant`, `demolished` and `ruin`. The message names the set, so the repair is
in the finding.

The compiled class schema emits `{"type": "string", "minLength": 1, "enum": [...]}` for a
`string` that declares a set — a constraint, unlike `x-yidam-unit`, because the gate refuses
an off-set value and a schema that admitted one would be looser than the gate. `universal.yml`
declares types and no sets, so the universal arm of the check is unchanged.

## What this does not touch

- **The query checker.** `site[status=abandoned]` against a class whose set lacks
  `abandoned` is a predicate no row can satisfy, and `unsatisfiable-predicate` is the
  rejection for it. It is not added here because it would move the MCP contract, and the
  contract stays where it is until a client asks. See the open questions.
- **Non-`string` types.** No set on a `number`, no set on a coined type.
- **A rename across the set.** `creek` → `stream` on the declaration and every instance is
  `property-rename`'s shape one level down, and it is a migration of its own (filed as a
  follow-up to #1052), not a consequence of declaring the set.
- **The index and `retrieve`.** A `string` with a set is the same column it was.

## Migration & compatibility

No corpus declares `values:`, so no corpus changes. A class file carrying it is refused by
the class-file schema of every CLI before this one (`additionalProperties: false`), which is
the right refusal: an older CLI would compile the set to nothing and validate `abandoned` as
fine, and a corpus should learn that from a schema error rather than from a green build that
means less than it says.

A corpus that adopts the field on a property with drifted instances will see each one
reported, which is the point. The 33 are the reason the field exists, and every one of them
is either a value to add to the set or a token to restore.

## Alternatives considered

- **An `enum` type.** Rejected above: it forks every reader that dispatches on `type`, and
  the set is a fact about a `string`, not a different kind of value.
- **An open list, or a `closed:` flag.** Rejected above: an open list is the description
  again, and a flag is a second declaration to forget.
- **A case-folded or trimmed match.** Rejected above: the schema's `enum` is exact, and the
  gate is no stricter and no looser than the schema.
- **Reading the set out of the description.** The `a | b | c` shape is regular enough to
  parse, and parsing it would have reported the 33 without a new field. Rejected because a
  description is prose, and a gate that reads prose as a declaration has invented one on the
  author's behalf — the failure `claim-tag-malformed` exists to report.

## Open questions

- **Whether the query checker should refuse an off-set literal.** `status=abandoned` where
  the set lacks it is unsatisfiable, and saying so is cheap. Lean: yes, when the contract
  next moves for another reason. It is one arm in `check_pred` and one line in the contract.
- **Whether a set should be declarable on a coined type.** A corpus that coined `site-status`
  and wants it closed has to retype to `string` today. Lean: no. A coined type is the corpus
  saying the gate does not know the shape, and a set is the gate knowing it.
