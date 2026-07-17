# RFC-0004 — Drift detection: making `.yidam.toml` enforceable (`yidam sync`)

- **Status:** Draft
- **Track:** I4
- **Relates to:** RFC-0001 (report contract), RFC-0003 (light reports-only binary), RFC-0006 (embed drift)
- **Versioning layers touched:** template (the `.yidam.toml` schema + the bootstrap scaffold); tooling (the `yidam` CLI, which is not itself one of the three release trains)
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

`VERSIONING.md` promises a `claudesync` tool that reads a derived repo's `.yidam.toml`, reports
drift against origin tags, and applies forward upgrades. No such tool exists, and nothing in the
CLI reads `.yidam.toml` at all. The version pin — the one mechanism that was supposed to make
drift *visible* — is enforced by nothing. This RFC specifies `yidam sync` (with a CI-gating
`yidam check-drift`), a real `.yidam.toml` reader, per-layer drift semantics, and exit codes. It
standardizes the `cli`/`cli_ref` fields BOSC invented into the documented schema, and — crucially
— composes with RFC-0001 and RFC-0003 so that drift for a *re-implementing* consumer is measured
by **behavioral conformance**, not by tag equality. Pin-matching alone is a placebo: BOSC's pin
is exactly current and its behavior already diverges.

## Problem

**The tool was documented and never built.** `VERSIONING.md:16-35` prints the `.yidam.toml`
schema and states: "`claudesync sync` reads `.yidam.toml` and reports drift against the origin
tags" (`VERSIONING.md:25`), and "`claudesync upgrade --template 0.2.0` fetches the target release
and applies forward changes" (`VERSIONING.md:26`); the bootstrap layer repeats the promise at
`VERSIONING.md:96-99`. The string `claudesync` occurs in the repository *only* on those three
lines. There is no `claudesync` binary, no `sync` subcommand, and no `.yidam.toml` reader anywhere
under `yidam/cli/src/`.

**The only config the CLI reads is a different file.** `config.rs:16-24` loads
`.yidam/config.toml` into `YidamConfig`, whose sole field is `[index] model`
(`config.rs:11-14`) — the *runtime* config (which embedding model to use), not the *provenance*
pin. The root `.yidam.toml` that records what upstream you inherited is parsed by nobody.

**The pin file isn't even scaffolded.** The bootstrap command that stands up a derived repo
(`overlay.rs:30-104`) copies `yidam/`, `BOOTSTRAP.md`, `sadhana/`, `samudaya/`, and
`mise.yidam.toml` (`overlay.rs:90-96`) — but never writes a `.yidam.toml`. The pin file is thus
neither generated nor read by upstream. Every derived repo hand-authors it, which is how the
schema fragmented: BOSC added two fields (`cli`, `cli_ref`) that appear in no yidam document.

**BOSC makes the stakes concrete.** BOSC does not vendor the yidam tree or build the Rust CLI; it
re-implements the four reports in Python (`watermark.site.corpus_mirror`) and pins the upstream it
tracks in `.yidam.toml:17-25` — `template`/`bootstrap`/`cli = "0.1.0"` and
`cli_ref = "8f7ada99…"`. The file's own comment says it pins the ref "so drift against upstream is
visible" and that "yidam's own drift tooling (`claudesync`) reads this file" (`.yidam.toml:13-15`).
It does not. BOSC even wired optional cross-check tasks — `yidam-graph-check`,
`yidam-corpus-index`, `yidam-open-questions`, `yidam-lint` (BOSC `mise.toml:144-158`) — that run
the real binary over the same mirror, but they are un-gated, fire only "when it is on PATH," and
are not part of `mise run check`. Nothing consults the pin, and nothing enforces the cross-check.

**The divergence is not hypothetical.** BOSC's mirror marks a node open via a structured
`claim_tag == open` field; it does not write `?` into the label or the literal `[open]` into node
text. The real `yidam open-questions` predicate keys on exactly those surface markers
(`has_open_claim`, `cmd/mod.rs:48-50`). So running the pinned binary over BOSC's own mirror would
**under-report** open questions relative to BOSC's replica. The replica and the tool it claims
parity with already disagree on the same corpus — while the `cli_ref` pin sits precisely on
`8f7ada99`. Tag equality certified nothing.

This is the parity gate's failure mode one layer up: `mise.toml:125-136` ("parity fixture check")
verifies only that a *fixture directory exists* per function, not that outputs match. A pin that
checks a commit hash without checking behavior is that same empty gate.

## Proposal

### Build it into `yidam`, not a separate `claudesync`

Retire the `claudesync` name (it was never built) and implement drift detection as a first-party
`yidam` subcommand. The reader is a sibling of `config.rs`; the reports it must diff already live
in the CLI (`cmd/corpus.rs`, `cmd/lint.rs`); and behavioral conformance (below) requires
*running* the pinned reports, which is RFC-0003's light binary, not a shell script. A standalone
tool would re-vendor all of that — the precise re-implementation tax this RFC set exists to kill.

