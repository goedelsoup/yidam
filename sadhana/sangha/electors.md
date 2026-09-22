# Electors

Recognized participants in this repository's sangha. Each elector maintains a
`ma/<name>` branch as their working position.

See [PROTOCOL.md](PROTOCOL.md) for how to register as a new elector.

**An elector is a seat.** A `ma/<name>` branch together with the row below that describes it —
that pair is the elector, and it is what every mechanism here already points at. An *agent*
elector is a seat whose `Kind` is `agent`. A model, an operator, a harness and a run are
**occupants** of a seat: recorded on it, and none of them is the elector. The distinction is not
a nicety. It is what makes "who holds this position" answerable at all, because a seat persists
and an occupant does not.

| Name | Branch | Role | Kind | Model | Version | Config | Key | Seated |
|------|--------|------|------|-------|---------|--------|-----|--------|
| *(no electors registered yet)* | | | | | | | | |

**Recording what produced a position grants it nothing.** Article II governs weight — no
elector's position is privileged by identity, seniority, or the model that produced it —
and Article III governs record. A recorded model, version, config hash or signing key buys
no vote, no tiebreak, and no priority in any resolution; it makes the position auditable,
which is the only thing it is for.

## The columns

`Name`, `Branch` and `Role` are the registry as it has always been. The rest are RFC-0012's
attestation, and every one of them is optional — a registry that fills none of them in is
read exactly as it was before they existed.

- **`Kind`** — `agent` or `human`. A human elector leaves the next three blank, written `—`.
- **`Model`**, **`Version`** — what produced this seat's positions, so a reader auditing the
  ancestry can tell `claude-opus-4-8` under one configuration from something else entirely.
- **`Config`** — a *hash* of the agent's operative configuration, never the configuration.
- **`Key`** — the seat's SSH public key, in `authorized_keys` form: `ssh-ed25519 AAAA…`.
  The key itself, not a fingerprint. `yidam lint` generates the allowed-signers file
  `git verify-commit` reads out of this column, at verification time and never as a
  committed artifact, so this file is the trust root: a key it does not carry verifies
  nothing, and a seat with no key declares its commits unverifiable.

  The principal in that generated file is **the seat's `ma/*` branch**, not a committer email,
  which is what gives a per-seat answer in a repository whose seats share one git identity. So
  what a verified signature says here is *this commit was produced by whoever holds this seat's
  key* — the seat signs. A model holds no key; an operator's key attests the operator; a run is
  an event and holds no standing.
- **`Seated`** — the `rigpa/<evolution>` that admitted this seat, which is registration step 3 in
  [PROTOCOL.md](PROTOCOL.md). Blank for a bootstrap seat, since the first elector registers
  themselves and no resolution admitted them, and blank for a registry that simply does not
  record it.

  It is here because Article III assumes a position is attributable to someone, and this is the
  repository's own answer to *who answers for this seat*: the resolution that admitted it, which
  is a committed record naming its executor and the tips it read. That is an artifact and not an
  out-of-band claim about who ran what.

  **Nothing computes it.** No check reads this column, and saying so is better than letting a
  reader assume one does. It is deliberately **not** part of a resolution's `independence:`
  reading either: `Kind`, `Model`, `Version` and `Config` describe a seat's occupant, which is
  what `shared-configuration` is about, and three seats admitted by one resolution are still
  three seats if their occupants differ.

## What a signature establishes here, and what it does not

Binding a key buys two things, and this file claims only those: **integrity** — the commit
is the bytes the key-holder produced, unaltered — and **third-party verification** — a
reader outside this repository can check, from this file alone, that a seat's commits
verify against the key it declares.

It does not buy independence between seats. Under a single operator, one key attests the
operator and distinguishes nothing the branch name did not; separate keys attest a
convention about which key was used for which seat. That is the measured case, not a
hypothetical: in the repository that has run this protocol, 126 commits across three
elector branches carry one git author. Signing is worth having for what it does establish.
It is not evidence that two seats are two minds.

**Where that fact gets recorded is the resolution, not this file.** The `Kind`, `Model`,
`Version` and `Config` columns are what a resolution's `independence:` reads to say whether it
could tell its participating seats apart — and `shared-configuration` is a legal answer that
costs the resolution only the right to call itself a synthesis of positions. A blank column
yields `unrecorded`, which is neither an accusation nor a clearance. See
[PROTOCOL.md](PROTOCOL.md).

Two consequences of that reading which bear on how this table is *edited*, now that
`resolution-independence-mismatch` computes the value rather than trusting it.

**This table is read at each seat's tip, not as it stands.** A resolution's derived
`independence:` comes from the registry as it was at the `ma/<elector>@<hash>` each seat's row is
looked up under. So correcting a row here does not re-open a settled record, and neither does
updating one: the resolutions that read that seat keep the answer they had. That is the whole
reason the reading is at the tips — a model bump is a material change, and a material change
should not turn somebody else's correct record red.

**A human seat's `—` is not a blank.** The `unrecorded` rule above is about agent seats, which is
what `shared-configuration` is about at all; a row whose `Kind` is `human` is distinct by being a
different person and its dashed agent columns are read as such. A row that leaves `Kind` itself
empty gets no such reading — the registry has not said what to expect, which is the ordinary
meaning of an empty column here and not a defect.
