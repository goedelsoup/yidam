# RFC-0009 — Resolution execution authority and the `synthesized-by` record

- **Status:** Draft
- **Track:** G2 (governance)
- **Relates to:** RFC-0008 (emergent claims), RFC-0012 (elector attestation), `CONSTITUTION.md`
  Articles II / III / VI, `sangha/PROTOCOL.md`, `docs/information-architecture.md`
- **Versioning layers touched:** template (protocol commentary) + bootstrap protocol (resolution
  record schema)
- **Downstream reference case:** none — sangha-layer, applies to every derived repo.

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
[`PROTOCOL.md:23-29`](../../sadhana/sangha/PROTOCOL.md#L23-L29): "Any elector may call a resolution
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
  represent.
- **Caller vs. executor.** The caller (who named the branch and notified) and the executor may
  differ. Worth a distinct optional `called-by`, or is that ceremony the git history already carries?
  Lean: omit for now; revisit if calling ever acquires obligations beyond notification.
