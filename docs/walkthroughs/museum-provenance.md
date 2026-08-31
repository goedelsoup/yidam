# Museum provenance walkthrough

*A registrar can have every document an object came with and still not be able to say where the
custody record stops — because the catalogue entry renders as one paragraph and a paragraph has
no holes in it.*

**This is a sketch.** There is no `examples/provenance/` corpus and **no command output on this
page** — every block below is illustrative YAML showing the *shape* a corpus would take, not a
transcript. What is runnable is the seed set at
[`samudaya/examples/museum-provenance/`](../../samudaya/examples/museum-provenance/).

Read this beside the [property research walkthrough](property-research.md). They are the same
shape at opposite stakes: one reader recognises a title search, the other restitution research,
and the ontology is nearly the same. If that one is closer to your work, start there — this
page assumes you can see the resemblance and spends its space on what differs.

## The work, before a corpus

Provenance is written as a line in a catalogue entry: a semicolon-separated sequence of holders
and dates, from the artist or the findspot to the present collection. It is a genuine
convention, it is compact, and it is a paragraph — which means **a gap in it looks exactly like
a semicolon.**

That matters more here than anywhere else in these walkthroughs, because in provenance the gap
is not missing metadata. It is the research result. An unexplained break in continental
European custody between 1933 and 1945 is the thing the file exists to surface, and museum
practice is explicit that publishing *"provenance unknown for this period"* is the expected
disclosure rather than an embarrassment to be tidied.

The paragraph cannot hold that. It can be written to mention the gap in prose, and often is —
by the careful, in a sentence that the next person summarising the entry drops.

## The ontology dialogue

**`object`** — the thing whose history is being established, identified by accession number
rather than by title. Titles move with attribution and translation; the accession number is
what the institution's own records key on.

**`custody-event`** — a transfer: a sale, gift, bequest, loan, seizure, restitution. With a
date, the parties, and the document recording it.

**`holder`** — a person or institution that held the object, as the documents name them.
Separate from the event, because the interesting questions are about a holder *across* objects
and across time, and a name inside a transfer record cannot be asked them.

**`document`** — what attests a transfer. A bill of sale, an auction catalogue entry, an export
licence, an accession register line.

### The class that was rejected: `gap`

It is the obvious one. The gap is the finding, so make the finding a node.

**No.** A gap is the *absence* of a documented transfer between two dated ones, and that is
derivable from the custody events themselves. Storing it creates two failures that the derived
form cannot have:

- **A stored gap can be asserted where the record does not entail one.** Somebody writes a gap
  node between two events that in fact join, and nothing contradicts it.
- **A stored gap can be closed by deleting a file.** In a domain where the gap is what the
  institution is supposed to be looking at, an interval that disappears from the record when
  somebody removes a node is the worst available failure mode.

Derived, the gap is exactly what the custody events say and nothing else. It moves when they
move, and it cannot be edited directly at all.

This is the third time this pattern appears in these walkthroughs, and the repetition is the
point: [`owner` is not a class](property-research.md) in title research,
[`root-cause` is not a class](incident-retrospectives.md) in incident review, and `gap` is not
a class here. **A stored conclusion cannot fail visibly, and a computed one can.**

### What the chain edge is for

Deriving the gap needs the sequence to be explicit — each custody event naming the one it
follows, rather than the order being inferred from dates:

```yaml
# illustrative — the shape, not a run
class: custody-event
label: Custody event
edges:
  - relationship: follows
    target: custody-event
    direction: out
    description: The custody event this one takes the object from.
  - relationship: transfers
    target: object
    direction: out
  - relationship: from-holder
    target: holder
    direction: out
  - relationship: to-holder
    target: holder
    direction: out
  - relationship: attested-by
    target: document
    direction: out
```

With `follows`, the gap is a **query**: a custody event with nothing following it, or two that
do not join. Ordering by date cannot express that question at all — two events six years apart
look exactly like two events six days apart — and a corpus that cannot ask the question ends up
storing the answer.

## Where `orphan-in` reads most clearly

