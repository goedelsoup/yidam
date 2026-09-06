# RFC-0012 — Elector identity and attestation

- **Status:** Accepted
- **Track:** G5 (governance)
- **Relates to:** RFC-0009 (`synthesized-by` points at the attested elector), `CONSTITUTION.md`
  Articles II / III, `sangha/electors.md`, `sangha/PROTOCOL.md`, `VERSIONING.md`
- **Versioning layers touched:** template (`electors.md` schema + protocol) + bootstrap protocol
  (registration check)
- **Downstream reference case:** one derived repository, 126 elector commits — measured, see the
  amendment below.

> **Amended 2026-09-04.** This RFC claimed signing makes a position *"cryptographically
> attributable rather than a bare branch name."* Measured against the one repository that has run
> a sangha ([#566](https://github.com/goedelsoup/yidam/issues/566)), that sentence is false in
> both available shapes: 126 commits across three elector branches carry one git author, none of
> the repository's 1,070 commits is signed, and under one operator **one key attests the operator**
> — it distinguishes exactly nothing the branch name did not — while **three keys attest a
> convention** about which key was used for which seat, the branch name's trust model wearing a
> fingerprint. The sentence is retracted. Signing is re-aimed at what it buys under any number of
> operators — **integrity** and **third-party verification** — and the fingerprint column is
> replaced by a trust root: `electors.md` generates the allowed-signers file, so verification is a
> real check rather than a column nothing consults. What signing cannot establish — what an agent
> elector *is*, what persists across cold instances, what independence requires — is deferred to
> E8 by name, under **Deferred to E8** below, so that E8's amendment extends this one rather than
> reworking it.

> **Built 2026-09-05** ([#566](https://github.com/goedelsoup/yidam/issues/566)). All four
> proposals ship against the amendment above, and nothing in them was re-argued. `electors.md`
> carries `Kind | Model | Version | Config | Key`, read by header rather than by position, so a
> registry written before the columns is read exactly as it was. The registry generates the
> allowed-signers file at verification time — one principal per keyed seat, the principal being
> the seat's `ma/*` branch rather than a committer email, which is what gives a per-seat answer
> in a repository whose seats share one git identity — and `elector-signature-unverified` gates
> on it. The `Key` column holds the public key, not the fingerprint the original table printed:
> a fingerprint is a record nothing consults, and the check says so when it finds one.
> `resolution-executor-unrecorded` now takes its severity from the condition written into it:
> every seat keyed, no two keys the same. A shared key is legitimate and does not arm it, which
> is the amendment's own point about one operator restated as a predicate. Every check is
> vacuous where no row binds a key, which is every corpus today, and a test asserts that the
> silence is the check running rather than the check missing.

## Summary

`electors.md` records only `Name | Branch | Role`. An agent elector's registration records nothing
about *what it is* — model, version, configuration — and `ma/*` commits are not signed per elector,
so "who holds this position" is an unverifiable string. Article II says the producing model confers
no privilege, and that is often misread as a reason not to *record* it. This RFC records it: add
**model / version / configuration** columns to `electors.md` and **wire commit signing to electors**
(the toolchain already signs release tags) — signing bought for integrity and third-party
verification, with the registry as the trust root, and claimed for nothing more. The framing is
the one the audit named: **Article II governs weight, Article III governs record.** Recording what
produced a position grants it nothing.

## Problem

**The registry is minimal.** `electors.md` was a three-column table — `Name | Branch | Role`,
and the columns below are what it carries since this RFC was built. Registration
([`PROTOCOL.md:15-21`](../../sadhana/sangha/PROTOCOL.md#L15-L21)) is: open a `ma/<name>` branch, be
added to `electors.md`, be included in a first resolution. Nothing records what an agent elector is,
and nothing binds a commit to an identity — a `ma/<name>` branch and a table row are both forgeable
strings.

**Article II is a rule about weight, not record.** "No elector's position is privileged by identity,
seniority, or the model that produced it"
([`CONSTITUTION.md:31`](../../yidam/prelude/CONSTITUTION.md#L31)). This governs how much a
position *counts*. It is silent on whether the producing model is *recorded* — and the
provenance-first ethic argues it must be: "resolution must preserve the ancestry of synthesized
knowledge" ([`:37`](../../yidam/prelude/CONSTITUTION.md#L37)), and the scripture's whole claim is that
"every node traces back to the conversation that produced it and the understanding that justified
it" ([`SCRIPTURE.md:87`](../../yidam/prelude/SCRIPTURE.md#L87)). The one provenance the system omits is
the provenance of its own actors.

**The gap is concrete.** RFC-0009's `synthesized-by` and the record's `tips` point at electors. When
an elector is an agent, a reader auditing the ancestry (Article III) cannot tell whether a position
was held by `claude-opus-4-8` under configuration X or something else entirely — the exact provenance
the system otherwise obsesses over. Meanwhile the signing infrastructure already exists: release tags
are signed — [`release.sh:341`](../../release.sh#L341) runs `git tag -s` and refuses the release
outright when `user.signingkey` is unset, with SSH signing the configured format. It simply is not
wired to
elector commits.

## Proposal

**1 — Attest the elector in `electors.md`.** Extend the registry so an agent elector's row records
what produced it:

```markdown
| Name    | Branch      | Role       | Kind  | Model            | Version | Config    | Key                |
|---------|-------------|------------|-------|------------------|---------|-----------|--------------------|
| aria    | ma/aria     | investigator | agent | claude-opus-4-8 | 4.8     | sha256:…  | ssh-ed25519 AAAA…  |
| j. okafor | ma/okafor | domain lead  | human | —                | —       | —         | ssh-ed25519 AAAA…  |
```

Human electors leave the agent fields blank. `Config` is a hash of the agent's operative
configuration, not the config itself (see open questions).

`Key` holds the public key, in `authorized_keys` form. *(Corrected 2026-09-05; this column read
`Key (fpr)` with a `SHA256:…` example, which the amendment's own trust-root design cannot use — an
allowed-signers file needs the key, and a fingerprint is exactly the column nothing consults that
the amendment replaced.)*

**2 — Sign elector commits, for what a signature establishes.** *(Revised 2026-09-04; the original
claimed attribution, and the measurement retracted it.)* Each elector's `ma/*` commits are signed
(SSH signing, the format the toolchain already uses for tags) with a key bound to the seat in the
registry. A signature establishes two things, and this RFC now claims only those:

- **Integrity.** The commit is the bytes the key-holder produced, unaltered.
- **Third-party verification.** A reader outside the repository can check, from the registry
  alone, that a position's commits verify against the key its seat declares.

What it does not establish, under one operator, is attribution between seats. One key across three
seats attests the operator and distinguishes nothing the branch name did not; three keys under one
operator attest a convention about which key was used for which seat. `electors.md` says so in the
same register the constitution uses: a recorded key grants a seat nothing (Article II), and under a
single operator it does not distinguish seats from one another at all.

**The registry is the trust root.** `git verify-commit` reads `gpg.ssh.allowedSignersFile`; that
file is *generated from* `electors.md` at verification time — one principal per seat, never
committed separately, so the registry and the trust anchor cannot drift. A key absent from the
registry verifies nothing; a seat row without a key declares its commits unverifiable, and the
check finds nothing to verify. This replaces "record the fingerprint": a `Key` column something
consults on every verification, rather than one nothing does.

**What this arms.** `resolution-executor-unrecorded` warns today, and its escalation condition is
written into the check itself: it becomes an **Error** when `electors.md` binds a distinct signing
key per seat, because the executor is then recoverable from the commit and a missing
`synthesized-by:` is a choice rather than an inheritance. This proposal is that condition; landing
it makes the escalation a decidable event rather than a promise.

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

## Division of labor: registry and receipt *(added 2026-09-04)*

The registry attests **who holds a seat** — a standing fact: name, branch, kind, model, version,
config hash, key. An RFC-0026 **receipt** attests **what ran** — a per-event fact: input sha,
image digest, model, seed, outputs. Neither substitutes for the other. A receipt cannot say who
holds the auditor's seat next month; a registry row cannot say which model and seed produced this
position today. The alternative this RFC rejected — duplicating the elector's identity into every
resolution record — stays rejected, and the receipt is not that alternative: it records execution
facts, not seat identity, and `synthesized-by:` still points at the registry row. When E8 asks how
independence becomes checkable ([#295](https://github.com/goedelsoup/yidam/issues/295)), both
substrates answer, each for its own half: the registry says the seats are distinct standings; the
receipt says the runs were distinct events.

## Where verification runs *(decided 2026-09-04)*

No workflow in `.github/workflows/` configures a signing key — and none needs one, because
verification consumes only public material: the allowed-signers file derived from the registry.
The venue question is therefore not *where is the secret* but *what makes the check conditional*.
The condition is the registry's own declaration: a seat's commits are verified when, and only
when, its row binds a key. That condition is repository content, so the check returns the same
answer in every venue — which is what lets it live in the standing lint the PR gate already runs:
vacuous in a corpus with no keyed seats (most, since collective mode is opt-in), armed by the
commit that lands a key in the registry. The alternative — a non-PR venue: verification at
resolution time only, or on a schedule — is declined, because it would let a signature that stops
verifying sit unnoticed between resolutions, and it reintroduces exactly the venue-dependence the
registry-derived condition removes. What stays out of CI is signing itself: no workflow signs an
elector commit, nothing here sets `tag.gpgsign`, and the scratch-repository configs that keep it
off stay true.

## Cluster dispatch *(constraint added 2026-09-04; mechanism deferred to #475)*

[#475](https://github.com/goedelsoup/yidam/issues/475)'s design has pods compute commits and emit
shas while one lander holds the only ref-write credential, re-parenting and retrying on a
compare-and-swap conflict. A signature covers the whole commit object, parents included — so a
lander that re-parents produces an object the pod never signed, and dispatch as designed would
invalidate every pod-side signature it retries. The constraint this RFC places on that design is
one sentence: **a dispatched position is either signed pod-side over its final parentage before
the sha is emitted, or its executor record is the run's receipt rather than a signature.** Which
of the two — pod-side signing with the retry moved into the pod, or the receipt formally standing
in as the executor record for dispatched runs — is #475's mechanism to choose, not this RFC's.

## Deferred to E8 *(#293, #294, #295)*

Three questions this amendment deliberately does not answer, so that E8's amendment extends this
one rather than reworking it:

- **What an agent elector is** — what signs: the model, the operator who ran it, the harness, a
  specific run? Each answers a different accountability question and only one can be verified from
  a commit. [#293](https://github.com/goedelsoup/yidam/issues/293)'s to answer; the registry
  columns here are the record its answer will fill, not the answer.
- **What persists across cold instances** — a key can outlive a session, but a standing position
  is more than a keypair: a `ma/*` branch maintained by successive cold instances may be many
  processes sharing a branch name, and now a key.
  [#294](https://github.com/goedelsoup/yidam/issues/294)'s to answer.
- **What independence requires** — non-privilege is not independence, and neither is a signature:
  three keys under one operator are the measured case in the amendment above.
  [#295](https://github.com/goedelsoup/yidam/issues/295)'s to answer, with both substrates from
  the division of labor as its evidence.

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
  next) but not the end state. *(2026-09-04: what phase two adds is narrower than the original
  implied — integrity and third-party verification, not attribution between seats one operator
  runs.)*
- **Put attestation in the resolution record instead of `electors.md`.** Rejected: an elector's
  identity is a standing fact about the elector, not a per-resolution one. The resolution's
  `synthesized-by` (RFC-0009) should *point at* the elector row, not duplicate the attestation into
  every record. *(2026-09-04: RFC-0026's per-run receipt is not this alternative — it records
  execution facts, not seat identity; see the division of labor above.)*

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

  > *Settled 2026-09-04.* SSH signing, and verification is neither resolution-time-only nor a mere
  > record: it runs in the standing lint, conditional on the registry binding a key — see **Where
  > verification runs** above. The Warn→Error escalation of `resolution-executor-unrecorded` is
  > the "hard gate later", and its trigger is written into the check.