Split the surface by who runs it and when:

- **`yidam sync`** — read-only observer. Reports how far the pin lags origin. Advisory; a
  deliberate pin is *supposed* to be behind. Run by a maintainer deciding whether to upgrade.
- **`yidam check-drift`** — the CI gate. Fails when the working tree no longer matches *its own
  pin* (a **breach**, not mere lag). Run in the derived repo's CI (BOSC's `mise run check`).
- **`yidam upgrade --template|--bootstrap|--cli <ver>`** — the mutator that was `claudesync
  upgrade`; folded into the CLI, out of scope here beyond noting it consumes the same reader.

The pin-reading and template-diff layers must live in RFC-0003's **light** feature so a consumer
can install a `fastembed`/`lancedb`-free binary purely to run `check-drift` in CI. A Node/Python
shop (BOSC) will not carry the heavy native stack just to check a hash.

### The `.yidam.toml` reader

A `DriftPin`, sibling to `YidamConfig` (`config.rs:5-14`), read from the repo **root**
`.yidam.toml` (distinct from `.yidam/config.toml`):

```rust
#[derive(Debug, Deserialize)]
pub struct DriftPin {
    pub origin: String,        // git remote of upstream yidam
    pub template: String,      // template tag,   e.g. "0.1.0"  → monorepo tag v0.1.0
    pub bootstrap: String,     // PROTOCOL_VERSION the tests/results/ snapshots are valid for
    #[serde(default)]
    pub cli: Option<String>,   // human label for the CLI build (advisory; no tag train yet)
    #[serde(default)]
    pub cli_ref: Option<String>, // commit the consumer's reports track — AUTHORITATIVE for drift
}
```

Standardize `cli`/`cli_ref` into `VERSIONING.md`'s schema. Because the CLI binary is unpublished
and has no tag train (RFC-0003), `cli_ref` (a commit) is authoritative and `cli` is a display
label until a `cli/v{x.y.z}` train exists. Missing `cli_ref` means "consumer does not track the
CLI" — pin-checking skips that layer rather than erroring.

### What "drift" means, per layer

| Layer | Pin field | Drift signal | Class |
|---|---|---|---|
| Template | `template` | template-owned files (prelude/, BOOTSTRAP.md, mise skeleton, `mise.yidam.toml`) differ from the pinned tag | **breach** if locally edited; **lag** if origin advanced |
| Bootstrap | `bootstrap` | `PROTOCOL_VERSION` const at origin ≠ pin | lag |
| SDK/parity | (implicit) | `prelude/sdks/parity/VERSION` at origin ≠ the pin's era | lag; **informational** for a consumer with no SDK |
| CLI | `cli_ref` | pinned commit missing/moved at origin, or origin's `cli` tag resolves elsewhere | lag; breach if the ref no longer exists |
| **Conformance** | `cli_ref` | RFC-0001 fixtures run through the RFC-0003 binary **at `cli_ref`** diverge from the consumer's own output | **breach** — the real guard |

"Lag" is expected and never fails CI on its own — you pin to hold a version. "Breach" means your
tree no longer *is* what it claims to pin: you edited an inherited file away from its tag, the ref
you pinned vanished, or your re-implementation's report output no longer matches the pinned
binary's. Breach is the failing condition.

### Report output and exit codes

`yidam sync` prints a human table by default and machine JSON under `--format json`:

```
$ yidam sync
layer      pinned      origin      state
template   0.1.0       0.2.0       LAG   (origin +1 minor)
bootstrap  0.1.0       0.1.0       OK
cli_ref    8f7ada99    3c1d0e77    LAG   (12 commits behind)
conform.   —           —           SKIP  (run `check-drift --conformance`)
```

Exit-code semantics — the load-bearing part, so CI can gate:

- **0 — clean.** No breach. (`sync` also exits 0 on pure lag; `check-drift` exits 0 only when
  every checked layer is a match and any requested conformance passes.)
- **1 — drift detected (breach).** Inherited files diverge from the pinned tag, `cli_ref` no
  longer resolves, or `--conformance` found a report mismatch. This is the CI-gating failure.
- **2 — operational error.** `.yidam.toml` unreadable or missing a required field, origin
  unreachable, or the pinned binary/fixtures unavailable. Never conflated with 1: a network
  outage must not read as a clean tree, and a real breach must not hide behind "couldn't check."

`check-drift --frozen` treats *any* lag as failure too (exit 1), for repos that want the pin
bumped deliberately in the same PR that pulls upstream — the analog of a lockfile `--frozen`
check.

## Compose with RFC-0001 + RFC-0003 — conformance is the real guard

Pin-checking answers "is my hash the same hash?" For a consumer that *re-implements* rather than
vendors, that question is nearly content-free: BOSC's pin is exactly `8f7ada99` and its
`open-questions` behavior still diverges (above). The only detector that catches this runs the
actual rules and diffs outputs.

