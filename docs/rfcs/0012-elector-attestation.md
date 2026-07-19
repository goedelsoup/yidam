# RFC-0012 — Elector identity and attestation

- **Status:** Draft
- **Track:** G5 (governance)
- **Relates to:** RFC-0009 (`synthesized-by` points at the attested elector), `CONSTITUTION.md`
  Articles II / III, `sangha/electors.md`, `sangha/PROTOCOL.md`, `VERSIONING.md`
- **Versioning layers touched:** template (`electors.md` schema + protocol) + bootstrap protocol
  (registration check)
- **Downstream reference case:** none — sangha-layer, applies to every derived repo.

## Summary

`electors.md` records only `Name | Branch | Role`. An agent elector's registration records nothing
about *what it is* — model, version, configuration — and `ma/*` commits are not signed per elector,
so "who holds this position" is an unverifiable string. Article II says the producing model confers
no privilege, and that is often misread as a reason not to *record* it. This RFC records it: add
**model / version / configuration** columns to `electors.md` and **wire commit signing to electors**
(the toolchain already signs release tags). The framing is the one the audit named: **Article II
governs weight, Article III governs record.** Recording what produced a position grants it nothing.

## Problem

**The registry is minimal.** `electors.md` is a three-column table
([`electors.md:8-10`](../../sadhana/sangha/electors.md#L8-L10)): `Name | Branch | Role`. Registration
([`PROTOCOL.md:15-21`](../../sadhana/sangha/PROTOCOL.md#L15-L21)) is: open a `ma/<name>` branch, be
added to `electors.md`, be included in a first resolution. Nothing records what an agent elector is,
and nothing binds a commit to an identity — a `ma/<name>` branch and a table row are both forgeable
strings.

**Article II is a rule about weight, not record.** "No elector's position is privileged by identity,
seniority, or the model that produced it"
([`CONSTITUTION.md:21-25`](../../yidam/prelude/CONSTITUTION.md#L21-L25)). This governs how much a
position *counts*. It is silent on whether the producing model is *recorded* — and the
provenance-first ethic argues it must be: "resolution must preserve the ancestry of synthesized
knowledge" ([`:29`](../../yidam/prelude/CONSTITUTION.md#L29)), and the scripture's whole claim is that
"every claim traces back to the conversation that produced it and the understanding that justified
it" ([`SCRIPTURE.md:87`](../../yidam/prelude/SCRIPTURE.md#L87)). The one provenance the system omits is
the provenance of its own actors.

**The gap is concrete.** RFC-0009's `synthesized-by` and the record's `tips` point at electors. When
an elector is an agent, a reader auditing the ancestry (Article III) cannot tell whether a position
was held by `claude-opus-4-8` under configuration X or something else entirely — the exact provenance
the system otherwise obsesses over. Meanwhile the signing infrastructure already exists: release tags
are signed (`git tag -s`, [`VERSIONING.md:118`](../../VERSIONING.md#L118)). It simply is not wired to
elector commits.

## Proposal

**1 — Attest the elector in `electors.md`.** Extend the registry so an agent elector's row records
what produced it:

```markdown
| Name    | Branch      | Role       | Kind  | Model            | Version | Config    | Key (fpr)    |
|---------|-------------|------------|-------|------------------|---------|-----------|--------------|
| aria    | ma/aria     | investigator | agent | claude-opus-4-8 | 4.8     | sha256:…  | SHA256:…     |
| j. okafor | ma/okafor | domain lead  | human | —                | —       | —         | SHA256:…     |
```

Human electors leave the agent fields blank. `Config` is a hash of the agent's operative
configuration, not the config itself (see open questions).

**2 — Sign elector commits.** Each elector's `ma/*` commits are signed (SSH signing is simplest) with
a key bound to the elector via the `Key` column. This makes "who committed this position"
cryptographically attributable rather than a bare branch name, and reuses the signing the toolchain
already performs for tags.

**3 — Frame it exactly as the audit did.** **Article II governs weight; Article III governs record.**
Recording what produced a position — model, version, config hash, signing key — grants it nothing: no
extra vote, no tiebreak, no priority in any resolution. It makes the position *auditable*. The two
articles are not in tension; they answer different questions — *how much does this count* (II, answer:
the same as anyone's) versus *what is it and where did it come from* (III, answer: recorded). A
one-line note in `electors.md` should say so, so a reader never mistakes a recorded model for standing.

**4 — Registration records attestation.** Amend PROTOCOL registration: an agent elector records its
model/version/config at registration; a material change (model upgrade, config change) is recorded as
an update, so the ancestry of a position includes the state of the agent that held it. This is the
same move that answers RFC-0009 — `synthesized-by` points at a now-attested row.

## Migration & compatibility

Template (the `electors.md` template + PROTOCOL registration commentary) plus a bootstrap-protocol
touch (a registration/attestation check). Signing reuses existing tooling; the `Key` binding is new.
Backward compatible: human electors need no agent fields, and existing registries (empty today) are
unaffected. Adding the columns is a schema change to a governance file, not to the corpus node model,
so it does not touch the parity surface.

## Alternatives considered

- **Record nothing (status quo).** Rejected: silent loss of the actors' provenance — the single
  provenance gap in a system built on provenance.
- **Record model but don't sign.** Weaker: a table row and branch name are forgeable, so attestation
  without signing is attestation in name only. Acceptable as a *phase one* (columns now, signing
  next) but not the end state.
- **Put attestation in the resolution record instead of `electors.md`.** Rejected: an elector's
  identity is a standing fact about the elector, not a per-resolution one. The resolution's
  `synthesized-by` (RFC-0009) should *point at* the elector row, not duplicate the attestation into
  every record.

## Open questions

- **Configuration granularity.** Full agent configuration is large and volatile. The minimal
  meaningful attestation is likely `model + version + config-hash`; is a hash sufficient, and what is
  in scope for the hash (system prompt, tool set, sampling params)? Recording too much creates churn;
  too little defeats the audit.
- **Re-attestation churn.** Does every model upgrade require a new row/commit? Lean: record material
  changes only, with the config-hash making "material" detectable.
- **Signing scheme and enforcement.** SSH vs. GPG, and does the harness *verify* signatures on `ma/*`
  tips at resolution time or merely record the key? Lean: SSH signing; verification as a rubric check
  first, a hard gate later.