An `object` that no custody event transfers is an uncited node. Everywhere else in yidam that is
a data-quality finding with a residence clock — see [the quality rubric](../quality-rubric.md).

Here it is the institution's central question, in the lint output. An accessioned object that no
custody event touches is an object whose history nobody has established, and the check that
reports it is not a nit about graph hygiene — it is the acquisitions backlog, counted.

That is the same check the [incident walkthrough](incident-retrospectives.md) shows holding an
exemption open for `incident`, where being uncited is the design. The check does not change; what
changes is what the class means, which is why `orphan-in` reads the whole ontology rather than one
class's edge list.

## The finding a paragraph cannot hold

Two claims that a provenance line renders identically:

```yaml
# illustrative — not a run
class: custody-event
label: Acquired by the museum, 1949
description: |
  Purchased from a dealer in 1949; the accession register records the price and the vendor.
  [verified]

  **The custody event this one follows is not established.** The vendor's stock was not
  catalogued and no bill of sale into the firm has been located, so nothing documents how the
  object reached the dealer. [open]
properties:
  claim_tag: open
links:
  - target: ../object/accession-1949-118.yml
    relationship: transfers
```

And the search that was done about it — which is **a document, not a gap node**:

```yaml
# illustrative — not a run
class: document
label: Negative search — dealer stockbooks, 1931–1949
description: |
  Four archives searched for a transfer into the dealer's stock between 1931 and 1949: the
  firm's surviving stockbooks, two auction-house sale records, and the national export licence
  series. No entry naming this object was found. [verified]

  This is a finding with an author, a date and a scope, and it is the only thing that
  distinguishes a gap somebody examined from one nobody has looked at. The gap itself is
  derived from the custody events and is not written down anywhere.
properties:
  claim_tag: verified
```

**That pair is the whole argument.** *No transfer is documented between 1931 and 1949* is a
property of the record, computed. *We searched four archives and found none* is a claim about
work that was done, authored. No absence can encode the second, and no node for the gap should
be asked to carry it — a gap node that quietly holds the search would be the stored conclusion
this ontology just refused.

## What the seed set gives you

[`samudaya/examples/museum-provenance/`](../../samudaya/examples/museum-provenance/) holds seven
files:

| Kind | What it seeds |
|---|---|
| `axiom` | `object`, `custody-event`, `holder`, `document` as irreducible kinds |
| `hint` | custody events chain explicitly, so the gap is a query |
| `constraint` | this corpus records custody, not authorship |
| `augmentation` | **record the search; derive the gap** |

```sh
yidam clone ../my-provenance
cp -R samudaya/examples/museum-provenance/*.md ../my-provenance/samudaya/
cd ../my-provenance && yidam samudaya-audit
```

A seed set seeds the discovery dialogue and does not replace it. Copy what is true of your work
and delete the rest.

## What this sketch does not show

**It has no corpus, so it has no run.** Nothing here is command output, and the YAML above has
never been linted. The worked walkthroughs — [property](property-research.md),
[journalism](investigative-journalism.md), [incidents](incident-retrospectives.md) — ship gated
corpora and real transcripts.

**It does not model two institutions citing each other.** Two museums holding parts of one
object's history, each citing the other's research without either editing it, is the right story
for this domain and it needs a second corpus. That is
[sharing a derivation](../sharing-derivations.md), and the walkthrough that demonstrates it is
[#456](https://github.com/goedelsoup/yidam/issues/456) rather than this page.

**It does not touch attribution or authenticity.** Who made the object, and whether it is what it
is said to be, are live scholarly arguments with their own standards of evidence. A corpus mixing
them with custody produces a graph where "the Rembrandt" and "the painting sold in 1923" are one
node under two contested descriptions.

**It does not model restitution outcomes.** A claim, its adjudication, and any resulting transfer
are custody events with a legal apparatus behind them that this ontology does not describe.

**No real institution, dealer or object appears.** The conventions are real — accession number
form, the semicolon-separated provenance line as catalogue entries print it, the shape of an
auction lot record, and the 1933–1945 disclosure practice. An invented provenance attributed to a
real museum or a real dealer would be a fabricated record about a real institution, and this
example ships to be copied.
