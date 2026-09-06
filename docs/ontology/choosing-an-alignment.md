# Choosing a foundational alignment

*BFO, UFO, or none. The bootstrap dialogue asks this once, before any file is written, and the
answer shapes what your classes can be asked afterwards.*

Your ontology says what kinds of thing exist in *your* domain — gages, reaches, statutes,
recordings. A **foundational ontology** sits above that and says what kinds of thing exist at
all. It is a small, fixed vocabulary that domain classes anchor to.

The bootstrap dialogue offers three answers, and **none** is a real one.

## Why anchor to anything

A foundational ontology does one useful thing: it forces a distinction your own words hide.

English lets one noun cover two different kinds of thing, and a class list is made of nouns. When
you name a class, nothing asks whether the word is doing one job or two. An alignment asks.

The [genealogy walkthrough](../walkthroughs/genealogy.md) has the case worth reading in full. In
short: a corpus of parish records wants a `marriage` class. Under UFO's vocabulary, that word is
two things.

- **A wedding is an `event`.** It happened at a time and a place. It lasted an afternoon. Records
  attest it directly.
- **A marriage is a `relator`.** It is an entity whose existence depends on two people and is
  neither of them. It lasts decades. No record attests it directly.

They have different start dates, different end conditions, and different evidence. Naming the
alignment is what surfaces that. Without it, "marriage" is one word in a class list and nobody
notices it is two things.

That is the whole argument for alignment. It is not interoperability, though you get some; it is
that the vocabulary makes you decide something you would otherwise leave ambiguous.

## BFO — does it persist, or does it unfold?

**Basic Formal Ontology** organizes everything along one axis: does a thing exist at a moment, or
happen over an interval?

- **Continuants** persist through time and have no temporal parts. A machine, a site, a quality,
  a disposition, a role.
- **Occurrents** unfold through time and have temporal parts. A process, an event, a process
  boundary.

The test is whether the thing is wholly present right now. A gage is; the gauging is not.

**Best fit for scientific, empirical and physical-process domains**, where the object/event
distinction carries analytical weight. Distinguishing a machine from the machining process it
performs is the shape BFO is built to make easy.

Common type values: `continuant`, `occurrent`, `quality`, `disposition`, `role`, `process`,
`site`, `function`, `material entity`.

## UFO — what must it be, and what does it merely play?

**Unified Foundational Ontology** organizes around rigidity and relationality — what a thing
necessarily is, versus what it contingently does.

- **Kind** — what something necessarily is. If it stops being one, it ceases to exist as that
  thing. A person is a Kind.
- **Role** — what something contingently plays in a relational context. The same person is a
  witness at one wedding and a spouse at another.
- **Relator** — a first-class entity that mediates a relationship and has its own identity and
  properties. A marriage, an employment, a lease. Rather than a bare edge, a relator carries the
  history and terms of the connection.

**Best fit for institutional, enterprise and process-modeling domains**, where the same entity
plays different roles and where relationships themselves carry meaning worth querying.

Common type values: `kind`, `subkind`, `role`, `phase`, `relator`, `mode`, `quality`, `event`,
`situation`.

## None — and when it is right

Classes are typed by domain convention only. No `foundational_type:` field is written at all.

Choose this when foundational alignment is not a goal of the corpus, or when you want to commit
later. It is not a lesser answer, and **every corpus this repository ships takes it.**

The cost of choosing `none` is only that the distinction above never gets forced. If your domain's
nouns each do one job, that costs you nothing.

## How to choose

The dialogue presents these against two or three classes from your own confirmed sketch, not in
the abstract. That is the moment to test them.

Take a class whose name you are least sure about and ask:

1. **Is this thing wholly present at an instant, or does it unfold?** If that question is sharp
   and useful in your domain, BFO is doing work.
2. **Does the same thing play different parts in different relationships? Does a relationship
   here have its own properties?** If yes, UFO is doing work.
3. **Did neither question tell you anything you did not know?** Take `none`.

Do not choose an alignment because it sounds more rigorous. An alignment you cannot apply
consistently is a field that will be wrong on some classes and right on others, and nothing can
tell those apart.

## Deciding later

An alignment is one field per class. Adding it later is an edit to each `.ont.yml`, not a
migration — no instance moves and no edge changes.

The decision is recorded in `.yidam/decisions/ontology.yml` at genesis, including a `none`
answer and the reasoning behind it. Revisiting it later means writing a new decision that
supersedes that one, which is the same shape as any other change of mind in a corpus.

What you cannot get back is the argument you did not have. The classes named before the question
was asked keep whatever ambiguity they were born with until somebody re-opens them.

## Next

[Alignment in practice](alignment-in-practice.md) is the field itself: what to write, what
validates it, and what reaches RDF.
