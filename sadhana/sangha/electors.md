# Electors

Recognized participants in this repository's sangha. Each elector maintains a
`ma/<name>` branch as their working position.

See [PROTOCOL.md](PROTOCOL.md) for how to register as a new elector.

| Name | Branch | Role | Kind | Model | Version | Config | Key |
|------|--------|------|------|-------|---------|--------|-----|
| *(no electors registered yet)* | | | | | | | |

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
