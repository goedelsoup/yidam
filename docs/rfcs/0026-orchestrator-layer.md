# RFC-0026 — A run is a commit somebody can refuse (the orchestrator layer)

- **Status:** Draft
- **Track:** I21
- **Relates to:** RFC-0020 (the carriage rule this extends from findings to executions), RFC-0023
  (the store whose bytes a receipt records, and the sentence this design is a second application
  of), RFC-0024 (the policy layer, and the constitutional-family question it deliberately left
  open — this RFC is what needs it settled), RFC-0019 (the citation contract a cross-corpus gather
  is bound by), RFC-0003 (the light binary this must run in), RFC-0001 (the report contract it
  emits on), RFC-0018 (the precedent that a new surface is a CLI surface and **not** a fourth
  parity function), RFC-0009 (the execution authority this must not claim)
- **Versioning layers touched:** template (the prelude gains a capability-manifest section;
  `directories.md` gains one) / bootstrap protocol (the scaffold gains
  `.yidam/capabilities.toml`) / tooling (`yidam` CLI implements it) — **no parity-surface change,
  no MCP contract change in this RFC**
- **Parent epic:** #460 — this RFC specifies **#471** through **#476**
- **Downstream reference case:** none yet. The first consumer is `examples/streamflow`, by
  construction — see "Why the first thing built is not the manifest".

> **Noted 2026-09-04.** Open question 2 — does a write-capable MCP tool live in the existing tier
> or a new one — is answered by [RFC-0029](0029-write-tier.md): the same tier mechanism, with an
> opt-in declaration and an identity gate (declarable only where a git author identity exists).
> The question's #426 clause is discharged rather than carried: RFC-0027 corrected the premise —
> a profile is a projection of the canonical list, so a new tier changes nothing a profile
> serves — and RFC-0029 §2.4 restates the constraint in that corrected form. The invariant this
> RFC states is what made the answer safe to give; RFC-0029 §3 says how.

## Summary

