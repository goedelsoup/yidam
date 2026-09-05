# Alignment in practice

*The field, what validates it, and what reaches RDF. Read this once you have picked an
alignment, or when a class is not exporting what you expected.*

## The field

One field per class, in `<class>.ont.yml`:

```yaml
# illustrative — not a run
class: marriage
label: Marriage
foundational_type:
  ontology: ufo
  type: relator
  iri: https://purl.org/nemo/gufo#Relator
description: |
  The bond two people enter at a wedding, and which the records attest indirectly.
```

Three keys, and only two are required.

| Key | Required | What it is |
|---|---|---|
| `ontology` | yes | `bfo` or `ufo`. Nothing else validates. |
| `type` | yes | The type in that ontology — `relator`, `continuant`, `role`. |
| `iri` | no | The identifier that type has in BFO or gUFO. |

**Omit the whole field for `none` alignment.** An empty or partial one is worse than an absent
one. The corpus then asserts an alignment nothing can resolve.

## Why the IRI is optional

`iri` is where `skos:exactMatch` comes from, and a corpus that has not looked one up should still
be able to state its alignment.

The alternative was deriving the IRI from `type` with a BFO and gUFO term table compiled into
`yidam`. That table would be a second copy of somebody else's vocabulary. It would be wrong the
moment either project revised it, and would refuse terms a corpus is right to use. So the
corpus supplies the IRI or does not, and the alignment exports either way.

For the same reason **`type` is not checked against a term list**. `ontology` is checked because
it is two values and they are ours.

## What validates it

`yidam schema` publishes the shape, so an editor underlines a malformed field as it is typed.
Two lint checks gate on it:

| Check | Fails on |
|---|---|
| `foundational-type-malformed` | An `ontology:` outside `bfo\|ufo`, an empty `type:`, or an `iri:` that is not absolute |
| `foundational-field-misspelled` | `bfo_type:`, `ufo_type:` or `bfo_anchor:` — three spellings that are read by nothing |

The second one exists because those three spellings were real, and for a long time nothing said
so. `bootstrap.md` told authors to write `bfo_type:` in prose while its own template wrote
`foundational_type:`. The RDF export read a third field, `bfo_anchor:`, that nothing produced.
None of it went red. The class body accepts undeclared keys, so a wrong field name simply sat
there. And no corpus this repository ships declares an alignment, so the path was never
exercised. Issue #613 has the history.

If you are carrying one of those fields, the check names the repair. A `bfo_anchor:` URI moves to
`foundational_type.iri`.

## What reaches RDF

`yidam export-rdf` emits Turtle and JSON-LD with identical triple sets. A class carrying an
alignment gets:

```turtle
# illustrative — not a run
yidam:marriage a owl:Class ;
    rdfs:label "marriage" ;
    yidam:foundationalOntology "ufo" ;
    yidam:foundationalType "relator" ;
    skos:exactMatch <https://purl.org/nemo/gufo#Relator> .
```

`foundationalOntology` and `foundationalType` are emitted **whenever the field is present**.
`skos:exactMatch` is emitted only when `iri:` is. So an alignment without an IRI still exports
the fact that the corpus made the decision. That is what a consumer needs to know before asking
anything else.

A class with no `foundational_type:` emits none of these. That is the `none` answer, and it is
silent by design.

## Changing your mind

Adding, removing or changing an alignment is an edit to each `.ont.yml`. No instance moves, no
edge changes, and no index rebuild is required — the field is read at export time.

What does change is the RDF a consumer has already fetched. A corpus published as a `.yiz` bundle
carries its classes, so a downstream repository that installed yours sees the old alignment until
it updates. That is the ordinary versioning story, and
[sharing a derivation](../sharing-derivations.md) covers it.

## Related

| | |
|---|---|
| Why align at all | [Choosing an alignment](choosing-an-alignment.md) |
| What a class declares | [What an ontology is here](what-an-ontology-is.md) |
| The full file schemas | [Information architecture](../information-architecture.md) |
| The export surface | [Domain computer](../domain-computer.md) |
