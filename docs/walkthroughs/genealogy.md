# Genealogy walkthrough

*A genealogist can hold four records naming the same person and no way to say that two of them
disagree — because the software asked for a birth year and took the last one typed.*

**This is a sketch.** There is no `examples/genealogy/` corpus and **no command output on this
page** — every block below is illustrative YAML showing the *shape* a corpus would take, not a
transcript of a run. What is runnable is the seed set at
[`samudaya/examples/genealogy/`](../../samudaya/examples/genealogy/), which you can copy into a
repository and bootstrap from.

## The work, before a corpus

The material is records: parish registers, census schedules, vital records, headstones, a
family bible. Each was created at a particular time by somebody with a particular reason to
know, and that provenance is the whole of its evidentiary weight — an enumerator walking a
district in 1880 and a descendant answering a question in 1940 are not equally good witnesses
to a birth in 1849.

Genealogy software models a person with fields. Birth year is a field. So when the parish
register says 1849 and the census implies 1851, the interface offers one box, and whoever is
typing picks. The disagreement — which is the actual state of the evidence, and often the most
interesting thing on the page — becomes a number with no history and no dissent.

The conventional workaround is a research log kept beside the tree: a prose document recording
what was searched, what was found, and what conflicts. It is written by the careful and it is
not machine-readable, so nothing built on the tree can see it.

## The ontology dialogue

Bootstrap asks what the irreducible kinds are, what relates them, and what is out of scope.
This domain's answers, and the argument that produced them.

**`person`.** Not a record. Two baptismal entries, a census line and a headstone may attest one
person or two people who shared a name in one parish, and deciding which is the research. A
person has to be something records point *at*.

**`record`.** The artifact, with its creator, date and reason to know. Separate from the person
and separate from the fact it asserts.

**`event`.** A birth, a marriage, a death, an enumeration. This is the class the dialogue has
to argue for, and the argument is below.

### The hard call: is a marriage a node or an edge?

The obvious model is an edge: `person -married-to-> person`. It reads correctly, it is what a
family tree draws, and it is wrong for a reason that has nothing to do with taste.

**An edge cannot be attested.** A marriage has a date, a place, a document that records it, and
frequently two documents that disagree about the date. An edge has nowhere to put any of that.
Modelling it as an edge does not simplify the corpus — it moves the evidence somewhere it
cannot be cited, and then loses it.

So a marriage is a node. Which node is the question a foundational alignment forces you to
answer out loud.

### Where `foundational_type` stops being a field

If the bootstrap dialogue chose an alignment, every class carries one
([information architecture](../information-architecture.md) has the schema):

```yaml
# illustrative — the shape a class file would take, not a run
class: event
label: Event
foundational_type:
  ontology: ufo
  type: event
description: |
  Something that happened at a time and a place, and the thing records attest.
```

UFO's vocabulary makes a distinction the English word hides. **A wedding is an `event`** — an
occurrence, with a duration of an afternoon. **A marriage is a `relator`** — an entity whose
existence depends on two people and which is neither of them, with a duration of decades. They
have different start dates, different end conditions, and different records attesting them.

Naming the alignment is what surfaces that. Without it, "marriage" is one word in a class list
and nobody notices it is two things.

**This corpus models the event and entails the bond**, and that is a decision with a cost worth
writing down. Every record attests an *occurrence* — a ceremony took place, an enumeration
recorded a household — and no record attests a bond directly. A `marriage` relator class would
therefore be a class whose instances no record supports, populated by inference and looking
exactly like the attested classes beside it.

The cost is real: "were they married in 1873" is answerable, and "were they married" is a
traversal and a judgement rather than a field. That is the same trade the
[property walkthrough](property-research.md) makes when it refuses an `owner` class, and for
the same reason — **a stored conclusion cannot fail visibly, and a computed one can.**

A different corpus could decide otherwise, and the point of the alignment is that it would have
to *decide*.

## The corpus, in outline

