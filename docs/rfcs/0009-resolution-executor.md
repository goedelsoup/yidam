# RFC-0009 — Resolution execution authority and the `synthesized-by` record

- **Status:** Accepted
- **Track:** G2 (governance)
- **Relates to:** RFC-0008 (emergent claims), RFC-0012 (elector attestation), `CONSTITUTION.md`
  Articles II / III / VI, `sangha/PROTOCOL.md`, `docs/information-architecture.md`
- **Versioning layers touched:** template (protocol commentary) + bootstrap protocol (resolution
  record schema)
- **Downstream reference case:** none — sangha-layer, applies to every derived repo.

> **Settled 2026-09-04.** Both proposals shipped in #565 (`b531d39`), the RFC-0009 half of #274,
> measured against the one derived repository running a sangha before anything was built. The
> protocol now answers the execution question in a section of its own, "Who may execute one":
> "Any recognized elector, whether or not they called it" ([`PROTOCOL.md:53`](../../sadhana/sangha/PROTOCOL.md#L53)).
> The record format carries the field ([`PROTOCOL.md:201`](../../sadhana/sangha/PROTOCOL.md#L201)),
> mirrored in [`information-architecture.md:113`](../information-architecture.md#L113), with the
> Article II / III reconciliation stated beside it as this RFC argued it:
> "It is a record, not a rank" ([`PROTOCOL.md:224-229`](../../sadhana/sangha/PROTOCOL.md#L224-L229)).
>
> Two checks hold the field to the registry, and their split is the settlement's shape.
> `resolution-elector-unregistered` (Error) fires on "a `ma/*` branch the record names, as a tip
> or as the executor, that `electors.md` does not register" — set membership between two committed
> files, green on the real corpus at 70 of 70 tips. `resolution-executor-unrecorded` (Warn) fires
> on a record naming no executor — 29 of 29 existing records, where this RFC's compatibility
> section expected none to exist. Verified against that corpus rather than predicted: 0 errors,
> 29 warns.
>
> What shipped names a **seat**, never an identity. The field points at a `ma/*` row in
> `electors.md`, and the commit's own subject names the gap it closed — "the record names which
> tips were read and never who read them". It deliberately stops there. Measured, no identity is
> recoverable behind a seat: all 126 commits across the three elector branches carry the
> operator's git author, 0 of the repository's 1,070 commits are signed, and the co-author
> trailers name one model for all three seats, "because a trailer names what produced a commit
> rather than which seat held it". As the #274 split comment on #253 puts it: "The branch name is
> not the weakest discriminator between them — it is the only one." Attestation that could change
> that is the RFC-0012 half of #274, split to #566 on that same measurement.
>
> Three declines, each with its reason kept. Retrofitting the 29 records written before the field
> existed: they "cannot be fixed", because recalling which seat held the pen is exactly what the
> corpus cannot recover, and "Gating on a debt that cannot be paid is how a gate gets switched
> off." So the executor check warns, and its escalation condition is written into the check rather
> than left to taste: "when `electors.md` binds a distinct signing key per seat, the executor
> becomes recoverable from the commit and a missing field is a choice rather than an inheritance"
> (`b531d39`). Requiring `synthesized_by` in `report.schema.json`: declined, "a released CLI
> predates the field, so a consumer that assumes it is present is wrong against old output and
> against every record written before today". And a `called-by` field: declined as this RFC
> leaned — "Only the executor is recorded" ([`PROTOCOL.md:66`](../../sadhana/sangha/PROTOCOL.md#L66)).
>
> With the settlement recorded here, the header moves per #598's convention: **Accepted**, not
> `Implemented`, because `b531d39` is an ancestor of no released tag — not of `cli/v0.9.0`, not of
> the template's `v0.3.0` — and the legend reserves `Implemented` for work "landed and referenced
> by a released layer". Both open questions below are decided in place.

## Summary

The protocol says "any elector may **call** a resolution," but nothing says who may **execute**
one — who authors the synthesis commit that becomes a `rigpa/<evolution>`. That commit is the single
act Articles III–V exist to bound, and its author is unconstrained and unrecorded. This RFC ratifies
two things in one move: **any elector may execute** a resolution (preserving Article II's epistemic
equality — a designated-synthesizer role would be exactly the privilege Article II forbids), and the
resolution record grows a **`synthesized-by:`** field naming that author. Identity stays unprivileged
in *weight* (Article II) and becomes recorded in *provenance* (Article III). The same field is what
RFC-0012's attestation hangs on.

## Problem

Calling a resolution is specified. Executing one is not.
[`PROTOCOL.md:40-44`](../../sadhana/sangha/PROTOCOL.md#L40-L44): "Any elector may call a resolution
by — identifying a question or tension... naming the `rigpa/<evolution>` branch... notifying
participating electors." But the procedure that follows — Read, Synthesize, Open tensions, Commit
([`PROTOCOL.md:36-58`](../../sadhana/sangha/PROTOCOL.md#L36-L58)) — never names who performs it. The
person who commits the synthesis is "whoever holds the pen at settlement," and that pen carries the
full authority Articles III–V are written to constrain: it decides what the collective understanding
*is*, which tensions become open questions, and what the rigpa records.

So the constitution bounds a resolution's **content** — it must preserve ancestry
([`CONSTITUTION.md:37`](../../yidam/prelude/CONSTITUTION.md#L37)), must not silently discard tensions
([`:40-41`](../../yidam/prelude/CONSTITUTION.md#L40-L41)), must read legibly
([`:47-51`](../../yidam/prelude/CONSTITUTION.md#L47-L51)) — while saying nothing about the **identity**
of its author. Two gaps follow:

1. **May any elector execute, or is a designated / neutral synthesizer required?** Undefined.
2. **Is the executor recorded?** No. The record format captures `evolution`, `date`, and `tips:` —
   the `ma/*` tips that were *read* — but never the author who did the reading
   ([`PROTOCOL.md:61-81`](../../sadhana/sangha/PROTOCOL.md#L61-L81),
   [`information-architecture.md:107-120`](../information-architecture.md#L107-L120)). The
   highest-authority actor in the system is the one actor the provenance record omits.

## Proposal

**1 — Any elector may execute.** Add protocol commentary: any recognized elector may perform a
resolution, whether or not they called it. Article II forbids privileging an elector "by identity,
seniority, or the model that produced it" ([`CONSTITUTION.md:31`](../../yidam/prelude/CONSTITUTION.md#L31));
a standing designated-synthesizer role would manufacture precisely that privilege. Execution
authority is therefore universal, and bounded — not by *who* the executor is, but by Articles III–V,
which bind them the same regardless. Being the pen-holder is not a tiebreaker: if the executor's own
`ma/*` position is in tension with another's, that tension still becomes an open-question node
(Articles V, VI), never a claim the executor resolves in their own favor by fiat.

**2 — Record the executor.** Add `synthesized-by:` to the resolution record frontmatter:

```markdown
---
evolution: <name matching rigpa/<evolution> branch>
date: <YYYY-MM-DD>
synthesized-by: ma/<elector>        # NEW — the voice that authored this synthesis
tips:
  - ma/<elector>@<short-hash>
  - ...
---
```

This is the Article II / III reconciliation stated as one field. Article II keeps the author's
identity from conferring **weight** — being `synthesized-by` grants no standing, no tiebreak, no
privilege in any future resolution. Article III demands the author be part of the **record** —
"resolution must preserve the ancestry of synthesized knowledge"
([`CONSTITUTION.md:37`](../../yidam/prelude/CONSTITUTION.md#L37)), and the human or agent who
performed the synthesis is part of that ancestry. Recording who synthesized is the same ethic as
recording which tips were read: the settlement becomes auditable without becoming authoritative.

`synthesized-by` points at an elector — exactly the entity RFC-0012 proposes to attest (model,
version, configuration). Together they let a reader trace any rigpa to a named, attested voice while
that voice's model still confers nothing. This RFC supplies the pointer; RFC-0012 supplies the
target.

## Migration & compatibility

Template patch (protocol commentary) plus a bootstrap-protocol touch (the record schema gains a
field). Update the format block in [`PROTOCOL.md:61-81`](../../sadhana/sangha/PROTOCOL.md#L61-L81) and
the mirror in [`information-architecture.md:107-120`](../information-architecture.md#L107-L120); add
`synthesized-by` to any bootstrap rubric check on resolution records. The field is additive and
required going forward; existing resolutions are unaffected (none exist — `electors.md` is empty).

## Alternatives considered

- **Designated / neutral synthesizer per resolution.** Rejected: a standing role privileges an
  elector by position, contradicting Article II, and nothing says who would designate them. A
  *per-resolution* neutral chosen ad hoc is just "an elector executing" under a heavier name — which
  is this proposal, minus the role.
- **Caller must execute.** Rejected: it couples calling to authoring for no reason. The elector best
  placed to synthesize is often not the one who noticed the tension; let them differ, and record who
  actually did it.
- **Leave the author unrecorded.** Rejected: silent loss of the most consequential actor in the
  event. The record already names the tips read; omitting the reader is the one gap that makes a
  resolution un-auditable.

## Open questions

- **Co-synthesis.** Should `synthesized-by` be a list, so a jointly authored resolution names all its
  authors? Lean: yes — a list, singular being the common case. Joint authorship is real and cheap to
  represent. *Decided as leaned (#565): list-or-scalar —
  "one seat, or a list where a synthesis was genuinely joint" ([`PROTOCOL.md:224-225`](../../sadhana/sangha/PROTOCOL.md#L224-L225))
  — and a dropped seat is one of the mutations the tests hold, since downstream it reads exactly
  like a record that never named one.*
- **Caller vs. executor.** The caller (who named the branch and notified) and the executor may
  differ. Worth a distinct optional `called-by`, or is that ceremony the git history already carries?
  Lean: omit for now; revisit if calling ever acquires obligations beyond notification. *Decided as
  leaned (#565): omitted. The protocol says the two may differ, "and often should", and that
  "Only the executor is recorded" ([`PROTOCOL.md:65-67`](../../sadhana/sangha/PROTOCOL.md#L65-L67)).*
