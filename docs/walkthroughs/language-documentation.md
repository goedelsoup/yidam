# Language documentation walkthrough

*A documentation project can hold every recording, transcript and analysis it has ever made and
still not be able to say which of them it is allowed to publish — because the permission was
granted in a conversation and written down nowhere a machine can read.*

**This is a sketch.** There is no corpus and **no command output on this page** — every block
below is illustrative YAML showing the *shape*, not a transcript. What is runnable is the seed
set at
[`samudaya/examples/language-documentation/`](../../samudaya/examples/language-documentation/).

**No language, community, speaker or archive is named on this page, and that is deliberate.**
The subject here is consent, and illustrating a consent argument with a real community's
material would be indefensible. Speakers appear as codes because that is what an archive
actually does.

## The work, before a corpus

The material is recordings and what comes out of them: session notes, transcripts, interlinear
analyses, a lexicon, a grammar sketch. The conventions are strong — a session record carries
participants, date, place and equipment; an interlinear record carries a surface line, a
morpheme breakdown, a gloss line and a free translation; an archival deposit carries access
conditions.

Two things about that material are true at once and are usually stored very differently.

**It is data, and it accumulates in files.** A lexicon is a file. A transcript is a file.

**It is licensed, per recording, by people.** A speaker agreed to a session, on a date, for a
stated purpose. Another speaker agreed to different terms. A third agreed to recording but not
to publication of anything from it.

The second lives in a project lead's memory, in an email, and sometimes in a consent form in a
drawer. It does not travel with the transcript, so the constraint is intact exactly as long as
the person who negotiated it is still on the project.

## The ontology dialogue

**`elicitation`** — a recording session. Who spoke, who else was present, when, where, in what
setting, and under what agreement. Almost every property a linguist later conditions on lives
here rather than in the transcript, including whether the speaker was answering prompts or
narrating — elicited and naturalistic data carry different evidential weight for nearly any
claim, and a corpus that cannot tell them apart will average them.

**`lexeme`** — the unit a dictionary entry is about. Held separately from the attestations
supporting it and from any analysis of it.

**`speaker`** — a person who contributed recordings, as they wish to be identified. A class,
not a field, because the questions worth asking are about a speaker *across* sessions: how a
form varies for one person over a decade, which speakers a generalisation actually rests on,
whether an analysis describes a language or an idiolect.

**`etymology`** — a proposed origin for a lexeme, with the evidence offered.

### The class that was rejected: `consent`

`speaker` is a class. **`consent` is not**, and the difference is the whole page.

Consent is a property of an **elicitation** — scoped to a session, a date, and a stated
purpose. Hoisting it to a class of its own, or to a field on `speaker`, produces a corpus
carrying one blanket permission that covers recordings it was never granted for. It would read
as *this speaker consents*, when what happened is that a person agreed to a particular
recording for a particular purpose on a particular afternoon.

The failure is not hypothetical and it is not malicious. It is what happens when a project
gains a new member who reads the field, sees a permission, and quotes a 2019 session in a 2027
publication that nobody discussed with anybody.

```yaml
# illustrative — the shape, not a run
class: elicitation
label: Elicitation
description: |
  A recording session, and the terms it was made under.
properties:
  - name: recorded_on
    type: date
  - name: mode
    type: string
    description: Elicited or naturalistic — different evidential weight, not a detail.
  - name: permitted_uses
    type: text
    description: What was agreed, for this session, in the words it was agreed in.
  - name: withheld_uses
    type: text
    description: What was explicitly not agreed. Absent is not the same as none.
edges:
  - relationship: recorded
    target: speaker
    direction: out
```

Note `withheld_uses` sitting beside `permitted_uses`. An empty `withheld_uses` means *nothing
was withheld*; a **missing** one means nobody asked. Those are different states and the corpus
has to be able to hold both — which is the same discipline the
[incident walkthrough](incident-retrospectives.md) applies to a remediation nobody checked.

## The strongest habit in the example set, at higher stakes

`examples/streamflow/README.md` names it:

> **A source, and what it does not answer.** `catalog/usgs-nwis.md` spends as much space on
> what NWIS *cannot* tell you as on what it publishes. That section is the one most often left
> out of a catalog entry and the one most worth having.

For a stream gage, that section is about **epistemic limits** — the record cannot tell you the
rating curve. For a fieldwork recording it is about **permission**, and the same sentence
becomes much stronger: a catalog entry recording what a source may not be used for is doing
something no folder of audio files does, and the constraint is not advisory.