Frame `yidam sync` as the **umbrella** that binds version-pin checking to report-conformance
checking:

1. RFC-0001 makes the four reports a versioned contract with **golden fixtures** — corpus inputs
   paired with expected `graph-check` / `corpus-index` / `open-questions` / `lint` output at a
   report-contract version.
2. RFC-0003 makes those reports installable as a **light binary** — no `fastembed`/`lancedb`/
   `protoc` — so a Node/Python CI can run them.
3. `yidam check-drift --conformance` installs the light binary at `cli_ref`, runs the RFC-0001
   fixtures through it, and diffs against the fixtures' expected output. A mismatch is a breach
   (exit 1) even when every version field matches.

The consumer's stronger move: point `--conformance` at its *own* mirror and diff its replica's
output against the pinned binary's — the run that would have surfaced BOSC's `open-questions`
under-report the day it was introduced. Pin equality is the cheap necessary check; conformance is
the sufficient one. `yidam sync` reports both and is honest about which it ran — a green `sync`
that only checked hashes must say so, not imply behavioral parity.

## Migration & compatibility

- **Template layer (minor).** Add `cli`/`cli_ref` to the `.yidam.toml` schema in
  `VERSIONING.md:16-35`, marked optional, `cli_ref` authoritative. Additive; existing files
  (three-field) keep validating.
- **Bootstrap scaffold.** Fix `overlay.rs` to *emit* a `.yidam.toml` pinned to the current origin
  tags plus `cli_ref` = the overlay commit — so a derived repo starts with a correct, machine-read
  pin instead of hand-authoring one (as BOSC did). This closes the loop that let the schema drift.
- **Tooling layer.** `yidam sync` / `check-drift` / `upgrade` ship in the CLI, with the pin-read +
  template-diff paths gated into RFC-0003's light feature. `claudesync upgrade` becomes
  `yidam upgrade`; observer (`sync`) and mutator (`upgrade`) stay separate so CI runs the observer
  read-only. The `claudesync` name is retired in `VERSIONING.md`, not aliased — keeping a name for
  a tool that never shipped is what produced this gap.
- **No break for BOSC.** Its current `.yidam.toml` validates as-is. BOSC gains a real gate by
  adding `yidam check-drift --conformance` to `mise run check` (promoting its optional
  `yidam-*` tasks, BOSC `mise.toml:144-158`, into an enforced one) once the RFC-0003 light binary
  is installable in its CI.

## Alternatives considered

- **Git submodule / subtree pin instead of a TOML ref.** Byte-exact provenance and a real
  `git diff`, no bespoke reader — but it forces vendoring the Rust tree, the very thing BOSC
  refused (no Rust toolchain; `.yidam.toml:5-9`), and couples the consumer's build to yidam's.
  Strictly stronger for a consumer that *does* vendor, wrong default for one that re-implements,
  and not mutually exclusive: a vendoring consumer can submodule and still run `check-drift
  --conformance` against it.
- **A per-repo CI action that clones origin and diffs.** This is what `yidam sync` automates;
  hand-rolling it in each derived repo re-implements the reader and layer semantics N times — the
  exact tax. Ship it once, in the binary the reports already live in.
- **Leave it to `tonpa` (the bundle dependency manager).** `tonpa` resolves *bundle* dependencies,
  a different axis from *template/CLI provenance*; it has no concept of a template tag or a
  `PROTOCOL_VERSION`, and overloading it would burden a tool built for another layer. Reconsider
  only if `tonpa` grows a first-class notion of upstream-template provenance.

## Open questions

- **Behavioral parity for a re-implementing consumer.** Tag equality tells you nothing about it —
  BOSC is the proof. Conformance requires either running the RFC-0003 binary in the consumer's CI
  (does BOSC accept a Rust binary at all?) or shipping RFC-0001 fixtures as a **language-neutral**
  expected-output artifact the consumer's own code checks itself against — at which point the open
  problem shifts to keeping *that fixture set* pinned, looping back to version-pin checking. Which
  of the two is the supported path is unresolved and gates how BOSC actually adopts this.
- **Breach vs sanctioned local override.** Derived repos are *expected* to edit some inherited
  files. The template-diff needs a declared "these paths are locally owned" allow-list (in
  `.yidam.toml`?) so intentional edits are not eternal breaches — without letting the allow-list
  become a way to silence real drift.
- **Where the parity/SDK-VERSION delta belongs** when the consumer ships no SDK. Reporting it as
  "informational" risks alert fatigue; omitting it hides a layer that *can* matter (the
  `[inferred]`/`[inference]` marker split noted for RFC-0006 rides this axis).
- **Whether the CLI earns its own tag train** (`cli/v{x.y.z}`). Until it does, `cli` is a label
  and `cli_ref` a bare commit; RFC-0003's publishable binary would let `cli` become a real
  version and make the CLI layer pin-checkable the same way `template` and `bootstrap` are.