Three classes and the edges between them:

```yaml
# illustrative — not a run
class: record
foundational_type: { ontology: ufo, type: kind }
edges:
  - relationship: attests
    target: event
    direction: out
    description: An event this record asserts happened.

class: event
foundational_type: { ontology: ufo, type: event }
edges:
  - relationship: involves
    target: person
    direction: out
    description: A person this event happened to or concerned.
```

Note what is *not* there: no edge from `person` to anything. A person node carries a name and
nothing else that a record could contradict. Everything assertable hangs off events, and every
event hangs off the records attesting it.

## Claims, honestly tagged — and the disagreement

This is the shortest argument for claim tags anywhere in these walkthroughs, and it costs no
corpus to make.

Two records disagree about a birth year. The parish register, created within days by a clerk
with a reason to know, says 1849. The 1880 census, taken thirty-one years later from whoever
answered the door, implies 1851.

Genealogy software picks one. The corpus does this:

```yaml
# illustrative — not a run
class: event
label: Birth of Ellen Marbury
description: |
  The parish register records a baptism on 4 March 1849, which is evidence of a birth shortly
  before. [verified]

  The 1880 census schedule gives an age implying a birth year of 1851. [verified]

  These are incompatible and the corpus does not choose between them. [open] The register is
  the stronger witness — created within days, by a clerk with a duty to record — and the
  census age was reported by an unknown household member thirty-one years later. That is an
  argument for 1849 and it is not a resolution; a resolution requires a third record, and none
  has been found.
properties:
  claim_tag: open
```

**The disagreement is a first-class object.** It has a node, it carries both attestations, and
`yidam open-questions` lists it. A gate can see it. Somebody reviewing the work three years
later can see it, without reading a research log that may not exist.

And the tag says something precise: not *"we are unsure"* but *"two records disagree and no
third has been found."* The action item is legible from the claim.

## What the seed set gives you

[`samudaya/examples/genealogy/`](../../samudaya/examples/genealogy/) holds seven files — the
commitments above, written as bootstrap input rather than as prose:

| Kind | What it seeds |
|---|---|
| `axiom` | `person`, `record` and `event` as irreducible kinds |
| `hint` | records attest in one direction; **a marriage is not an edge** |
| `constraint` | the corpus stops at the living |
| `augmentation` | a contradiction between records is `[open]`, not an error to fix |

Copy it into the repository being bootstrapped, not this one:

```sh
yidam clone ../my-genealogy
cp -R samudaya/examples/genealogy/*.md ../my-genealogy/samudaya/
cd ../my-genealogy && yidam samudaya-audit
```

Then run the bootstrap. **A seed set seeds the discovery dialogue and does not replace it** —
these files state what the domain's practitioners commit to before any dialogue happens, and
the bootstrap is still required to ask. Copy the ones that are true of your work and delete the
rest: a seed you did not mean is worse than one you did not write, because the dialogue will
argue for it and you will have to argue back.

## What this sketch does not show

**It has no corpus, so it has no run.** Nothing on this page is command output, and the
`[open]` birth-year node above has never been linted. The two worked walkthroughs —
[property research](property-research.md) and
[investigative journalism](investigative-journalism.md) — ship gated corpora and real
transcripts; this one ships an argument and an executable seed set.

**It does not model the relator.** The marriage-as-relator question is argued above and
resolved one way. A corpus that resolved it the other way would need a fourth class and a
different account of what its instances rest on, and that account is not written here.

**It does not handle identity.** *Are these two records about the same person* is the central
labour of genealogy, and the corpus above gives it a place to live — a `person` node is where
the decision is recorded — without saying how the decision is made. That is method, and it
belongs in a skill.

**No real person appears, living or dead.** Ellen Marbury is invented. The conventions are
real: the column set of an 1880 census schedule, parish-register field shapes, and the citation
form for a vital record. A real dead person has living descendants, and a corpus asserting
contested things about them ships here to be copied.
