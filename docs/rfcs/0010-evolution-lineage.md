# RFC-0010 — Evolution lineage: forking, parentage, and explicit baselines

- **Status:** Draft
- **Track:** G3 (governance)
- **Relates to:** RFC-0009 (resolution record schema), RFC-0011 (partial-sangha binding),
  `CONSTITUTION.md` Article III, `sangha/PROTOCOL.md`, `prelude/GRAPH.md`, `sangha/README.md`
- **Versioning layers touched:** template (protocol) + bootstrap protocol (record schema + branch
  convention)
- **Downstream reference case:** none — sangha-layer, applies to every derived repo.

## Summary

The resolution record and its surrounding prose quietly assume a **linear** chain of evolutions —
"a new rigpa supersedes the previous one" — yet nothing records *which* evolution a rigpa descends
from, and an elector's divergence from baseline is only ever an implicit git merge-base. This RFC
**permits forks** and makes lineage explicit: the evolution record gains a **`supersedes:`** field
naming its parent evolution(s), and `ma/*` branches gain an **explicit baseline declaration** rather
than an inferred merge-base. Article III's ethos is that history should state its own structure; a
linearity assumption is exactly the kind of implicit fact the rest of the system refuses to leave
implicit. If a sangha wants linearity, it should be a **declared constraint**, not the accident of a
missing field.

## Problem

**Rival syntheses are conceivable but unrepresentable.** Two electors could each cut a rigpa from the
same or overlapping tips, or inquiry could legitimately branch into two settled lines. The record
cannot express the relationship: the format is `evolution` / `date` / `tips:`
([`PROTOCOL.md:61-81`](../../sadhana/sangha/PROTOCOL.md#L61-L81),
[`information-architecture.md:107-120`](../information-architecture.md#L107-L120)), where `tips:`
lists the `ma/*` tips that were *read* — **not** any parent rigpa. An evolution's ancestry *among
evolutions* is nowhere in the record.

**The prose presumes one line.** "A new rigpa branch **supersedes the previous one** and becomes the
common baseline" ([`sangha/README.md:28`](../../sadhana/sangha/README.md#L28)); "Prior rigpa branches
are not deleted — they remain as provenance. The new `rigpa/<evolution>` is **the active baseline**"
([`PROTOCOL.md:85-87`](../../sadhana/sangha/PROTOCOL.md#L85-L87)). "The active baseline," singular,
only parses if there is one line.

**Divergence-from-baseline is implicit.** "An elector's `ma/*` branch may diverge freely from
`rigpa/*` after a resolution" ([`CONSTITUTION.md:57-58`](../../yidam/prelude/CONSTITUTION.md#L57-L58))
— but from *which* `rigpa/*`? Today the answer is inferred by merge-base against the sole active
baseline. With one baseline that works; the instant a fork exists, "which evolution does this branch
diverge from" has no stated answer. And this is precisely the kind of fact yidam otherwise insists on
making explicit: "the git history *is* the graph" ([`SCRIPTURE.md:15`](../../yidam/prelude/SCRIPTURE.md#L15)),
Article III preserves ancestry, the whole system's promise is that structure is legible rather than
inferred.

## Proposal

**1 — Permit forks.** The evolution lineage is a DAG, not a line. A rigpa may be cut from tips
overlapping another's; governance does not assume a single baseline. Linearity becomes a choice a
domain may declare, not a default baked into the schema's silence.

**2 — Record parentage in the evolution record** via a new `supersedes:` list naming the parent
`rigpa/<evolution>` this synthesis builds on or replaces:

```markdown
---
evolution: <name>
date: <YYYY-MM-DD>
synthesized-by: ma/<elector>          # from RFC-0009
supersedes:                            # NEW — parent evolution(s); absent/empty at genesis
  - rigpa/<prior-evolution>
tips:
  - ma/<elector>@<short-hash>
---
```

A genesis-level evolution omits `supersedes`. A fork names the shared parent. A reconciliation of two
forks names both — a merge-of-evolutions is just a resolution whose `tips` are drawn from branches on
both lines and whose `supersedes` lists both parents.

**3 — Declare the baseline on `ma/*` explicitly.** An elector states which evolution their branch
currently diverges from, rather than leaving it to merge-base inference. Recommended mechanism: a
`Baseline: rigpa/<evolution>@<short-hash>` trailer on the branch's working commits (git-native, no
new file, greppable, updated by an explicit act). The declaration is a *stated fact*: "this position
is measured against that evolution."

**4 — Linearity is opt-in.** A domain that wants a single line declares it as a constitution
extension or decision record ([`CONSTITUTION.md:62-70`](../../yidam/prelude/CONSTITUTION.md#L62-L70)),
so the constraint is visible and intentional — not an emergent property of an absent field.

The explicit baseline is not only for forks: it is the field **RFC-0011** reuses to answer "which
baseline binds whom." Abstainers from a partial resolution keep their declared baseline; adoption is
an explicit re-declaration. Two RFCs, one field.

## Migration & compatibility

Template (protocol) + bootstrap-protocol (schema + branch convention). `supersedes:` is additive; the
baseline trailer is a new convention needing no data migration, though a bootstrap rubric check may
assert its presence on `ma/*` tips. Existing resolutions are unaffected (none exist). Downstream, a
future graph traversal could consume `supersedes` to walk the rigpa DAG (as `find_reachable` walks
corpus edges); out of scope here.

## Alternatives considered

- **Implicit merge-base only (status quo).** Rejected: leaves lineage unstated, silently breaks the
  moment a fork exists, and contradicts the make-structure-explicit ethos the rest of the system is
  built on.
- **Enforce linearity in governance.** Rejected as the *default*: it forecloses legitimate rival and
  branched syntheses by fiat. Retained as an *opt-in* per-domain constraint (proposal point 4) for
  sanghas that genuinely want a single line — declared, not assumed.
- **Parentage in git only (merge commits), not the record.** Rejected: the record is the legible
  provenance surface a reader consults; raw git parent edges neither name evolutions nor distinguish
  "supersedes" (retire the parent) from "forks from" (parent stays live). The relationship needs a
  word, and the record is where words live.

## Open questions

- **`supersedes` vs. `parent`.** Does naming a parent mean the parent is *retired* (superseded) or
  that this line *coexists* with it (a fork)? These are different relationships and may want two
  fields — `supersedes:` (retires) and `derives-from:`/`parent:` (coexists). Lean: distinguish them;
  conflating retire and fork is how "the active baseline" became ambiguous in the first place.
- **Baseline drift on supersede.** When a `ma/*` branch's declared baseline is later superseded, does
  it auto-migrate or require explicit re-declaration? Lean: explicit re-declaration — consistent with
  RFC-0011, where adoption is a deliberate act, not a silent rebase.
- **Trailer vs. tracked marker.** A commit trailer is git-native but easy to omit; a tracked
  `.yidam/sangha/baseline` file is enforceable but adds a mutable file to a refs-first design. Lean:
  trailer, with a rubric check; revisit if omission proves common.
