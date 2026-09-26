# RFC-0026 — A run is a commit somebody can refuse (the orchestrator layer)

- **Status:** Implemented
- **Commands:** `run`, `phase`
- **Track:** I21
- **Relates to:**
  - RFC-0020 (the carriage rule this extends from findings to executions)
  - RFC-0023 (the store whose bytes a receipt records, and the sentence this design is a second application of)
  - RFC-0024 (the policy layer, and the constitutional-family question it deliberately left open — this RFC is what needs it settled)
  - RFC-0019 (the citation contract a cross-corpus gather is bound by)
  - RFC-0003 (the light binary this must run in)
  - RFC-0001 (the report contract it emits on)
  - RFC-0018 (the precedent that a new surface is a CLI surface and **not** a fourth parity function)
  - RFC-0009 (the execution authority this must not claim)
- **Versioning layers touched:** template (the prelude gains a capability-manifest section;
  `directories.md` gains one) / bootstrap protocol (the scaffold gains
  `.yidam/capabilities.toml`) / tooling (`yidam` CLI implements it) — **no parity-surface change,
  no MCP contract change in this RFC**
- **Parent epic:** #460 — this RFC specifies **#471** through **#476**
- **Amended 2026-09-25 (#941):** the status reads `Implemented` on the command surface above —
  `run` and `phase` shipped, and 0.14.0 was cut with them. Two of #460's children are open and
  neither is a command: **#475**, the invariant of §3 as a permission rather than a code path,
  and **#476**, asking a `tonpa` pin a question. An `Accepted` status would have been the
  truer reading of those two and a false one about a released binary, and the release is the
  half a reader can check.
- **Amended 2026-09-26 (#1028):** §4.2 states the read half of `writes` — what a
  calculator puts in `.yidam/computed/` and how a reader reaches it. `writes` governed the
  executor from the first commit and governed nothing else; two calculators wrote there for
  weeks and no retrieval surface could see a word of it.
- **Downstream reference case:** none yet. The first consumer is `examples/streamflow`, by
  construction — see "Why the first thing built is not the manifest".

> **Amended 2026-09-20 — the epistemic arm is built.** §2's second sentence had no
> implementation: `manifest.rs` refused an epistemic verb at load, so nothing could produce an
> epistemic commit at all and the clause was held by making the whole family undeclarable. It is
> now a second **destination**. `Capability::route` reads the verb through the families
> `classify_commit` already draws and returns where the commit goes — the invoking branch for an
> operational verb, `propose/<head>` for an epistemic one — and the property §3.1 exists to
> protect is stated in the executor's module doc as the thing that must not weaken: *there is no
> path, and no config value, by which a run advances the current branch with an epistemic
> commit.* There is no manifest field, and `deny_unknown_fields` is what makes an attempt to
> write one a parse error rather than a line somebody reads as effective. §2.1 below is the
> record.

> **Amended 2026-09-20 — a capability that depends on model weights must pin them.** §1's
> input state is *"a commit sha plus a digest over the config and manifest that governed it"*,
> which is complete for a deterministic calculator and silently incomplete for anything whose
> output depends on weights: the same `input_state` then produces a different output sha on
> different hardware, and the corpus records a change that is not one. **§4.1 is the pin**, and
> it is fields on the manifest rather than a new file, for reasons that turn on which direction
> the declaration flows. It is specified and deliberately **not built** — no capability in any
> corpus runs a model today, and §4.1.6 says what would have to be true first.

> **Amended 2026-09-22 — the manifest is a plan, and `crates-index` can tell a crate that
> implements something from one that does not.** #471 shipped one step per invocation, which was
> the vertical slice and said so. #472 generalised it: `after` declares what a step waits for,
> and a run resolves the transitive closure, orders it so every dependency precedes what declares
> it, and invokes each stale step against the commit the one before it landed — a cycle is
> refused with the cycle named, and nothing runs. Freshness is the input state §1 already
> specified, now read *before* a step is invoked rather than after, so a fresh step is skipped
> rather than run and discarded; `ageing_days` is §6's declared interval for the case an input
> state cannot see, and it is per capability because that is where the catalog's TTL already
> lives. `--dry-run` resolves the plan and writes nothing. And §4's `writes` refusal now has a
> companion that is the same argument about `reads`: a step is invoked in a tree holding exactly
> what it declares, so a capability that did not declare its own implementation is refused by
> name rather than failing in a shell. The index column is below.

> **Noted 2026-09-04.** Open question 2 — does a write-capable MCP tool live in the existing tier
> or a new one — is answered by [RFC-0029](0029-write-tier.md): the same tier mechanism, with an
> opt-in declaration and an identity gate (declarable only where a git author identity exists).
> The question's #426 clause is discharged rather than carried: RFC-0027 corrected the premise —
> a profile is a projection of the canonical list, so a new tier changes nothing a profile
> serves — and RFC-0029 §2.4 restates the constraint in that corrected form. The invariant this
> RFC states is what made the answer safe to give; RFC-0029 §3 says how.

## Summary

`prelude/GRAPH.md:566-640` closes the commit vocabulary, and seven of its operational verbs name
acts a *pipeline* performs rather than acts a person performs. Four of them —
[`extract`](../../yidam/prelude/GRAPH.md#L625), [`refresh`](../../yidam/prelude/GRAPH.md#L626),
[`compute`](../../yidam/prelude/GRAPH.md#L627), [`reconcile`](../../yidam/prelude/GRAPH.md#L630) —
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

#### 2.1 — The second route, and why it is not a permission

*Built 2026-09-20.* The first slice implemented half of the sentence above and held the other half
by refusing the verb at load. That was honest about what existed — there was nowhere for an
epistemic commit to go — and it had a cost that only became visible when something wanted the arm:
a run could compute a number and could not open a question about one. Deterministic calculators
never needed it. A classifier does, and that is what surfaced the gap.

The refusal is replaced by a destination:

| the capability's verb | where its commit goes | what moves |
|---|---|---|
| operational (`compute`, `extract`, …) | the branch the run was invoked from | that branch |
| epistemic (`establish`, `revise`, …) | `propose/<head>` | nothing the person was on |

**`Capability::route` takes `&self` and reads one field.** It is a total function of `verb`, and
`verb` is closed by the manifest's own validation, so there is no third answer and no arm to get
wrong. A manifest cannot declare a route; `deny_unknown_fields` means `route = "branch"`,
`allow_direct = true` and `epistemic = false` are all parse errors rather than ignored lines. That
distinction is the design: an *ignored* permission is worse than a refused one, because the corpus
that wrote it believes it is in effect.

The reasoning is §3.1's, unchanged. A field here would make the safety argument a config value,
which is the contradiction RFC-0024 named. What changed is only that the rule now has two branches
instead of one and a half.

Three things follow that are worth writing down because somebody will want each of them:

- **The proposal commit is stacked on the proposal branch, not on HEAD**, when one already stands
  at this head. The receipt still records HEAD as the input state — the step read HEAD — so two
  epistemic runs at one head accumulate the way `propose`'s commits do rather than each discarding
  the other.
- **Idempotence is measured against the proposal branch's tip.** `already_landed` compares the
  receipt and the output blobs at whatever ref the run is about to move, so a re-run that computes
  the same answer writes nothing on either route.
- **Nothing merges itself, and the report says so.** An epistemic run prints the `git log
  --reverse` that reviews the branch and the two things a person may then do with it. The commit's
  own trailing paragraph says the corpus is unchanged until somebody merges, where an operational
  one says the pipeline advanced and no understanding changed.

#### `phase settle` prepares; it does not merge

*Built 2026-09-23 (#473), with `phase start` and `phase run` beside it.* The surface is three
verbs: `start` snapshots the input state §1 specifies and commits it to `.yidam/phases/<name>.yml`
on the phase's own branch, `run` records the plan before the first step and each step as it
completes, and `settle` validates and drafts. The record is committed as `scaffold:` —
operational, so the invariant below is untouched rather than argued around, and the one verb this
layer might have wanted is the one only a person writes. RFC-0028 §3 makes the composition call
the record is ranked under. `phase_record.rs`'s
`settle_authors_no_commit_and_moves_no_ref` asserts the limit this section states, over the whole
object graph rather than over the branch.

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
it, `reads`, `writes`, the verb it authors, what it waits for, and how it ages.

```toml
[capability.low-flow]
kind   = "calculator"
run    = ["cargo", "run", "-p", "lowflow", "--"]
reads  = [".yidam/corpus/gage/**"]
writes = [".yidam/corpus/reach/**"]
verb   = "compute"
after  = ["gather-gages"]   # optional — what must be up to date first
ageing_days = 30            # optional — see §6
```

`writes` is load-bearing rather than documentation. It is what lets the executor refuse a step that
wrote outside its declaration, and what makes the operational/epistemic classification decidable
**before** the step runs rather than after it has already produced a tree.

`reads` is load-bearing on the same terms and in a way worth stating, because it has a consequence
a first reader meets as a puzzle: the step is invoked in a tree holding exactly what `reads`
resolves to, so **a capability must declare its own implementation**. The executor refuses a `run`
argument that names a tracked file the declaration does not cover, rather than leaving it to a
shell to report a missing file. Declaring it is also what puts the implementation in the input
state, so editing a calculator is what makes its step stale.

`after` is resolved and not advisory. A run plans the transitive closure and orders it, so a step
is never invoked against an upstream that had not run — and a step may not come `after` one
declaring an epistemic verb, because what that step writes lands on a proposal branch and is not
in the tree a dependent would be materialized from. Waiting for it would mean waiting for a person
to merge, which is not a thing a plan can contain.

Credentials are **named, never carried**. `vault/mod.rs` states the rule this inherits: a committed
file is not a place for a secret. A capability declares which secret it needs by name; the value
arrives from the environment.

### 4.1 — A capability whose output depends on weights pins them

*Specified 2026-09-20. Not built — see §4.1.6.*

Everything above assumes a step is a function of its declared inputs. `already_landed` says so
outright: a calculator *"that is not a function of its inputs — a clock, a random seed, a network
read it did not declare — would otherwise have its new output silently dropped on the grounds that
its inputs had not moved, which is the one failure that would make a receipt a lie."*

Model weights are a fourth item on that list and the only one a corpus would add **on purpose**. A
capability that classifies, embeds or scores is reproducible exactly to the degree that its
weights, precision, tokenizer and thresholds are fixed and recorded; without that, the same
`input_state` computes a different output sha on different hardware, and the commit that lands
records a change to the corpus that did not happen.

#### 4.1.1 — The precedent, and what it establishes

`embed.config.json` — `docs/domain-computer.md`, *"Embedding reproducibility"* — already solved
this once for the index:

> `model_file` matters as much as `model_id`: quantized and fp32 exports of the same model drift ~1e-2 per element, far beyond retrieval-safe tolerance.

Two things carry over. **A model identity is not a pin** — the weights file is half the answer and
the precision is the reason. And a tolerance that cannot be met is **declared rather than
widened**: sentence-transformers cannot load the quantized export, so its looser bound lives in
the fixture's `[known_delta]` section where a reader meets it, instead of the shared bound being
loosened until everything passes.

#### 4.1.2 — Fields on the manifest, echoed in the receipt. No new file.

```toml
[capability.tier-classifier]
kind   = "calculator"
run    = ["cargo", "run", "-p", "tier", "--"]
reads  = [".yidam/corpus/**"]
writes = [".yidam/computed/**"]
verb   = "assess"

[capability.tier-classifier.model]
id            = "Xenova/all-MiniLM-L6-v2"
revision      = "e4ce9877abf3edfe10b0d82785e83bdcb973e22e"
weights       = "b4c6…"   # content address of the weights, 64 hex — see 4.1.3
tokenizer     = "9a1f…"   # ditto, and it is not optional — see below
precision     = "q8"
output_digits = 4

[capability.tier-classifier.model.thresholds]
accept = 0.82
```

**Why not a file.** `embed.config.json` is written by a producer and read by three runtimes, and
carried into every index-bearing export; it is an *artifact of a build*. A capability's pin flows
the other way — a person declares it and one runner reads it — which is what a manifest is. And a
separate file is the one shape that fails unsafely, for the reason §4.1.5 gives.

**Why no change to the input state.** `Receipt::input_state` already *"takes the declaration whole
rather than field by field: every field of it is part of the identity, so a signature that
enumerated them would have to be revisited — and silently could not be — each time #472 adds
one."* A pin added to `Capability` is in the input state the day it exists, with no second place
to forget it. `manifest_sha256` covers the same bytes a second time; that redundancy is existing
behaviour and is left alone.

**The fields, and why each is there.**

| field | what it fixes |
|---|---|
| `id` | which model |
| `revision` | which state of it — a commit on the model host, **never a tag**. A tag moves, which is the defect `external-citation-pin-moved` reports one layer up and for the same reason. |
| `weights` | §4.1.1's finding as a field: the export, by content, not by name |
| `tokenizer` | the same argument on the input side. A tokenizer is a second set of weights and nobody thinks of it as one; two tokenizers disagreeing about a sentence is the same failure as two exports disagreeing about a vector, with no error signal either. |
| `precision` | `q8`, `fp16`, `fp32`. Named rather than inferred from a filename, because a filename is a convention of one model host. |
| `thresholds` | the number that turns a score into a decision. Untyped, per capability, for the reason a node's `properties` are: the vocabulary is the domain's, not this layer's. |
| `output_digits` | §4.1.4. |

#### 4.1.3 — Where the weights live: the catalog and the vault, which are one mechanism

Weights are not committed. RFC-0023 is the mechanism and its sentence is the design — *a vault
stores bytes, git stores the record of them* — and the record already exists. `CatalogArtifact`
carries `sha256`, `bytes`, `media_type`, `vault` and `redistributable`; `yidam vault push` and
`pull` already move exactly those bytes; `yidam vault verify` already re-hashes what is cached and
reports anything that is not what it claims.

Model weights are that object precisely. They are bytes obtained from somewhere, too large to
commit, with a routing question and — for a model licence — a real `redistributable` question that
a corpus must not be able to answer casually.

**So the pin carries the content address and nothing else about the bytes.** The catalog entry is
where they came from, how big they are, which vault holds them, and whether they may leave the
machine. One hash, two readers: the vault entry's recorded hash **is** the pin, rather than a
second copy of it that can drift.

Two consequences:

- A new check, `capability-model-unanchored`: a pin whose address no catalog entry declares. That
  is the join made checkable, and it is the same shape as `catalog-artifact-unroutable`.
- Before invoking, the executor resolves the address through the vault cache and refuses when the
  bytes are absent or do not hash to what was declared. `yidam vault verify`'s predicate, called —
  not a second one.

#### 4.1.4 — Declared output precision, and the honest account of what it buys

`output_digits` says how many significant digits the capability writes. It is **declared, not
enforced**: nothing here can check that a calculator rounded, and a check that tried would be
parsing the calculator's own output format.

What it buys is a falsifier, and the mechanism is already built. `already_landed` compares output
blobs by git object id, so a run whose inputs and weights are unchanged and whose output respects
the declared digit writes **no commit at all**. A capability that drifts below the declared digit
therefore lands a commit whose entire content is noise, visibly, on the next run — and the
question changes from *why did this number move?* to *the capability did not honour its own
declaration.* That is a smaller and answerable question, which is the whole of the claim.

#### 4.1.5 — The migration hazard, measured, and why the refusal is the right outcome

`Capability` is `#[serde(deny_unknown_fields)]`. A manifest carrying a `[capability.x.model]`
table therefore **fails to parse on every released binary**. Measured against `yidam 0.12.0`, with
the table appended to `examples/streamflow`'s manifest:

```
Error: parsing .yidam/capabilities.toml

Caused by:
    TOML parse error at line 25, column 22
       |
    25 | [capability.low-flow.model]
       |                      ^^^^^
    unknown field `model`, expected one of `kind`, `run`, `reads`, `writes`, `verb`
```

The blast radius, also measured: **every capability in that manifest stops running**, including
unpinned ones, because the refusal is at `Manifest::parse` and before `get(step)`. `yidam lint` is
unaffected — it reported `0 finding(s), no errors` on the same tree — so the gate survives and
only `run` is down.

**This is the correct failure, and it is the reason the pin is not a file.** The alternative is an
old binary that reads the manifest, ignores the pin, runs a model capability without verifying the
weights, and reports success — which is the exact defect the pin exists to prevent, delivered
silently. `deny_unknown_fields`'s own module doc already made this argument about `after`: *"A
binary that silently ignored an `after = [...]` it did not implement would run a dependent step
before the step it depends on and report success — the failure would be in the corpus, not in the
exit code."*

So: **upgrade the binary, then add the pin.** The order is forced, the error names the field, the
line and the accepted set, and `yidam --version` is the next thing a reader checks. What cannot be
improved is the message itself — a binary already released cannot be taught to say *this is a
field from a later version*, and claiming otherwise in this RFC would be describing a fix nobody
can ship backwards.

#### 4.1.6 — Why this is specified and not built

No capability in any corpus runs a model. Shipping the fields now would add a declaration nothing
reads and nothing writes, which is the shape this repository keeps finding at the end of a
measurement rather than the start of one. Three things should be true before the fields land:

1. **A real model capability exists**, so the pin is verified against weights somebody actually
   loads rather than against a fixture written to satisfy it.
2. **`capability-model-unanchored` has a subject** — a catalog entry holding those weights — so
   the join in §4.1.3 is exercised rather than asserted.
3. **The parity question is asked once**, in the form §4 already answers for the manifest: this
   stays Rust-only unless a second runtime loads the same weights, and if one does, the precedent
   is `embed_config`'s — a declared exception with a named non-SDK runner, not a fourth parity
   function.

The decision this RFC makes now is the shape and the migration, because both are cheaper to settle
before there is a corpus to migrate than after.

#### 4.1.7 — The pin and the route are the same argument

A capability that classifies is almost always epistemic — `assess`, `establish`, `revise` — so
§2.1 routes its output to `propose/<head>` and a person reads it before it reaches the corpus.
**The pin is what makes that review possible.** A reviewer asked to accept a proposal generated by
a model needs to know which weights, at which precision, above which threshold; without a pin the
honest answer is *some model, on whoever's machine ran it*, and the proposal branch is a formality.

#### No parity-surface change

The manifest parser stays Rust-only, and this RFC says so explicitly because an earlier draft of
#460 had it joining the parity surface.

No `.yidam/` config file has ever been a parity function. The ten are document and graph parsers;
`embed_config` is a declared **exception** with a section in `parity/README.md` naming the non-SDK
runner that reads it. RFC-0018 established that a new surface is a CLI surface and not a fourth
parity function, and RFC-0024 followed it and declared *"no parity-surface change"* in its header.
Admitting this one would mean three implementations that must agree, of a file no SDK consumes.

### 4.2 — What a calculator writes is read back by declaration

*Amended 2026-09-26 (#1028).*

`writes` is load-bearing on the way **in** — §4 says so, and the executor refuses a step that wrote
outside its declaration. It was nothing at all on the way **out**. `examples/streamflow` declared
two calculators writing `.yidam/computed/**`, both ran, both committed, and no surface in the CLI
opened the directory: `paths.rs` had no helper for it, `doctor` did not ask about it, and `embed`
walked past it. A computed answer could be produced, receipted and reviewed, and still not reach a
search. That is the failure shape in "Why the first thing built is not the manifest", arrived at
from the other end — not a surface with no consumer, but a **product** with no consumer.

**A file declares its own readability rather than being guessed at.** A top-level
`format_version: 1` and a `signals:` list whose rows each carry a `node:`:

```yaml
format_version: 1
method:
  rule: |
    A derived assertion travels only as far as the weakest claim beneath it.
signals:
  - node: gage/canyon-outlet
    travels_as: open
    downgraded: true
```

Every other key in a row is a signal about that node. Everything outside `signals:` is not read —
`method:` above is a calculator's account of itself, and a summary table beside it stays legal. A
file carrying no `format_version` is **listed and not read**, so a calculator whose output is a
report rather than a table is visible in `doctor` and silently ignored by `embed`, instead of
having to be either renamed or parsed on a guess.

Guessing was the alternative, and it was tried first: streamflow's disclosure calculator wrote a
`tiers:` block, and a reader inferring structure from shape read its three tier names as three
nodes. A wrong answer that parses is worse here than no answer, because the signal it invents is
attached to a node reference nothing else in the corpus uses.

**A row is keyed in the reference grammar (RFC-0032) and in nothing else.** `gage/canyon-outlet`,
`node/gage/canyon-outlet`, or the absolute form naming this corpus. The alternatives were a
class-scoped id and a bare filename, and the reason to refuse both is that either one is a second
permanent name for a node the corpus already names one way. A revision pin is refused rather than
accepted and ignored: a signal computed at a past commit is not a signal about the node as it
stands, and ignoring the pin would attach it as though it were.

**A signal name is repository-wide, and a collision is refused naming both files.** The rejected
alternative is a per-file namespace, which sounds safer and is worse: it makes a calculator's
filename load-bearing, so renaming the file renames every signal, and it gives each signal a second
permanent name at the one place — a query filter — where a short one is the whole value. Two files
claiming `tier` is one word meaning two things; `doctor` says so and names them.

**Validation is against the manifest, not against the directory.** A computed file is attributed to
the capability whose `writes` covers its path, and a file no declaration covers is reported. That
follows §6.1 exactly — the correspondence is declared, never inferred — and it is what makes
"which calculator asserted this, and has it run since its inputs moved" answerable from the
committed record rather than from a filename convention.

**Freshness is the receipt, read back.** §6 says freshness is `due`'s clocks and not a second
mechanism; this adds no third one. `doctor` reads the receipt committed at HEAD, recomputes the
input state from the working tree, and reports a step whose inputs have moved. It reads the receipt
from **HEAD** and not from disk for a reason found by running the command: §5's executor touches
neither the working tree nor the index, so between a run and the `git restore` that syncs a
checkout, a receipt on disk is the *previous* one. Reading disk made `doctor` answer *it has never
run against this corpus* in exactly the window where a run had just happened — and the honest answer
in that window is the other one: the result stands, the files are at HEAD and not in your checkout.

**Where a signal surfaces, and where it deliberately does not.** `yidam embed` attaches a node's
signals to its embedding record as a `signals` object, **absent** where there is none — additive, so
an index already built stays valid and a corpus computing nothing is byte-identical to before.
Signals are not folded into the embedded text: a boolean concatenated into prose is a token the
model has no use for and a fact a filter can no longer read. `doctor` gains a `computed` question
and `status --format json` the counts. What is **not** here is an index column, and that is
deliberate rather than pending: #1029 owns the local index schema, and `index-push` builds its
remote metadata from the local rows rather than from the embedding JSON, so filterable metadata
follows that decision instead of duplicating it.

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

**Two readings, and the second is declared.** A step is stale when what it reads, or what it
declares, is not what its committed receipt was computed from — §1's input state, asked before the
step is invoked rather than after, so a fresh step is skipped rather than run and thrown away. For
anything that is a function of its declared inputs that is the whole answer.

It is not the whole answer for a connector, whose input state can sit unchanged across a year in
which everything it describes moved. `ageing_days` is that case, and it is the catalog clock's
distinction exactly — *"An expiry does not claim the upstream changed. It claims nobody has
looked."* It is declared **per capability** rather than as a `[due]` key for the same reason a
source's TTL is declared on the source: reading it from a second place would be a second place to
set one number. The age is measured from the commit that last carried the receipt, which is the
clock the receipt deliberately does not hold.

A step re-run under an ageing rule whose answer had not moved still lands a commit, and that is
the intent rather than an oversight: the receipt names a new input commit, and that commit is the
record that somebody looked. The report distinguishes it from a step whose output changed, because
those are different events and only one of them is a change to the corpus.

### 6.1 — A directory is not a capability

`crates-index` and `packages-index` reported on directories, so a crate implementing a connector
and a crate implementing nothing produced the same row — the first of the three symptoms in the
Problem section, and the one that survives longest because a table of directories looks like a
table of capabilities. Each row now names the capability that runs it and its kind, read from the
manifest, and an em dash where nothing declares it.

**The correspondence is declared, never inferred.** A capability claims a package by naming it in
`run` — the package name as a token, which is `cargo run -p <name>`'s own spelling, or a path at
or under its directory. Nothing guesses from a crate's name or its shape. #460's failure table has
that as *"a gather aligns schemas by name"*, and the answer is the same one: correspondence is
declared per thing, never inferred from resemblance.

The guard on this is discovery in both directions and carries no list: every declared capability
resolves to a file the repository tracks, and every implementation beside one is declared. Even
the directory implementations live in is discovered — it is wherever declarations point — so a
corpus that keeps its calculators somewhere else is covered by the same two assertions.

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