**A catalog that records only what a source answers will be read as licensing everything it
does not forbid.**

This is also where the derived artifacts matter more than they look. An index is a re-encoding
of the corpus and each row carries the node's text verbatim; a bundle carries more. So a
restriction that applies to a transcript applies to every artifact computed from it — which is
exactly what [`vault push`](../artifact-vaults.md) checks before anything leaves, and what the
[journalism walkthrough](investigative-journalism.md) shows refusing a document by name.

## The augmentation that has to outlive the dialogue

This is the one seed set where an `augmentation` carries `constitutional: true`, and the flag
is the argument.

A non-constitutional augmentation shapes one bootstrap and is destroyed with `samudaya/` at the
genesis commit. Archival ethics cannot be that. **The people who agreed to these recordings are
not party to the derived repository's later decisions**, and a norm that evaporates at genesis
protects them for exactly as long as nobody has started working yet.

So the rule is committed permanently into the derived repository's `CONSTITUTION.md`, where it
governs that repository's resolutions for its lifetime. `yidam samudaya-audit` flags any
constitutional augmentation before genesis and asks that it be checked against the
constitution's existing articles — a review that is a feature of this seed rather than an
obstacle to it. A rule governing a repository for its lifetime should be read by a person once.

Documentary linguistics has spent two decades building this into its archives: deposits carry
access conditions, and the CARE principles for Indigenous data governance make the point that a
permission to *hold* material is not a permission to *publish* it.

## Claims, honestly tagged

A contested etymology is the easiest case in any of these walkthroughs, because a corpus
reporting every etymology as settled is visibly wrong to any practitioner who opens it.

```yaml
# illustrative — not a run
class: etymology
label: Proposed origin — borrowing
description: |
  Proposed as a borrowing, on the phonological correspondence set given in the analysis and on
  the semantic field it shares with the donor candidate. [inference]

  A competing derivation treats it as inherited, and accounts for the same correspondence as a
  conditioned reflex. [inference]

  The two are incompatible and this corpus does not choose. [open] Deciding between them needs
  attestation from a period neither proposal has evidence from, and no such attestation has
  been located.
properties:
  claim_tag: open
```

Two proposals, both `[inference]`, and the resolution `[open]`. Not "we are unsure" — *these
two accounts are incompatible, and here is the evidence that would settle it.* The action item
is legible from the claim, which is the whole reason the tag is structural rather than
editorial.

## What the seed set gives you

[`samudaya/examples/language-documentation/`](../../samudaya/examples/language-documentation/)
holds seven files:

| Kind | What it seeds |
|---|---|
| `axiom` | `elicitation`, `lexeme`, `speaker`, `etymology` — and, in `speaker`, the argument that consent is not this class |
| `hint` | every analytical claim cites the attestations it rests on |
| `constraint` | a recording's access conditions bind everything derived from it |
| `augmentation` | **`constitutional: true`** — a source states what it may not be used for, permanently |

```sh
yidam clone ../my-documentation
cp -R samudaya/examples/language-documentation/*.md ../my-documentation/samudaya/
cd ../my-documentation && yidam samudaya-audit
```

The audit will flag the constitutional augmentation for review before genesis. Read it. It is
about to become an article governing that repository for its lifetime, and that is the point.

## What this sketch does not show

**It has no corpus, so it has no run.** Nothing here is command output and none of the YAML has
been linted. The worked walkthroughs — [property](property-research.md),
[journalism](investigative-journalism.md), [incidents](incident-retrospectives.md) — ship gated
corpora and real transcripts.

**It does not show interlinear data.** The structure of an interlinear record is a real
convention worth knowing — surface line, morpheme breakdown, gloss line, free translation, with
the standard gloss abbreviations — and filling one in would mean inventing language data. A
linguist excerpting invented data is a worse outcome than a reader not seeing an example.

**It does not model the community as a party.** Authority over material frequently rests with a
body rather than with an individual speaker, and that body's decisions are not a property of any
elicitation. Representing it properly is a genuine design problem this page does not attempt.

**It does not enforce anything.** `withheld_uses` is text, and no gate reads it. What the
apparatus enforces is one layer down — `.yidam/private-paths` and `redistributable` on an
artifact record are machine-actionable and are what `vault push` refuses on. The prose is what a
person has to read, and this page is arguing for writing it down, not for automating it.

**No real language, community, speaker or archive appears.** The conventions are real: the shape
of an elicitation-session record, the standard interlinear gloss abbreviations, the metadata
fields a language archive requires, and the CARE principles. The material is not.
