# RFC-0050 — A list property type

- **Status:** Draft
- **Track:** G14
- **Relates to:**
  - RFC-0040 and RFC-0046 (the precedent for a new declared type: one arm in `property_type_violation`, one in each SDK's schema compiler, one parity fixture)
  - RFC-0044 (the declared value set, which this RFC applies to each item of a list)
  - RFC-0018 (the query surface, which refuses an operator it cannot answer on a list)
  - RFC-0049 (the warehouse contract, which today refuses a `list` property outright)
- **Versioning layers touched:** SDK parity 0.18.0 → 0.19.0 (`OntologyProperty.items` and `.fields`, the `list` arm in all three class schema compilers). `yidam-core` 0.11.0 → 0.12.0, because two public fields on a struct a caller can construct exhaustively read as breaking to `cargo semver-checks`. Tooling: one arm in `property-type`, a narrowed node-standing read, two new lint checks, a warehouse skip in place of a refusal. **No MCP contract bump:** a list has no order, and both refusals below use codes the contract already freezes. No migration.
- **Downstream reference cases:**
  - #1391, `travel-demand`'s `pathway.effect`: several effect sizes on one node, each with its own source and its own standing, declared with a coined `type: effect` that nothing reads.
  - #1382, civic-infrastructure's `model.parameters` and `failure-mode.effects`: an untyped list whose entries carry `claim_tag: open`, read as the standing of the node that holds them.

## Summary

A corpus cannot say that a property holds several values, or that each value is a record with
its own fields. This RFC adds **one type, `list`**. A list declares what its items are in one
of two ways: `items:` names a scalar type that every item has, and `fields:` declares the
fields of a record that every item is. A `claim` field inside a record is the standing of that
entry and not of the node. A `source` field beside it resolves the way a link's `source:` does.

## Problem

### The two corpora that asked

Both derived repositories hold values that carry their own evidence. One pathway in
`travel-demand` carries a short-run and a long-run fare elasticity. One rests on a
meta-analysis of 33 studies and the other on 3, so a single node-level standing cannot hold
both. civic-infrastructure keeps a model's parameters the same way.

Neither corpus has a sanctioned form to write this in, so each wrote what it could:

- **`travel-demand` coined `type: effect`.** The gate leaves a coined type alone, so a
  nonexistent `source`, a `claim_tag` of `definitely-true` and a text `value` all passed both
  `graph-check` and `lint` (#1391).
- **civic-infrastructure left the list untyped.** The claim reader walks the whole node at any
  depth, skipping only `links`. It found `parameters[].claim_tag: open` and reported that the
  node declares `[open]`, while the node's own declared standing was `inference` (#1382).
  `claim-property-undeclared` then told the corpus to declare `claim_tag` as a top-level
  property, which misdescribes the node.

### What is on this machine

Measured 2026-10-29 over the tracked corpora under `/Users/cparent/Code` (20 repositories,
4,603 instance nodes). The two reference corpora are not among them.

- **5 property values are lists**, all in one repository.
- **`demi-moore` already coined `type: list`**, on `observation.extracts` and
  `priority.extracts`. It declares neither `items:` nor `fields:`. Every value is a non-empty
  list of mappings with the keys `file`, `produced_by`, `key`, `grain`, `reconcile` and
  `series`.
- **No other property value is a list**, outside `claim` and `quotation`, which already
  accept one.

The population is small, and that makes the change cheap. It also puts one constraint on the
design: **`type: list` written bare must stay valid**, because one corpus has already written
it.

## Proposal

### One type, three forms

```yaml
properties:
  - name: aliases            # a list of one scalar type
    type: list
    items: string

  - name: effects            # a list of records
    type: list
    fields:
      - { name: label,     type: string, required: true }
      - { name: value,     type: number, unit: elasticity }
      - { name: claim_tag, type: claim }
      - { name: source,    type: ref }

  - name: extracts           # a list of anything
    type: list
```

**A value is a non-empty YAML sequence.** A single item that is not wrapped in a list is
reported by `property-type`. The one-or-a-list tolerance that `claim` and `quotation` have
exists because `claim_tag: [open]` becomes a list without the author meaning it to. A
property declared `list` was meant to be a list. An empty list is reported as well, because it
says nothing that leaving the property out does not. This is the same reading RFC-0044 gives
`values: []`: an empty list is absent.

**`items:` and `fields:` are mutually exclusive.** A declaration with both is malformed, and
so is one naming a refused type. A malformed declaration is reported on the class, by
`list-declaration-invalid` (Error), and its values are checked only as a bare `list`. Neither
is required. A bare `list` checks only that the value is a non-empty sequence, which
is what `demi-moore` relies on.

### `items:` — a list of one scalar type

`items:` names one of `string`, `text`, `ref`, `date`, `number` or `claim`. Each item is
checked by the same arm of `property_type_violation` that checks a top-level property of that
type. `values:` and `unit:` on the list apply to every item, as they would to one scalar.

`items: list` and `items: quotation` are refused. A nested list has no reference case. A
quotation already accepts a list of itself.

### `fields:` — a list of records

`fields:` is a list of property declarations, in the grammar class properties already use:
`name`, `type`, `required`, `values`, `unit` and `description`. Each item must be a mapping:

- each declared field is checked by the arm for its type
- a field declared `required: true` must be present in every item
- **a key the declaration does not name is reported**, by `property-type`. A record follows
  the rule a class follows. `undeclared-property` gates a key the class did not name, once the
  class names any. Silence still constrains nothing: a bare `list` declares no fields and
  closes nothing.

A field may be any scalar type or `claim`. A field typed `list` or `quotation` is refused in
this RFC. The quotation reader matches top-level keys only, so a nested quotation would go
unchecked and uncounted. See *Open questions*.

### A record's standing is the record's

This is the #1382 fix.

**A node's standing is read only from the claim-typed properties at the top level of
`properties:`, or of the document itself.** One reader, `declared_values`, now serves
`node_standing`, `structural_claims` and the counts, and it never descends into the value of
any property. `named_values`, the walk it replaced, is gone. The document root stays a level
because the edge-claim fixtures write `status:` there. That covers both cases:

- a property declared `list`
- an undeclared property whose value is a list or mapping

GRAPH.md already says that "a node's standing here is a property its class declared `type:
claim`", and `agent-conduct.md` calls it "the declared claim-typed field". A tag inside an
undeclared list is neither. The walk is what departed from that rule.

**A `claim` field in a record is one claim per entry.** `status`, `open-questions`,
`corpus-index` and the MCP `claims` tool count it, with the same parser that grades a node's
`type: claim`. Each entry is served as a statement-scoped claim, labelled by its list and item number,
and `node_standing` reads node-scoped claims only, so it does not count toward the node's own
standing. An entry tagged `[open]` is an open entry of a node that may itself be
`[verified]`.

**`claim-property-undeclared` stops reporting a tag that a `fields:` declaration names.** A
nested `claim_tag` under a property that is **not** declared `list` is still reported, and
the suggestion changes: declare the property `type: list` with a `claim` field, instead of
declaring the tag at the top level.

### A record's source resolves like a link's

This is the #1391 ask. When a `fields:` declaration includes a `claim` field, the record is
an *evidenced entry*. A `source` key in an evidenced entry resolves exactly as an edge
`source:` does, by stem or by path through `source_targets`. One new check:

| Check | Severity | Reported when |
|---|---|---|
| `entry-source-unresolved` | Error | an evidenced entry's `source:` names no catalog entry or node |
| `list-declaration-invalid` | Error | a `list` declaration names both `items:` and `fields:`, an `items:` type outside the six, or a field typed `list` or `quotation` |

The population is empty until a corpus declares `fields:` with a `claim` field, so no
existing corpus fails the day this ships. A resolved `source:` counts as a citation of its
entry, the way an edge `source:` counts (#1174). It is rewritten by `yidam rename`, which
already rewrites `source:` by key name, in block style only. A flow-style entry, `{ source: x }`,
is not rewritten, as a flow-style edge is not today.

The rule is keyed on the name `source` and not on a type. A link already uses that name for
the same meaning, and a `ref` field is not resolved anywhere today. Both #1391 and #1382 spell
the field `source`.

### The compiled schema

`property_schema` takes the whole declaration, not `(type, values)`, in all three SDKs:

| Declared | Compiled |
|---|---|
| `list` | `{"type":"array","minItems":1}` |
| `list`, `items: T` | `{"type":"array","minItems":1,"items":<schema of T with the list's values/unit>}` |
| `list`, `fields: […]` | `{"type":"array","minItems":1,"items":{"type":"object","properties":{…},"required":[…],"additionalProperties":false}}` |

The record object is closed, as the class's property bag is, because the gate reports an
undeclared key. `required` is omitted when it is empty, as at class level. An empty `fields: []`
declares nothing and compiles as a bare `list`, the way `values: []` declares no set. `items:` and `fields:` declared together, or
either one set to a type that is refused, compile to `true`. This matches the gate, which
reports the declaration and constrains nothing.

`every-declared-type.toml` gains all three forms. A new fixture holds a malformed `fields:`.
Rust's `parse_class` deserialises with serde, and one wrongly shaped field would empty the
whole class, while Python and TypeScript tolerate a bad field one at a time. The fixture
holds all three to the tolerant behaviour.

### What the other readers do

- **`yidam schema`** adds `list` to both type enums and declares `items` and `fields` on the
  property item. The property item's `additionalProperties: false` would otherwise refuse
  them. A field's schema is not recursive: it has no `items` or `fields` of its own, because
  a field cannot be a `list`.
- **Query.** `prop?` works. Orderings are refused `unordered-property`, as on every type
  except `date` and `number`. `=` and `~` are refused `unsatisfiable-predicate`. Membership
  (`aliases=foo` meaning that any item equals `foo`) is a new semantics for the contract, and
  is deferred.
- **Warehouse.** A `list` property gets no column, as `claim` and `quotation` get none, and
  textql lists it in the *Stated in prose* section. Today the contract refuses it, so this
  loosens the contract. Emitting a child table is RFC-0049's business.
- **`migrate`.** A retype into `list` is accepted only when every value already satisfies
  the declaration. That is the rule every retype follows, through `property_type_violation`.
- **`graph.json`, the editors and `universal.yml`** carry the bare type string and nothing
  else in this RFC. `items:` and `fields:` are read from class files only.

### Prose

- **GRAPH.md** gains one paragraph beside `number` and `quotation`, and the standing sentence
  names the top level. The word budget in `prelude_rules_and_evidence.rs` moves by that
  paragraph.
- **GRAPH.evidence.md** gains a `#list` anchor with the reference cases.
- **`docs/configuration.md`**, **`bootstrap.md`**, **`yidam/sdks/README.md`** and the
  `type` description in `report.schema.json` gain the word. Three of these already omit
  `number` or `quotation`, so this edit corrects them as well.
- **`docs/upgrading.md`** gains an entry.

## Migration & compatibility

- **A corpus that coined `list`.** The gate now reads the type. `demi-moore`'s five values
  satisfy the bare form as they stand (measured above).
- **The warehouse contract** stops refusing that corpus.
- **A corpus whose node standing came from a nested tag.** Its standing changes. That
  standing was the #1382 defect, but a corpus could have come to depend on it.
  `watermark-directory` projects its corpus and puts typed fields wherever its exporter
  writes them. Its projection is untracked, so it was not measured.

  **Measured 2026-10-29 over the 20 corpora on hand, 5,132 nodes.** Every one of the 1,921
  declared claim values sits at the top of `properties:`. None is nested and none is at the
  document root. The narrowing moves **no** count on any of them. The unit test that pinned
  the old nested read, `a_nested_declared_field_is_found`, is replaced by
  `a_nested_declared_field_is_not_the_nodes`.

## Alternatives considered

- **`type: [string]` or `list<string>`.** These are shorter, but they put a grammar inside a
  string. Every reader that matches `type` as a bare word would mistake the string for a
  coined type and pass it silently.
- **A `multiple: true` flag on any type.** This handles scalar lists and has no place to
  declare a record's fields, which is the case both issues are about.
- **A `record` type, so that a list of records is `items: record`.** That adds two types
  where one will do. No corpus holds a single record that is not in a list.
- **Leaving a record open to undeclared keys.** This would be looser than the class level,
  where `undeclared-property` gates. A misspelled `claim_tg` would then pass as an undeclared
  key, and the entry would read as having no standing. That is the silent loss #1391 reports.
- **Splitting each entry into its own node with a graded link.** This is #1391's fallback.
  It works today. It costs the corpus the engine-facing shape it was asked for, and it costs
  one node per elasticity.

## Open questions

- **Nested quotations.** A `quotation` field in a record needs `quotations::Declared` to walk
  record fields. The three quotation checks and the citation count read that walk. This is
  deferred until a corpus asks.
- **Query membership.** Should `=` on an `items:` list mean "any item equals"? Answering yes
  changes the contract and bumps the MCP version.
- **A `property-type` finding for an unknown type.** #1391 also asks for this. It is a
  separate change, because it reddens every corpus that coined a type, not only the ones
  that coined `list`.
- **The node-standing narrowing.** *Settled by the measurement in Migration*: no corpus on
  hand puts a declared claim field anywhere but the top level, so the walk "a projected corpus
  may put its typed fields anywhere" served no corpus measured. An unmeasured projection could
  still depend on it.