`prelude/GRAPH.md:477-494` closes the commit vocabulary, and seven of its operational verbs name
acts a *pipeline* performs rather than acts a person performs. Four of them —
[`extract`](../../yidam/prelude/GRAPH.md#L481), [`refresh`](../../yidam/prelude/GRAPH.md#L482),
[`compute`](../../yidam/prelude/GRAPH.md#L483), [`reconcile`](../../yidam/prelude/GRAPH.md#L486) —
name capabilities that exist nowhere in this repository. The other three have a command that
produces an artifact and stops.

Not one of the seven authors its own commit. `cmd/propose/write.rs:251` is the only non-test code
path in the CLI that calls `git commit-tree`.

> **The verbs were written for a runtime that was never built.**

This specifies that runtime. Its unit is a **run**, and the design question it answers is not
*which execution engine* — `mise` and CI already sequence, and `due` already answers what is owed —
but **what a run may author, and what stops it authoring more**:

> A run authors **operational** commits directly. Every **epistemic** commit it produces goes to a
> proposal branch, and nothing merges itself.

That needs no new authority concept, because the commit vocabulary's own split is the permission
model. It is RFC-0020's *carriage, not composition* applied one level out: from what a proposal may
say about a finding, to what an execution may say about a corpus.

## Problem

### The measurement

Sort every command the CLI ships by what it does to the repository and four kinds come out: reports
that read and print, gates that read and exit nonzero, exports that write outside the graph, and
builds that write derived bytes. Then `yidam propose`, which writes commits — and it is alone.

| Verb | `GRAPH.md` says | What performs it | Authors its commit |
|---|---|---|---|
| `extract:` | Structured data pulled from a primary source | nothing — no command exists | no |
| `refresh:` | A connector re-run against its source | nothing — no command, and no connector | no |
| `compute:` | A calculator run and its output committed | nothing — no command, and no calculator | no |
| `reconcile:` | Catalog and corpus brought back into agreement | `catalog-audit` reports the disagreement | no |
| `index:` | The vector index rebuilt | `index-build` writes the index | artifact only |
| `bundle:` | The export bundle regenerated | `bundle` writes the bundle | artifact only |
| `regen:` | REGEN blocks refreshed | `regen` rewrites the blocks | artifact only |

Reproduce the right-hand columns:

```sh
grep -c 'Command::new("git")' yidam/cli/src/cmd/{regen,bundle,index_build}.rs   # 0, 0, 0
grep -rn 'commit-tree' yidam/cli/src/ --include='*.rs' | grep -v test           # propose/write.rs only
grep -n 'Reconcile' yidam/cli/src/main.rs                                       # no matches
```

Three of the seven produce an artifact and stop; a person then stages it and writes the subject
line by hand. **That is the moment provenance is invented rather than recorded** — the commit
saying `refresh: gauge records through August` is a person's account of what a tool did, written
after the fact, checkable against nothing.

### The same absence, from three directions

- **The domain computer is a scaffold with no runtime.** `sadhana/crates/README.md` describes
  connectors and calculators. Nothing declares which exist, invokes them, or records that one ran.
  `crates-index` reports on *directories*, so a crate that implements a connector and one that
  implements nothing are the same row.
- **A phase is a convention about ref names.** `prelude/PHASES.md` specifies a branch, declared
  outputs and a `--no-ff` merge. Nothing holds any of it. `cmd/phases.rs` derives `state` from ref
  shape, which is why #272 reports 26 active phases against a true count of 1.
- **`due` says it is time to something that cannot act.** `cmd/due.rs` exists because *"a practice
  is performed because it is time, and nothing here said it was time."* Of its four clocks exactly
  one has a mechanical discharge, and `cmd/serve/tools.rs` dispatches thirteen tools of which every
  one reads.

This is the third instance of a pattern this repository keeps finding and the first of its kind.
#194 found *the mechanism exists and has no path to a user*. #249–#253 found *every command is a
report, a gate, or an export*. This one is **the vocabulary names acts nothing can perform**.

## Proposal

### 1 — The unit is a run, and its output is commits

A **run** is one execution of one or more declared steps. It has:

- an **input state**: a commit sha plus a digest over the config and manifest that governed it, so
  *has this already run against this corpus* is an equality check rather than a heuristic;
- **steps**, recorded as they complete, which is what makes a run resumable and an interrupted
  phase legible;
- **outputs**, classified before they land;
- a **receipt**: what ran, against what, producing which bytes.

The receipt is RFC-0023's sentence applied to execution. That RFC says *a vault stores bytes, git
stores the record of them*; a receipt is that record for a computation rather than a fetch.

### 2 — The invariant, and why it needs no new mechanism

> A run authors operational commits directly. Every epistemic commit it produces goes to a proposal
> branch, and nothing merges itself.

`GRAPH.md`'s operational family is defined as *"the pipeline advanced; no understanding changed"* —
which is precisely the class of act a machine may perform unsupervised. The vocabulary already drew
the line this layer needs, years before anything could cross it.

Three consequences, each closing a shortcut somebody will reasonably propose:

- **A run may not author a node.** That is `establish:`, which is epistemic.
- **A run may not resolve.** Article V confines synthesis to a resolution event and forbids
  introducing what no elector held.
- **A run may not decide a question is answered.** RFC-0020 already applied this correction to
  `propose`; this layer inherits the finding, not the temptation.

**The invariant is mechanically testable**, which is the property that makes it worth stating this
way: classify every commit a run wrote by leading verb, and assert the epistemic ones are all on a
`propose/*` ref. `classify_commit` is already a parity function with fixtures in three SDKs.

#### `phase settle` prepares; it does not merge

`phase:` is an **epistemic** verb. So `yidam phase settle` must not author the `--no-ff` merge —
doing so would breach the invariant on the layer's second surface.

This is not a limit invented here. `cmd/due.rs:48` already reached it, of the phase clock:

> A person. Merging a phase, or abandoning it, is not a mechanical consequence of a finding.

`settle` validates that the phase produced outputs and drafts a vocabulary-checked subject. A
person merges. Same shape as `propose`, which is the point.

### 3 — The invariant is not policy-expressible, and RFC-0024 left that open for this

RFC-0024 shipped an authoritative policy layer: a corpus policy overrides a built-in guard by
package name and can therefore **loosen** it. It also drew the boundary and named what it deferred
([`0024-policy-as-code.md`](0024-policy-as-code.md), "Composition"):

> Disclosure is authoritative. **The constitution cannot be** […] a build that removed a refusal
> Article V imposes would be exactly that contradiction.
>
> So composition is a property of a **family**, not of the layer, and this RFC fixes it only for
> disclosure. When the constitutional family is built it needs the tighten-only composition
> declined above, and the open question is where that gets declared — in the policy itself, or in
> the Rust that owns the family. It is named here so that the disclosure design does not foreclose
> it, and left open because nothing yet needs it settled.

**This layer is what needs it settled**, and this RFC answers rather than inherits:

1. **What a run may author is not policy.** It is Rust, in the executor, with no override path. A
   corpus that could write an authoritative policy here could license its own runs to author
   `establish:` on the baseline, and the entire safety argument would become a config value. This
   is the contradiction RFC-0024 names, arriving through the door it left open.
2. **Tighten-only composition, for the constitutional family, is declared in the Rust that owns the
   family** — not in the policy. The reason is the one RFC-0024 gives for the boundary itself: a
   family whose composition rule is written in the artefact it composes can be loosened by editing
   that artefact. The Rust that owns a constitutional refusal is the only place the refusal cannot
   be edited out of.
3. **What *is* corpus-policy-governed:** step admission and throughput. Whether a step may run,
   how often, and how many proposal branches may stand open are judgements a corpus makes about
   itself, exactly like `escalate_after` — *"a value compiled into the binary would be one corpus's
   answer imposed on every other"* (`config.rs:52`). Those belong in policy. What a run may
   **author** does not.

The distinction to keep: **policy decides whether a run happens; it does not decide what a run may
say.**

### 4 — The manifest

`.yidam/capabilities.toml`. Per entry: `name`, `kind` (`connector` | `calculator`), how to invoke
it, `reads`, `writes`, the verb it authors, and how it ages.

```toml
[capability.low-flow]
kind   = "calculator"
run    = ["cargo", "run", "-p", "lowflow", "--"]
reads  = [".yidam/corpus/gage/**"]
writes = [".yidam/corpus/reach/**"]
verb   = "compute"
```

`writes` is load-bearing rather than documentation. It is what lets the executor refuse a step that
wrote outside its declaration, and what makes the operational/epistemic classification decidable
**before** the step runs rather than after it has already produced a tree.

Credentials are **named, never carried**. `vault/mod.rs` states the rule this inherits: a committed
file is not a place for a secret. A capability declares which secret it needs by name; the value
arrives from the environment.

#### No parity-surface change

The manifest parser stays Rust-only, and this RFC says so explicitly because an earlier draft of
#460 had it joining the parity surface.

No `.yidam/` config file has ever been a parity function. The ten are document and graph parsers;
`embed_config` is a declared **exception** with a section in `parity/README.md` naming the non-SDK
runner that reads it. RFC-0018 established that a new surface is a CLI surface and not a fourth
parity function, and RFC-0024 followed it and declared *"no parity-surface change"* in its header.
Admitting this one would mean three implementations that must agree, of a file no SDK consumes.

### 5 — The executor writes the way `propose` already writes

The write half is not new code. `cmd/propose/write.rs` builds commits against a temporary index
(`GIT_INDEX_FILE`), and its module doc gives the reason, which applies here unchanged and with more
force:

> The reason is not tidiness: a command that stashed, branched, committed and switched back would
> fail halfway on a dirty tree and leave somebody's work somewhere they did not put it.

Four properties follow, and a run needs every one: it touches neither the working tree nor
`.git/index`; it is safe to run mid-edit; it writes objects and one ref; and it separates author
from committer, so the record says the tool drafted and a person ran it.

### 6 — Freshness is `due`'s clocks, not a second mechanism

A step's staleness is a fifth reading of machinery that already exists. `cmd/due.rs` reads four
clocks and refuses to compile an interval into the binary, for `config.rs:52`'s reason. This RFC
adds no interval type, no scheduler, and no second notion of "stale".

`due` gains the column it was always missing: what would discharge this clock, and whether anything
can.

## What this does not do

- **It does not schedule.** `mise` and CI decide when; `due` answers what is owed. This answers what
  happened.
- **It does not synthesize.** No run merges a branch, resolves a tension, or closes a question.
  Article V is the ceiling and RFC-0009 owns the question of who may execute a resolution.
- **It does not replace the agent.** `PHASES.md` says the agent directs which connectors and
  calculators to invoke and synthesizes their outputs into corpus nodes. That stays true. What
  changes is that the invoking becomes an act the repository can record.

## Why the first thing built is not the manifest

The failure this repository keeps finding is *a surface with no consumer*: the fixture directory no
runner reads, the mechanism with no path to a user, the documented capability nothing demonstrates.
A manifest format with no executor is exactly that shape, and it would pass every gate while
asserting nothing.

So #471 declares **one** calculator in `examples/streamflow`, invokes it, commits its output as
`compute:`, and writes the receipt — no dependency resolution, no clocks, no second step.
Everything after it is generalisation of something that already works.

Two constraints on that first slice, both learned here:

- **The fixture must be git-tracked.** `tests/example_corpus.rs` treats an untracked file under
  `examples/*/.yidam/` as ungated — *"nothing here runs against it"*.
- **streamflow proves the mechanism only.** It is 8 nodes; it could not carry the retrieval claim
  in #264 and it cannot carry a throughput one. No performance claim rests on it.

## Open questions

1. **How does a corpus decline to be gathered from?** #476's cross-corpus gather reads a peer at a
   pin, through a bundle that peer published — and publishing is currently the whole of the consent.
   That is probably right inside one organisation and probably not past that boundary. Named here
   so the gather design does not foreclose it, and left open because nothing yet needs it settled.
2. **Does a write-capable MCP tool live in the existing tier or a new one?** RFC-0005 froze thirteen
   tool names and every one reads. #474 adds the first write-capable surface, which is a contract
   change rather than an addition. The refusal shape already exists — `refuse_unbacked` in
   `cmd/serve/tools.rs` declines a tool a server does not declare — and should be reused rather
   than re-invented. #426 records that the ChatGPT connector wants two specific names against
   RFC-0005's thirteen; whatever is decided must not make that worse.
3. **Where does a receipt live when a corpus has no vault?** The record is committed either way.
   Whether the *outputs* a receipt names must be retrievable, or may be absent with the receipt
   still standing as provenance, is not settled. RFC-0023's answer for catalog artifacts — a stale
   vault cannot lie, because the digest is in the commit — probably transfers, and has not been
   checked against a computation whose output nothing else references.
