# RFC-0028 — The form a practice takes (the kuten layer)

- **Status:** Draft
- **Track:** I23
- **Relates to:** RFC-0020 (the carriage lineage this extends a third step — from findings to
  executions to conduct), RFC-0026 (the permission layer this composes with, and the two seams
  on `cmd/phases.rs` and `classify_commit` decided here), RFC-0024 (the policy layer every
  severity a kuten proposes must enter through, visibly), RFC-0019 (the citation contract the
  object slot's coupling checks reuse rather than re-invent), RFC-0008 (the strict reading of
  Article V the constitutional argument here extends), RFC-0001 (the report contract
  `kuten check` emits on), RFC-0003 (the light binary it must run in)
- **Versioning layers touched:** template (the prelude gains `kuten/`, vendored at genesis into
  `.yidam/.vendor/prelude/kuten/`; the binding rule below joins the vendored text) / bootstrap
  protocol (the dialogue gains the selection; the scaffold gains `.yidam/decisions/kuten.yml`) /
  tooling (`yidam kuten`, `yidam kuten check`; `phases` and `lint --commits` read the
  declaration) — **no parity-surface change and no MCP contract change in this RFC**; the one
  candidate parity change is argued and deliberately deferred in §4, "Registers scope
  recognition, never classification"
- **Parent epic:** #572 — this RFC is A1 (#573) and specifies **A2–A7** (#574, #575, #576,
  #286, #577, #288)
