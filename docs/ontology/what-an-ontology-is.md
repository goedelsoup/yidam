# What an ontology is here

*The schema layer of a corpus: what kinds of thing exist in this domain, and how they may
connect. Written once at bootstrap, and the thing every gate reads afterwards.*

A yidam corpus has two layers. **Classes** say what kinds of thing exist. **Instances** are the
things. Both are files, and the corpus is the pair.

```
.yidam/corpus/
  gage.ont.yml            ← a class: what a gage is
  gage/
    valley-bridge.yml     ← an instance: one gage
    mill-creek.yml
  reach.ont.yml
  reach/
    lower-canyon.yml
```

A class file is `<class>.ont.yml`. Its instances live in `<class>/`, one file each. That is the
whole layout, and [information architecture](../information-architecture.md) has the full
schemas.

## What a class declares

```yaml
# illustrative — not a run
class: gage
label: Stream gage
description: |
  A fixed installation that measures stage or discharge at one point on a stream.
properties:
  - name: operator
    type: string
    description: The agency that maintains it.
    required: true
edges:
  - relationship: measures
    target: reach
    direction: out
    description: The reach this gage reports on.
edge_policy: characteristic
max_lines: 40
```

Four things are being said, and each one is what some check reads later.

**`description`** says what an instance *is*. Not what it is for. A class whose description
states a purpose states it where no claim tag attributes it, so the checks that run on tags
read past it. `class-asserts-purpose` reports it.

That is a rule about what those checks read, not a fact about corpora. Class prose *does* carry
evidence tags — measured over seventeen corpora, three classes in three of them do. Two checks
make that visible. `claim-tag-malformed` reads every field of a class file, so a folded tag like
`[verified — a source]` is reported wherever it is written. `class-claim-uncounted` reports, at
`Info`, how many well-formed tags a class asserts.

Neither one counts them. `yidam status` counts claims in nodes, and Article V defines a claim as
a statement in a node — see [constitutional governance](../constitutional-governance.md). If a
class is arguing something that needs counting, the argument belongs in a node.

**`properties`** are the typed fields an instance may carry. `required: true` makes an absent one
a finding. A property typed `claim` marks a field whose value *is* an evidence tag rather than
prose mentioning one.

**`edges`** are the relationships instances may bear, from whichever end authors them. An edge to
a class file or into the catalog is a citation, not a relationship, and is not read here.

**`edge_policy`** decides whether that list is a bound or a description:

| Value | Meaning |
|---|---|
| `exhaustive` | The vocabulary is closed. A relationship outside `edges:` is an error. |
| `characteristic` | `edges:` names what the class is *defined by*. Anything outside it is a deliberate coinage, and passes. |
| *(unstated)* | The class has not said. Reported, and does not gate. |

The default is deliberate. A list never says "and no others" on its own, and gating an unstated
policy would enforce a contract nobody wrote.

## An ontology is not a specification of the code

This is the most common misreading, and worth stating plainly. **A class is not expected to
have a type of its name in `crates/`.**

That is measured, not asserted. Across twelve derived corpora, **129 of 157 declared classes have
no `struct` or `enum` bearing their name**. Widening the match to traits, aliases and every
language in the tree makes it worse — 165 of 186, or 88%. Five of those corpora match nothing at
all.

They are not behind. Their ontologies model a research domain, and their `crates/` model the
pipeline that gathers evidence about it. There is no expectation that the two share a name.

So a class may declare `implemented_by:` and be held to it, or say nothing and be held to
nothing. A check that ran unconditionally would call 88% of every ontology a debt.

## Where the ontology comes from

A yidam repository does not ship with an ontology and does not infer one. It is
**bootstrapped**. An agent reads the inherited prelude, runs an ontology-discovery dialogue
with you, and writes the genesis commit only after you confirm the model.

Scaffolding before that dialogue is the failure the whole process is shaped against. See
[bootstrap flow](../bootstrap-flow.md) for the sequence.

One question that dialogue asks is which **foundational ontology** the classes align to, if any.
That is a separate decision with its own page: [choosing an alignment](choosing-an-alignment.md).

## Why it is a schema and not a suggestion

The point of declaring any of this is that a corpus becomes computable over. A query can walk
`reach -measured-by-> gage` because `measures` is declared, typed, and checked. A retrieval can
filter by class because classes exist. A gate can refuse a node that names no class.

None of that is available to a folder of Markdown. All of it follows from a schema layer
somebody wrote down, and a gate holds them to.

## Reading on

| If you want to | Go to |
|---|---|
| Pick a foundational alignment | [Choosing an alignment](choosing-an-alignment.md) |
| Write the alignment field | [Alignment in practice](alignment-in-practice.md) |
| See the full file schemas | [Information architecture](../information-architecture.md) |
| Watch one get written | [Bootstrap flow](../bootstrap-flow.md) |
