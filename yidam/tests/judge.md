---
name: judge
description: Evaluate a bootstrap result against the yidam rubric and produce a score report
---

# Skill: judge

**Held out from the repository under test.** This file used to live in
`yidam/prelude/skills/`, which is the one directory step 8 of the bootstrap skill vendors
into a derived repo — so every derived repository inherited a scorer for a test it can never
run, and the bootstrap skill's step 1 instructed the agent to read it before starting work.
The agent under evaluation held the criteria it was about to be scored against.

It lives here instead, with the rubric it applies and the harness that invokes it, and
`prepare_worktree` excludes `yidam/tests/` from the tree the bootstrap agent runs in. The
quality bar the agent legitimately needs is in the skill and in the conduct guidelines; the
bands, the criteria, and the scenario's reference description are the instrument, and an
instrument the subject can read measures the reading.

Invoked by the test harness after a bootstrap run completes. Reads the resulting repo state
and the test scenario, scores the result against [rubric.md](rubric.md), and
produces a structured quality report.

## Inputs

- The resulting repo state: corpus nodes, commit log, directory structure
- The scenario that seeded the domain owner
- The rubric

## Scoring bands

| Band | Meaning |
|---|---|
| `pass` | Criterion clearly met |
| `marginal` | Criterion partially met or ambiguous |
| `fail` | Criterion not met |

## What to assess

### Interrogation quality

Did the bootstrap agent ask clarifying questions before scaffolding? Look at the exchange
between the bootstrap and domain owner. A good bootstrap:
- Asks about the domain before naming any concepts
- Probes individual concepts for relationships and decomposability
- Confirms the ontology sketch before writing files

A bootstrap that jumps straight to scaffolding without dialogue fails this criterion.

### Corpus node quality

For each corpus node:
- Is it scoped to one concept? (fails if two or more concepts are fused)
- Is the content substantive — does it say something true and specific about the concept?
- Is the size appropriate? (a paragraph is right; a sentence is thin; multiple screens is too large)
- Does it have at least one meaningful edge — a link that reflects a real conceptual relationship,
  not a citation to a directory or README?

### Genesis commit quality

The genesis commit message should:
- Name the domain
- Describe the ontology — what the seed nodes are
- Note at least one edge (relationship between nodes)

A message that reads as boilerplate or lists only filenames fails this criterion.

### Ontology coherence

Looking at the seed nodes as a set: do they feel like the right starting points for this domain?
Are they at a consistent level of abstraction? Would a domain expert recognize them?

Use the scenario's `good_bootstrap_looks_like` field as the reference.

## Calibration

Bands are only comparable across runs if they mean the same thing in each. These anchors fix
what the middle band is for. They are drawn from a domain no scenario uses — bridge
inspection — because an anchor written in the domain under test is an answer key, and this
document is held out precisely so that it is not one.

The anchors are illustrative, not a target. A corpus that resembles them is not thereby good.

**Q3 — content is substantive and domain-specific**

| Band | Looks like |
|---|---|
| `pass` | *"A fracture-critical member whose failure would collapse the span; inspected at 24-month intervals under NBIS, hands-on rather than visual."* Specific enough to be wrong. |
| `marginal` | *"A structural member that is important to the bridge and needs regular inspection."* True, and true of almost anything. It names no interval, no standard, no failure mode. |
| `fail` | *"An important part of the bridge structure."* A restatement of the label. |

**Q4 — edges reflect real conceptual relationships**

| Band | Looks like |
|---|---|
| `pass` | `deck-joint → [admits water to] → bearing-assembly`. The relationship carries a mechanism; the two nodes are different kinds of thing and the edge says how they meet. |
| `marginal` | `deck-joint → [relates-to] → bearing-assembly`. The pair is real; the relationship name says nothing, so the edge asserts adjacency rather than knowledge. |
| `fail` | `deck-joint → [instance-of] → component.ont.yml`, and nothing else. Structural links only. The node is filed, not connected. |

**Q8 — edges assert only relationships the domain supports**

Q4 asks whether an edge is a relationship rather than a filing gesture. Q8 asks whether the
relationship is *true*. They are independent, and the combination that matters is a corpus
with a rich, mechanistic edge vocabulary that still asserts things the field does not hold —
every defensible edge around a wrong one lends it credibility.

| Band | Looks like |
|---|---|
| `pass` | Every edge states something a practitioner would accept, and the node body says why. Where the corpus is unsure it uses a weaker relationship rather than a bolder one. |
| `marginal` | One or two edges reach past what the corpus can support — a direction overstated, a condition dropped — but nothing that would mislead a reader about the domain's structure. |
| `fail` | An edge states a relationship the field contradicts, or states a conditional relationship as unconditional. A reader building on this graph would build on something false. |

Judge the *claim*, not the vocabulary. This is the criterion that needs domain knowledge rather
than the rubric, so quote the edge and the node text around it and say what is wrong with it. A
`fail` here is worth more to a reader than any other band here, and is also the easiest to
assert without grounds.

**Q6 — consistent level of abstraction**

| Band | Looks like |
|---|---|
| `pass` | Every seed is a class of thing, or every seed is a named specimen. One or the other, held throughout. |
| `marginal` | Mostly one level, with one or two nodes at the other, and the edges between the levels still read sensibly. |
| `fail` | Half the corpus is `inspection-regime` and half is `the 2019 inspection of the Third Street bridge`. Two corpora sharing a directory. |

A criterion whose evidence is thin should be `marginal`, not `pass`. `pass` means you could
show a domain expert the quoted evidence and they would agree without further context.

---

## Output format

Reply with a **single JSON object and nothing else**. No preamble, no summary after it.

```json
{
  "criteria": [
    {
      "id": "Q1",
      "evidence": ["what you are scoring, quoted"],
      "band": "pass",
      "rationale": "one or two sentences"
    }
  ],
  "overall": "marginal",
  "most_important_finding": "the single thing worth acting on"
}
```

Three rules the harness enforces, and rejects the verdict for breaking:

**Evidence before band.** Fill `evidence` first, then decide `band`. Quote the node text, the
commit line, or the transcript fact the band rests on. A verdict written before its evidence
is a verdict the evidence was assembled to support.

**Evidence even for absence.** A criterion can fail because something is missing — *"no
assistant turn precedes the first Write"* is evidence, and states what is absent. An empty
list is not; it is a band answerable to nothing, and the harness refuses it.

**Every criterion, exactly once**, in ID order. Six of seven is not six passes and a gap. Do
not revise a band once you have moved past it — a criterion re-scored in light of a later one
is scored against the corpus's overall impression rather than against itself.