- **Downstream reference case:** A0's population — eighteen derived corpora on disk, 6,900
  commits, read-only. Six of them define `inquiry`; two are object-coupled; one is a projected
  mirror of 1,656 commits (#582).

## Summary

A yidam repository declares what it is **about** and never what its work is **for**. The domain
layer is genuinely parameterized — the ontology dialogue, `.ont.yml`, `edge_policy`, the
selectable prelude domains. The telos layer has exactly one value, stated once, and then assumed
by everything downstream: the four phase types, the four clocks, the genesis rubric, the lint
severities, the bootstrap's *"what is the central question."*

> **A kuten is a committed, vendored declaration of what a corpus's practice is aimed at. It
> narrows and parameterizes the loop; it may not widen the model.**

*kuten* (སྐུ་རྟེན, *sku rten*) — the support in which a form is present; also the medium through
which an oracle speaks. Gloss it **"the form a practice takes"**, never "support" — `prelude`
and `vault` already occupy the substrate reading.

Most of this layer was settled by #572's scope decisions and by A0's measurement, and this RFC
**transcribes** those parts rather than re-deriving them: the slot inventory and its verdicts,
the five prohibitions, one kuten at A3, the revision model, A5 co-designing with #286. What was
not settled, this RFC **decides**, on the record:

1. the phase composition with #460 — who owns `cmd/phases.rs`, and whether the run record
   replaces or feeds the shipped `RefKind` derivation;
2. where path-scoped registers live relative to `classify_commit` — argued both ways, decided;
3. the `object` slot's direction — projection endorsed as a declared state, against #582;
4. #578's disposition — the question-pressure slot ships its epistemic half standing alone, and
   #578 is explicitly unscheduled, with the conditions that would schedule it;
5. the constitutional argument, made once, in the RFC-0020 → RFC-0026 carriage lineage, with
   the binding rule destined for vendored prelude text — so #287 can cite it by section instead
   of making it again.

## Problem

### The telos layer has one value, and everything downstream assumes it

Reproduce the count:

```sh
grep -n "sustained inquiry" yidam/prelude/IDENTITY.md    # one hit — the only telos value
find examples -name '*.ont.yml' | wc -l                  # 14 classes across 4 corpora
```

[`IDENTITY.md:27-28`](../../yidam/prelude/IDENTITY.md#L27-L28) states the purpose — *"the
corpus grows through sustained inquiry"* — and it is the only statement of purpose the template
carries. The four corpora under `examples/` declare fourteen classes between them and share not
one name, so the *domain* axis demonstrably parameterizes. On the *telos* axis, a design corpus,
a public-records corpus and a hydrology corpus are phased by
[`PHASES.md`](../../yidam/prelude/PHASES.md)'s four fixed types, clocked by `cmd/due.rs`'s four
clocks, scored by one genesis rubric, and asked one bootstrap question.

### A0 — measured before designing, and it killed two predictions

Transcribed from #572, which is the record of the measurement; the numbers are repeated here
because every design decision below leans on one of them.

**Eighteen derived corpora, 6,900 commits, 3,300 instance nodes, read-only.** One cluster
survives: **`inquiry`** — six repositories, six unrelated domains, 73 to 1,123 commits —
converges on phase commits **13–26%**, nodes/commit **0.50–1.11**, median node **35–62 lines**,
off-vocabulary **exactly 0% in all six**. That is A2's extraction target, and it is falsifiable.

A second cluster was reported and then retracted. Two controls dissolved it, and both are
mandatory in any repeat of this measurement (including #288's — "the method A7 inherits"):

| Control | What it found |
|---|---|
| **Vendored-prelude vintage** | yidam `35aee3f` (2026-08-08) closed the vocabulary and added the `phase` verb; `bccfc4e` added the `.yidam.toml` pin the same day. All three candidate members vendored a prelude with **no `phase` verb and no closed vocabulary**, and none carries the pin. Their 0% phase usage and their 43%/73% "violations" are properties of the template they hold |
| **Repository maturity** | Nodes-per-commit halves over a repository's life (`allen-county-ohio`: 2.07 @69 → 0.98 @250 → 0.54 at HEAD). At commit 69 a core inquiry repository accretes faster than two of the three candidates |

What survived, better controlled than the original claim:

- **Object coupling is the only thing that breaks vocabulary conformance.** Of the nine
  repositories whose vendored prelude closes the list, **seven are at exactly 0%**
  off-vocabulary. The two that are not — 25% and 4% — are the two object-coupled ones. Object
  coupling is an **axis, not a profile**: it crosses both shapes.
- Node length varies **3.4×** among vintage-matched repositories alone (median 35 → 118 lines).
- Phase use varies **0–26%** among the nine that have the verb — six use it, three do not.
- One genuine outlier, n=1: a corpus at **11.26 nodes/commit at matched maturity**, 5.4× the
  next, 583 of 777 nodes in two structured classes. A case to understand (#578), not a kuten to
  mint.
- `.yidam/config.toml` is empty in **17 of 18** corpora, and **not one** carries a `.rego`
  override. A blank nobody has filled in eighteen tries is not a preference being expressed.

### The vocabulary has three failure causes, not one

The plan first proposed narrowing the closed list. Splitting off-vocabulary commits by which
paths they touch shows narrowing helps one of three populations:

| Cause | Evidence (A0) | Answer |
|---|---|---|
| **Register bleed** | one repository: 24 of 32 off-vocabulary commits touch **no corpus file at all** — `feat:`/`fix:`/`test:` on the artifact, reported as corpus violations. Another: 107 of 111 | **Path-scoped registers** (§4) |
| **Genuine coinage** | one repository: 260 off-vocabulary commits touching only `.yidam/`; `corpus:` coined against a *closed* list, 40 uses | #292's forum, which currently has a gap and no subject |
| **Mixed commits** | one repository spans both registers in 27 of 37 off-vocabulary commits | A conduct norm — [`PHASES.md:74`](../../yidam/prelude/PHASES.md#L74)'s rule about not mixing phase types, applied one level out |

**A repository with an object has two commit registers and yidam models one.**

### Two registers, one classifier — run, not argued

Every commit-reading surface here classifies from the subject line alone.
[`git.rs:81`](../../yidam/prelude/sdks/rust/src/git.rs#L81)'s `classify_commit(hash, message)`
takes no paths; its totality — Epistemic is the default — is proved in
[`graph.dfy:5`](../../yidam/prelude/sdks/spec/graph.dfy#L5); `yidam log` consumes it verbatim at
[`log.rs:118`](../../yidam/cli/src/cmd/log.rs#L118); and `lint --commits` reads
`--format=%H%x00%P%x00%s` — hash, parents, subject, **no paths** —
([`commits.rs:31`](../../yidam/cli/src/cmd/lint/commits.rs#L31)).

Run the classifier over an artifact-register commit stream (the Python SDK, same fixtures):

```sh
cd yidam/prelude/sdks/python && python3 -c "
from yidam_core.git import classify_commit, is_recognized_verb
for m in ['feat: add dark mode', 'fix: guard the empty query', 'test: cover the pagination edge']:
    v = m.split(':')[0]
    print(m, '->', classify_commit('abc1234', m).kind, '| recognized:', is_recognized_verb(v))"
```

```text
feat: add dark mode -> CommitKind.Epistemic | recognized: False
fix: guard the empty query -> CommitKind.Operational | recognized: True
test: cover the pagination edge -> CommitKind.Epistemic | recognized: False
```

Three commits that touch only the artifact get three different treatments, none of them about
the artifact: `feat:` and `test:` are reported by `lint --commits` as corpus-vocabulary
violations and counted by `log --epistemic` as **testimony**; `fix:` — which happens to be in
the closed vocabulary as *"a defect corrected"* — sails through lint silently and lands in the
corpus's operational history. The register-bleed number above is therefore an *undercount*: an
object-coupled repository's `fix:` commits are being absorbed into the corpus record without
even registering as noise.

Outside-ratio across the measured population ranges **6% to 100%**, so this is not a corner
case; it is most of what separates one object-coupled repository from another.

### Projection — the largest corpus is not in git

The largest repository in the population — **1,656 commits** — gitignores `/.yidam/*` on
purpose and regenerates 744 corpus files (492 corpus, 236 catalog) from its own data via its own
mirror command, negating exactly three files back in — `config.toml`, `authorship.yml`,
`lint-baseline.yml` — so it can still run the gate (#582). It read the model and decided the
corpus is a projection of a system whose source of truth is elsewhere. `GRAPH.md`'s premise —
*the files are the data, `git log` is the audit trail* — is plainly false there: the arrow runs
**object → corpus**, the corpus has no history of its own, and `replay`, `--at`,
`log --epistemic` and every residence clock answer nothing. The model has no word for this, and
a second repository reaching the same conclusion would have to re-derive it — and might negate a
different three files.

## Proposal

### 1 — What a kuten is, and the slots it declares

A kuten is one profile under `kuten/` in the template, **vendored at genesis** into
`.yidam/.vendor/prelude/kuten/` like the rest of the prelude, selected in the bootstrap
dialogue, recorded with its revision in `.yidam/decisions/kuten.yml`, and reported by `doctor`.
A repository holding **no** kuten is a supported state and reports as one — that is all eighteen
measured corpora today, and A2 ships no behaviour change (#574: every corpus under `examples/`
produces byte-identical output from every existing command, before and after).

**One kuten at A3: `inquiry`, plus the `object` slot** (#572, scope decision 1 — revised twice
before filing, and not re-litigated here). The evidence for a second practice was an artifact of
the two controls; a second profile waits for a second instance at a comparable vintage.

The slot inventory, with A0's verdicts as #572 records them:

| Slot | A0 verdict | Where it lands |
|---|---|---|
| **phases** — the valid phase types | real (0–26% use among the nine with the verb) | A3, §3 |
| **vocabulary** — the registers, and a glossed subset | real, and specified wrongly by the plan: register scoping is the primary job, narrowing the secondary benefit | A3, §4 |
| **classes** — the shape of the corpus the practice accretes | real (extraction target: 0.50–1.11 nodes/commit, 35–62 line medians) | A2 |
| **object** — the artifact outside the corpus, and its direction | real — the one axis that breaks conformance | A3 §6, A6 |
| **dialogue** — what the bootstrap asks | real | A2 |
| **skills** — what the practice routes through | real | A2 |
| **rubric** — the criteria a contribution is scored by | real | **A5, co-designed with #286** (scope decision 3): a rubric built alone would be `escalate_after`'s argument violated at rubric scale — *"a value compiled into the binary would be one corpus's answer imposed on every other"* ([`config.rs:52-53`](../../yidam/cli/src/config.rs#L52-L53)) |
| **clocks** — proposed `[due]` values | premature: config empty in 17 of 18 — ships as a **proposal with values**, not a permission with blanks | A2, §9 |
| **policy** — proposed severities and overrides | premature: no `.rego` in eighteen — ships as a **proposal with values**, through RFC-0024's layer, visible as an override | A2, §7 row 5 |
| **question-pressure** — what kind of question this corpus should open | not measurable (nothing existing creates it); settled in #572's negotiation | A3, §5 |

The verdict sentences are #572's verbatim: six varied and are real (phases, rubric, classes,
object, dialogue, skills); one is real and was specified wrongly (vocabulary); two are premature
rather than absent (the config values and the policy overrides). Where A0's working notes
enumerate slots this table folds together — #572 counts eleven — the working notes are
authoritative for the count; the verdicts and their assignments are not in question. Flagged in
Open questions.

### 2 — The revision model

A kuten is vendored, and A0's whole correction was that a repository works from the prelude it
**vendored**, not from current yidam. Without a revision, `score` would score repositories
against a kuten they may not hold and `fit` would compare repositories holding different ones —
designing A0's own confound into A0's deliverable. So (#572, scope decision 6):

- **The kuten carries a revision**, recorded in `.yidam/decisions/kuten.yml` at genesis and on
  every re-vendor.
- **Cross-revision comparison refuses or annotates, never silently proceeds.** The precedent is
  the harness's cross-`PROTOCOL_VERSION` rule
  ([`VERSIONING.md:126-129`](../../VERSIONING.md#L126-L129)): comparisons are valid only at the
  same version, and the tool *"rejects cross-version diffs with an explicit error rather than
  silently producing misleading output."* `score` (A5) and `fit` (A7) inherit that shape
  verbatim.
- **A kuten may change after genesis** as a `decide:` commit with a superseding decision record
  (scope decision 4). `replay` marks the discontinuity rather than smoothing it, and `score`
  refuses a range spanning one.
- **Every consumer reads the vendored kuten, never the template's current one.** `kuten check`
  must not report a vintage artifact as a divergence: a repository whose vendored `GRAPH.md` has
  no `phase` verb has not stopped running phases; it never could (#574). Distinguishing *a
  repository that does something else* from *a repository whose vendored prelude could not have
  done this* is the whole lesson of A0's retraction.

### 3 — Phase composition: the kuten declares the types; the run record feeds the shipped derivation

Two open children rewrite `cmd/phases.rs` — #575 its **type** half (the enumeration moves from
the binary into the vendored kuten) and #473 its **state** half (a phase gains an input-state
snapshot and a run record) — and until now neither issue named the other. This section is the
one place the composition is decided; #473's restatement (its thread, 2026-09-04) defers both
calls here explicitly.

**The kuten declares the valid phase types.** `PHASES.md` keeps the discipline prose — one
phase one branch, settle with a merge, bound phases, do not mix types — because none of that is
kuten-specific. What moves is the enumeration: `yidam phases` reads the vendored profile, and a
repository holding no kuten gets today's four types and reports that it is using the default
(#575).

**The run record stores a phase's declared type and an input snapshot that names the kuten
revision.** #473 (as restated) gives `phase start` a snapshot — the sha, the manifest digest,
and **the kuten revision**, with the declared type validated against the vendored kuten's list
at start. The sha technically pins the vendored kuten in-tree already; the record names the
revision anyway, for RFC-0026's own reason — *"an equality check rather than a heuristic."* A
phase resumed across a re-vendor is then detectable by comparing two fields, and §2's
refuse-or-annotate rule applies to it: a run record must not validate its type against a list
that changed under it.

**#473 owns the `cmd/phases.rs` rewrite, sequenced before #575.** One issue rewrites the file;
the other reads the result. #473 lands the record and the state derivation; #575 lands the
declared enumeration on top of it.

**Decided: the run record feeds the shipped `RefKind` derivation; it does not replace it.**
#473's original DoD said *"the ref-shape inference is deleted, not left as a fallback"* — written
without noticing that `42354da`'s `RefKind::{Position,Evolution,Phase}` split, with `phase_tally`
and pinning tests, had been on main since 2026-08-22 and is what fixed #272's
26-active-phases-against-a-true-count-of-1. The restatement struck that clause; this section
makes the architectural call it deferred. The argument for feeding rather than replacing:

1. **Refs without run records exist forever.** Every phase in every existing repository, and
   every phase a person opens by hand — `git switch -c phase/<name>` is `PHASES.md`'s own
   documented flow. A derivation that reads only the record would mismeasure the entire A0
   population, which is the vintage error A0 exists to warn against.
2. **Deleting the ref-shape mechanism re-opens a closed defect.** `42354da`'s tests pin the
   26-vs-1 count on a repository that reproduces it; tearing the mechanism out to re-earn the
   number from a record is work the restatement on #473 already struck.
3. **The two sources answer different questions, and the shipped code says so.**
   `ref_state` is documented as *"the single classifier. `yidam status` counts these and
   `yidam phases` prints them, and they must not be able to disagree"*
   ([`git.rs:241-244`](../../yidam/cli/src/git.rs#L241-L244)). `RefKind` answers *what is
   this ref*; the run record answers *what happened in this run*. Collapsing them recreates
   #272's actual defect — two surfaces free to disagree — one level up.

Concretely: `RefKind` remains the namespace classifier. Where a run record exists for a ref, the
record is authoritative for **state** (active, interrupted, settled) and carries the declared
**type**; where none exists, state stays ref-derived and the row says it is inferred. One
classifier, two evidence sources, ranked — not two classifiers.

### 4 — Registers scope recognition, never classification

The register declaration: the `object` slot names the object's paths; the corpus register is
`.yidam/**` plus whatever else the declaration claims for the corpus; everything in the object's
paths is the artifact register. `lint --commits` then reports `feat:` on the artifact as nothing
at all, and `establish:` on the corpus exactly as before. **A commit touching both registers is
a conduct finding with its own message** — `PHASES.md`'s do-not-mix rule applied one level out —
not a vocabulary finding. Severity is the kuten's to propose and the corpus's to override, per
§7 row 5; the proposed default is Warn, the severity `unrecognized-verb` already carries,
because history cannot be rewritten to fix it.

The load-bearing question is *where the register split lives relative to `classify_commit`* —
because `classify_commit` is a parity function fixtured in three SDKs, its totality is
Dafny-proved, `yidam log` calls it directly, and RFC-0026's invariant is built on it: *classify
every commit a run wrote by leading verb, and assert the epistemic ones are all on a `propose/*`
ref.* Whatever is decided, one constraint is non-negotiable:

> **`yidam log`, `lint`, and RFC-0026's invariant test must classify the same commit the same
> way.**

Two arms, both argued:

**(a) A shared pre-classification path filter in yidam-core, parity-tested.** The register
becomes a fact about the commit established *before* classification, in one function all three
SDKs implement and fixture; `log`, `lint`, and the invariant test consume the filtered
population and agree by construction. This is the strongest form of the constraint — and it is a
parity-surface change, which this RFC's own DoD forswears and which the RFC-0017/0018 precedent
prices at one RFC per contract change. Under this arm, the present RFC records the decision and
the change ships under its own follow-up RFC. The costs that must be paid there: every consumer
grows a path read (`log` and `lint` today read subjects only — the format strings above); the
fixture model changes shape, because
[`parity/fixtures/classify_commit/`](../../yidam/prelude/sdks/parity/fixtures/classify_commit)
is `(hash, message) → (kind, verb, subject)` with **no repository in it**, and a register filter
takes paths *and a declaration* as input; and three implementations plus the VS Code extension
must agree about a file only the CLI consumes — the exact shape RFC-0019 declined (*"would have
to arrive as a new function on every implementation to serve one consumer"*) and RFC-0018 ruled
against as precedent (a new surface is a CLI surface, not a fourth parity function).

**(b) Capabilities never declare writes on object-register paths; `classify_commit` untouched.**
The register split scopes **recognition** — whether the corpus vocabulary governs a commit — and
never **classification**. The distinction is already load-bearing in the code:
[`git.rs:75-76`](../../yidam/prelude/sdks/rust/src/git.rs#L75-L76) keeps `is_recognized_verb`
deliberately separate from `classify_commit` — *"Recognition is the question of whether the log
is legible; classification is the question of what a commit did"* — and classification **must
remain total**. Under this arm, `lint --commits` gains the path read and applies the register
filter before the recognition check; `log`, the SDKs, the Dafny proof and the fixtures are
untouched; and the invariant test's population is kept register-pure by a manifest rule:
**`capabilities.toml` `writes` globs must lie inside the corpus register, and the executor
refuses a manifest that declares outside it** — checked at declaration time, which is RFC-0026's
own property (*"decidable before the step runs"*).

The constraint holds under both arms. Under (a) by construction. Under (b) because
classification is one total function everywhere — `feat:` on the artifact is Epistemic to all
three consumers alike — and only *jurisdiction* differs: `lint` declines to report a commit the
declared vocabulary does not govern, and the invariant test never meets an artifact-register
commit from a run because no run may be declared onto those paths. No surface ever calls the
same commit two different kinds.

**Decided: arm (b), and the reason is the fixture model.** A register-aware classifier is
corpus-relative — the same commit classifies differently in two repositories holding different
declarations — and a corpus-relative parity function cannot be pinned by fixtures of the shape
the parity surface is built on. The harm A0 measured lives in `lint`'s report (violations that
are not violations) and nowhere else: no gate consumes `log`'s tallies, and the kind of an
out-of-register commit is consumed by nothing that acts on it. So the fix belongs where the harm
is. The residue is stated rather than waved away: under (b), `log --epistemic` on an
object-coupled corpus still counts artifact commits in its tallies. If that is ever measured to
mislead a reader — not merely to look untidy — the remedy is a register column in `log`'s
*presentation*, read from the same vendored declaration by the CLI, which forks nothing; and if
a second consumer of register jurisdiction appears, the filter gets one shared home in the CLI
rather than a second inline copy. Arm (a) remains the recorded escalation path, under its own
RFC, if the register ever genuinely needs to be a parity-visible fact.

### 5 — The question-pressure slot, and #578's disposition

The kuten declares **what kind of question this corpus should be opening** — the one generative
element in the layer. Its constitutional footing is RFC-0020's, and it is specified in those
terms or not at all: **opening a question asserts nothing the work did not already assert**,
which is exactly why `propose` is licensed to draft `open:` and not `establish:`. This slot
creates pressure toward a kind of question; it does not author one. `kuten check` reports the
divergence (*"inquiry opens epistemic questions; this corpus has opened none in two hundred
commits"*), `cycle` (A4) may name it as a next act, and nothing writes. #575's DoD pins it: the
slot authors nothing, and a test asserts it.

The slot has two halves and only one can be built:

- **The epistemic half ships standing alone** — pressure toward `open:` questions about
  understanding, which `inquiry` needs and which depends on nothing outside this epic. This is
  already recorded in #573 and is restated here as the build order.
- **The coverage half has nothing to point at until #578 lands.** A corpus completing a series
  can only be pressed to open *coverage* questions if a class can declare what its instances
  span — and that is a class-contract change whose precedent is `edge_policy`, living in
  `.ont.yml` with the ontology lineage, deliberately filed outside this epic (#572, scope
  decision 5). The slot's schema **reserves** `kind: coverage` as a named, unimplemented value —
  naming the state rather than leaving a blank to be invented twice — and the only prose the
  model has about coverage today is one unenforceable sentence about the `scope` verb
  ([`GRAPH.md:449-450`](../../yidam/prelude/GRAPH.md#L449-L450)).

**#578 is unscheduled, on the record.** No track in the current iteration carries it. Two
conditions would schedule it, either sufficing: a second series-completing corpus appears at a
comparable vintage (the same bar scope decision 1 sets for a second kuten), or the measured
outlier's practice files a concrete upstream need. Until then the reserved `coverage` kind is
the whole of this epic's interface to it.

### 6 — The object slot carries a direction, and projection is a declared state

**Decided: the `object` slot declares its direction — `authored` or `projected` — and
projection is endorsed rather than ruled out.** #582's evidence is n=1 and n=1 understates it:
1,656 commits, the largest repository in the population, and a deliberate reading of the model
rather than a careless one. Ruling projection out would define the model's largest real instance
as misuse while its `.gitignore` argues its case; leaving it neither endorsed nor excluded
forces every repository that reaches the same conclusion to re-derive it.

- **`authored`** (the default, and the only value `inquiry` proposes): the corpus is written in
  git; `GRAPH.md`'s premise holds; every history-derived surface applies; A6's coupling checks
  (#577) run corpus → object, reusing RFC-0019's `cites:` — a verbatim span plus pin and
  standing, deliberately not an edge.
- **`projected`**: the corpus is regenerated from the object by the repository's own tooling;
  the arrow runs object → corpus; `git log` is the audit trail of the *project*, not the corpus.
  The declaration makes the consequences explicit instead of silently empty: `replay`, `--at`,
  `log --epistemic`, the residence clocks and `kuten check`'s history half report **not
  applicable by declaration** rather than answering nothing; the gate, `query` and the exports
  still run; `doctor` names the state — which also answers #582's third question, since an
  undeclared untracked corpus is today indistinguishable from no corpus at all; and A6's
  coupling checks do not run, because the projection *is* the coupling.

This answers #582's three questions in order: a projected corpus **is** a corpus (it gates,
queries, exports) whose epistemic-history surfaces are declared inapplicable; the model **does**
say so, in the slot; and `doctor` says which state a repository is in. **The decision adopts
#582 into this epic:** the direction field and the `doctor` line become A3 acceptance criteria
(#575), the history-surface behaviour lands with them, and #582 closes when they do — not
before, because a decision a reader cannot yet see in a report is exactly the
surface-with-no-consumer failure this repository keeps finding.

### 7 — The invariant: five prohibitions, each guarded, each guard mutation-tested

A kuten may not widen the model. Transcribed from #572; the guards land with A3 (#575), and each
one is **mutation-tested** before it is trusted — a guard that greps a whole file is satisfied
by that file's own comments.

| Prohibited | Because | Instead |
|---|---|---|
| Add a commit verb | The closed vocabulary ([`GRAPH.md:416-417`](../../yidam/prelude/GRAPH.md#L416-L417)) is what makes `log --epistemic` decidable, and `classify_commit` is a parity function pinned by fixtures in three SDKs | Declare a **subset** and gloss it. A needed-and-absent verb is evidence for #292, not a patch |
| Add or alter a claim standing | Article V reads the standings as a total order when it licenses lowering a claim at resolution ([`CONSTITUTION.md:72-76`](../../yidam/prelude/CONSTITUTION.md#L72-L76)) | Nothing. This is constitutional |
| Contradict Articles I–VI | Article I — the prelude is not subject to resolution, and a kuten is vendored prelude | A domain extension appended at genesis, which the constitution already provides for |
| Change the graph encoding | Files are nodes, links are edges, commits are events. This is the premise, not a policy | Nothing |
| Loosen a gate quietly | RFC-0024 settled that a local rule may be more permissive and may not be *silent* | Surface it three ways as an override already is: `policy check`, an `Info` lint finding, and `doctor` |

### 8 — Article V and the kuten

*This section is the constitutional argument #573 requires and #287 cites; it is made once,
here.*

A kuten makes claims about **how the work goes**, not about what the corpus knows. #573's
framing — that this is *probably* outside Article V's scope, and probably is not good enough to
build on — is resolved here the way
[`sangha.rs:14-16`](../../yidam/cli/src/cmd/sangha.rs#L14-L16) resolved its own limit: as a
constitutional finding with a stated ground, not a scoping convenience.

**The lineage.** This is the third application of one licence, each step one level further out:

1. **RFC-0020, for findings:** a proposal is legal iff it carries what a finding or a corpus
   declaration already said — `transport`'s licence
   ([`GRAPH.md:459-461`](../../yidam/prelude/GRAPH.md#L459-L461)): *carriage and not synthesis,
   which is what makes it legal outside a resolution event*, because carrying introduces no
   node, edge or claim its author did not hold.
2. **RFC-0026, for executions:** a run authors operational commits directly; every epistemic
   commit goes to a proposal branch; nothing merges itself. Carriage applied from what a
   proposal may say to what an execution may author.
3. **This RFC, for conduct:** a kuten is generated, vendored pressure on conduct that a reader
   will treat as authoritative and that no elector holds — and it is licensed because **it
   asserts nothing about the corpus and binds nobody about the work.**

**What vendored pressure may assert.** Article V binds three objects — nodes, edges, claims
(with claims bound through the standing;
[`CONSTITUTION.md:65-76`](../../yidam/prelude/CONSTITUTION.md#L65-L76)). A kuten introduces
none of them, and the five prohibitions in §7 are that fact made mechanical: no verb, no
standing, no encoding change, no contradiction of I–VI, no quiet loosening. Its one generative
element — question pressure — is licensed on RFC-0020's exact ground and authors nothing (§5).
Its clock values are proposals a corpus's own config holds or declines (§9). Its severities
enter only through RFC-0024's layer, visible as overrides. So the kuten operates entirely below
Article V's objects: it parameterizes *which acts the loop invites*, never *what the corpus
holds*.

**In whose name.** Upstream authors the profile; **the corpus declares it**. The genesis
selection is recorded in `.yidam/decisions/kuten.yml`, and a post-genesis change is a `decide:`
commit with a superseding decision record (§2) — the same act, by the same authors, as any other
decision the repository makes about itself. A reader who treats the kuten as authoritative is
reading a declaration the corpus made about its own practice, on its own record, revisable by
its own mechanism — not a claim upstream made about the corpus. That is why *vendored* is
load-bearing rather than incidental: the text a reader sees is the text the corpus adopted, at a
revision it can name.

**And it binds nobody.** Divergence from the kuten is a question for a person, never a defect:
`kuten check` exits zero, on `due`'s argued precedent that *a corpus with three expired sources
is not unhealthy, it is owed*. Anything that refuses — a gate, a severity — enters through the
policy layer and is visible three ways (§7, row 5). The advisory character is constitutional,
not provisional: a future slot that would author or refuse must arrive through the doors already
licensed for those acts (`propose` for authorship; a policy override for refusal), not through
the kuten growing teeth.

**The binding rule, destined for vendored prelude text.** Following the `2fcbd19` division —
the rule lands in the vendored text where it binds a repository that never reads an RFC; the
argument stays here — A2 vendors this paragraph at the head of the kuten profile document:

> A kuten declares what this corpus's practice is aimed at. It narrows and parameterizes the
> loop; it may not widen the model: it may not add a commit verb, add or alter a claim
> standing, contradict Articles I–VI, change the graph encoding, or loosen a gate except as a
> visible policy override. It asserts nothing the corpus holds — no node, no edge, no claim,
> no standing — and it binds nobody: divergence from it is a question for a person, not a
> defect. It speaks in this corpus's name from the decision record that adopted it, and it
> changes only by a superseding decision.

**The #287 boundary.** The practice document is this argument's second consumer, and its scope
is decided here so it is not re-argued there: **#287 measures conduct against the declared
kuten, sharing `kuten check`'s divergence semantics** — divergence is reported as a question,
vintage is never reported as divergence, and the document is regenerated, never authored. Where
a repository holds no kuten, the document describes conduct without a baseline and says so; it
is not scoped to undeclared conduct only, because the declared baseline is precisely what makes
its findings answerable to something the corpus chose. #287 cites this section — *RFC-0028,
"Article V and the kuten"* — and discharges its constitutional note by citation rather than by
a second argument.

### 9 — The surfaces, and the one line about `due`

A2 (#574) builds the reporting half, specified there and only constrained here: `yidam kuten`
writes the `AGENTS.md` REGEN block (regenerated, never hand-copied — a hand-copied declaration
is one re-vendor away from being silently wrong); `yidam kuten check` reads the vendored
declaration and the history and reports divergence, read-only, exit zero, on RFC-0001's
contract; `doctor` reports which kuten is held and at what revision. A2's proof obligation is
A0's cluster run backward: a declared `inquiry` that fails to recognise the six repositories
which defined it is a wrong extraction.

And one transcription line, so `due` never grows a precedence rule:

> **`due` reads only `[due]` keys; the kuten proposes values, never holds live ones.**

`cmd/due.rs` declares its intervals *"never compiled in"* and reads them from
`.yidam/config.toml` ([`due.rs:217`](../../yidam/cli/src/cmd/due.rs#L217),
[`356`](../../yidam/cli/src/cmd/due.rs#L356),
[`444`](../../yidam/cli/src/cmd/due.rs#L444)); the kuten's clock slot is a proposal the
bootstrap offers and the corpus's config holds or declines. There is exactly one live home for
an interval, and this RFC adds no second one — the same sentence RFC-0026 wrote about staleness.

## What this does not do

- **No parity-surface change and no MCP contract change.** The one candidate parity change —
  the register filter — is argued in §4, decided against, and its escalation path priced at its
  own RFC. `classify_commit`, the fixtures, and the Dafny proof are untouched.
- **Not #460 / RFC-0026.** That answers *what a run may author* — permission. A kuten answers
  *what work is worth doing* — priority. They compose: `capabilities.toml` says what can run,
  the kuten says what should — and §3 and §4 are the two seams where they meet, decided once.
- **Not E4 / #252.** A kuten proposes no commits. It declares what a good one would have been.
- **Not a scheduler.** `mise` and CI sequence; `due` says it is time; `cycle` (A4, blocked on
  #474's contract question, which this RFC does not touch) says what is next.
- **No new rule on corpus content.** Every severity a kuten sets is a policy override, visible
  as one.
- **Does not build the object.** yidam governs the corpus; the corpus governs the build.
- **Not #578.** Coverage is a class-contract change with the ontology lineage; this RFC
  reserves the slot value and schedules nothing (§5).
- **`due` is unchanged** (§9).

## Open questions

1. **The slot count.** §1's table folds slots A0's working notes may enumerate separately
   (interval values and threshold values both live in `config.toml`; severities ride the policy
   slot). #572 counts eleven; the working enumeration is authoritative for the count and should
   be reconciled into the table when A2 extracts the profile.
2. **Where the vendored binding rule lands.** §8 fixes the text and its destination class
   (vendored prelude, at the head of the kuten profile document); whether a one-line pointer
   also belongs in `GRAPH.md` or `CONSTITUTION.md`'s commentary is A2's placement call.
3. **The mixed-register conduct finding's proposed severity.** §4 proposes Warn by analogy with
   `unrecognized-verb`. A0 can be re-read for how often mixed commits occur in otherwise
   conformant repositories before A3 fixes the proposal.
4. **Whether `log` ever grows a register presentation.** The (b) residue in §4: left until a
   reader is measured to be misled, not merely until the tally looks untidy.
5. **The second kuten's trigger.** Scope decision 1 waits for a second instance at comparable
   vintage; A7's first controlled run (#288) is the natural place to check for one, and should
   record the check either way.
