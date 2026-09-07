# RFC-0028 — The form a practice takes (the kuten layer)

- **Status:** Accepted
- **Track:** I23
- **Relates to:**
  - RFC-0020 (the carriage lineage this extends a third step — from findings to executions to conduct)
  - RFC-0026 (the permission layer this composes with, and the two seams on `cmd/phases.rs` and `classify_commit` decided here)
  - RFC-0024 (the policy layer every severity a kuten proposes must enter through, visibly)
  - RFC-0019 (the citation contract the object slot's coupling checks reuse rather than re-invent)
  - RFC-0008 (the strict reading of Article V the constitutional argument here extends)
  - RFC-0001 (the report contract `kuten check` emits on)
  - RFC-0003 (the light binary it must run in)
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

Two of those four were falsified, by the population itself, eight days later. See "A0's bands,
corrected" below.

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
- ~~`.yidam/config.toml` is empty in **17 of 18** corpora, and **not one** carries a `.rego`
  override. A blank nobody has filled in eighteen tries is not a preference being expressed.~~
  **Corrected 2026-09-06 under #633 — see below.**

#### A0's config finding, corrected (2026-09-06, #633)

#633 was filed because the four `clocks` scalars had no falsifier: nothing consumed the slot and
nothing re-read the corpora's config, so no later measurement could contradict them. Re-measuring
the same eighteen corpora, read-only, contradicted them — and contradicted the finding they rest
on first.

| Slot value | A0 | Re-measured 2026-09-06 |
|---|---|---|
| `catalog.ttl_days: 180` | no measured interval | **182 declarations**, 165 of them inside the six-repository cluster the bands were extracted from: allen-county-ohio 129 (median 365), bitrecover-bitwipe 24 (90), hermetic-ch 12 (30). In-cluster pooled median **365**; 180 is the pooled 25th percentile and no member's median |
| `due.questions_after: 100` | no measured interval | 1 corpus holds one, at **200** — of **2** whose pinned binary could name the key when A0 ran |
| `due.phases_after: 60` | no measured interval | 1 holds one, at **50** — same denominator |
| `due.index_after: 25` | no measured interval | 0 hold one; 1 declined it in writing |
| `policy.proposes_overrides: []` | no `.rego` in eighteen | **stands.** No corpus carries a file in `.yidam/policy/`, which is where an override lives — the seven `.rego` files three corpora hold are vendored prelude policy, not overrides |

Two errors, different in kind.

**The TTL was read in the secondary form.** A0 read `[catalog] ttl_days` in `.yidam/config.toml`.
The per-entry `ttl_days:` is the primary one: [`configuration.md`](../configuration.md) says
*"The per-entry `ttl_days:` is the primary form, because a gauge record and a statute do not age
at the same rate"*, and `due` names both channels in one remedy line. The interval the slot
reports as nonexistent is declared 165 times inside the very cluster the profile was extracted
from.

**"Eighteen tries" was never eighteen.** No vendored document names `questions_after`,
`phases_after` or `index_after` — they are documented in `docs/configuration.md`, which
`clone` does not vendor — so the only channel that names them to a corpus is `due`'s own output
for an unset clock. That output landed 2026-08-30 and released in `cli/v0.7.0` the following day,
four days before A0. Two of the eighteen pinned a binary containing it at that moment.

The verdict those sentences supported is unchanged: these slots ship as **proposals with values**,
not permissions with blanks. A smaller denominator supports that more strongly than the larger one
did — a population that could not yet answer is more premature, not less. What is withdrawn is the
claim that there was nothing to measure, and with it the warrant for calling the four scalars
anything but an illustration until they are measured or retired. §1's `clocks` row carries the
retirement rule.

One caveat travels with the correction: eligibility is inferred from each corpus's pinned yidam
revision, never from an observed invocation. That instrument cannot separate *could not have known
the key existed* from *knew and declined*, and under the read-only constraint none available can.

#### A0's bands, corrected (2026-09-06, #644)

The same failure, one layer over. The four bands had no falsifier either: A0 published them as
ranges and no per-repository table, so nothing could ask whether a band still contained the
measurements it was fitted from. Re-measuring the six with the repository's own `kuten::measure`,
read-only, says two of them never did.

| Band | A0 | Observed 2026-09-06 | Re-fitted |
|---|---|---|---|
| `phases.commit_share` | 0.13–0.26 | **0.1250–0.2623** | **0.12–0.27** |
| `vocabulary.off_vocabulary_share` | 0.0–0.0 | **0.0–0.0164** | **0.0–0.02** |
| ~~`classes.nodes_per_commit`~~ | 0.50–1.11 | 0.5061–1.1096 | ~~0.50–**1.12**~~ |
| ~~`classes.median_node_lines`~~ | 35–62 | 35–62 | ~~35–62~~ |

**The bottom two rows are retired — see "A0's `classes` bands, retired" below.** This
correction fixed how they were quoted, and they were measuring the wrong thing.

**One cause, applied twice.** A0 quoted a min/max band to two decimal places and rounded
**inward**: 0.2623 became "0.26", 0.1250 became "0.13", 0.0164 became "0.0". Each rounding put
the repository whose measurement had set that endpoint outside the band it defined — which §9's
obligation calls a wrong extraction. `nodes_per_commit` escaped only because 0.5061 and 1.1096
happen to round outward at two decimals.

This is not drift. The member that fails both bands last committed **2026-08-20**, before A0 ran:
its numbers today are its numbers then. One member has genuinely moved — its phase share fell from
0.1279 to 0.1250 as its last hundred commits ran at 0.076 — and that is divergence, which is the
instrument working rather than an extraction to correct.

The zero has a second cause. Of the four off-vocabulary commits, two carry a valid verb with a
`(scope)` suffix [`GRAPH.md`](../../yidam/prelude/GRAPH.md) forbids, written at genesis under a
prelude that had not yet closed the list; the other two are coinages — `report:` and `publish:` —
against a closed one, which is the middle row of the table below. Stripping the suffix before
matching the verb is the one way to read this population as exactly zero, and it is a rule the
document explicitly refuses.

**What changes, so the next re-fit is legible rather than surprising.** The estimator is written
down — *the observed range, quoted to two decimal places, rounded outward* — the fit carries a
date, and the six measurements are recorded in the profile under `measured.members`. A guard reads
them back through `compare` and holds every band to containing its own evidence, which is the half
of §9's obligation that can run in CI while the corpora themselves cannot.

#### A0's `classes` bands, retired (2026-09-07, #692, profile revision 2)

The correction above is right and was not enough, and the reasoning it recorded is what this
supersedes. It stated:

> ~~a repository that changes its practice moves off a correctly-fitted band, and that is
> divergence rather than a wrong extraction~~

`allen-county-ohio` is one of the six that **defined** those bands. It left both of them within
**six hours** of the fit that used it — `measured.members` row 6 is that repository at `abd6ecd`
(12:58), and a clean clone at `9ed2281` (18:40 the same day) reads 1,372 commits, 683 nodes and a
64-line median, which `kuten check` reports as `[diverges]` on both. Its practice did not change.
It aged sixty commits along a trend it has been on for its entire life.

**The two `classes` bands measured how old a repository is.** The profile's own comment said they
were read at matched maturity; nothing read them at matched maturity. `compare(profile,
measurement, vintage)` takes no age term, and the fit spans 73 to 1,312 authored commits — an 18×
range — so the band width *was* the age range.

| | Measured over the six `measured.members` rows |
|---|---|
| Spearman rho(authored, nodes-per-commit) | **-0.886** |
| Band ceiling 1.12 | the **youngest** member, 81 nodes / 73 commits |
| Band floor 0.50 | the second-**oldest**, 125 / 247 |

Both quantities are monotone in age *within* one repository as well as across the six, so a band
on either is a window every member passes through and then exits — including all six that defined
it. Two derived corpora traverse `[0.50, 1.12]` and leave through the bottom; both traverse
`[35, 62]` and leave through the top.

**What it cost.** `divergent` on either metric was uninformative about practice: it reported a
corpus's stage. And §9's falsifier —
`every_band_contains_the_measurements_it_was_fitted_from`, which reads `measured.members` back
through `compare` — was a test of the **snapshot date** for these two rather than of the band. It
was green only because the rows are frozen at the fit.

**Retired rather than widened**, on the profile's own rule: a wider window is still a window, and
numbers chosen to fit are what this layer forbids. The quantities remain measured and reported
under `measurement`; nothing judges them. What replaces them is an age-invariant statistic — a
trailing-window accretion rate, or a declared decay curve with distance from it reported instead
of a box — and choosing between those needs the eighteen-corpus re-measurement A7 (#288) is the
instrument for. **The profile is at revision 2**, so a repository holding revision 1 is told its
declaration and its vendored profile disagree, which is the revision model doing its job.

The general lesson is the one worth carrying past this row: **measuring before writing a number
down is not sufficient.** Both bands were fitted correctly, from real measurements, over a real
cluster. What was never asked is whether the quantity they measure is a property of the practice
or of the calendar.

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
[`log.rs:119`](../../yidam/cli/src/cmd/log.rs#L119); and `lint --commits` reads
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
| **classes** — the shape of the corpus the practice accretes | ~~real (A0's extraction target: 0.50–1.11 nodes/commit, 35–62 line medians; re-fitted 2026-09-06 to 0.50–1.12 and 35–62)~~ — **retired 2026-09-07 at profile revision 2 (#692)**: both bands were monotone in repository age, so the slot measured stage rather than practice. The verdict stands as A0 recorded it; what was never asked is whether the quantity belongs to the practice or to the calendar | A2 |
| **object** — the artifact outside the corpus, and its direction | real — the one axis that breaks conformance | A3 §6, A6 |
| **dialogue** — what the bootstrap asks | real | A2 |
| **skills** — what the practice routes through | real | A2 |
| **rubric** — the criteria a contribution is scored by | real | **A5, co-designed with #286** (scope decision 3): a rubric built alone would be `escalate_after`'s argument violated at rubric scale — *"a value compiled into the binary would be one corpus's answer imposed on every other"* ([`config.rs:54-55`](../../yidam/cli/src/config.rs#L54-L55)) |
| **clocks** — proposed `[due]` and `[catalog]` values | premature, but not unmeasured (A0 correction, above): the `[due]` keys had a denominator of 2 when A0 ran and 1 corpus held two of them; `catalog.ttl_days` is declared 165 times inside the cluster, pooled median 365 against the proposed 180. Ships as a **proposal with values**, not a permission with blanks — now with the distribution beside it and a rule that retires it | A2, §9 |
| **thresholds** — proposed `[lint]`/`[propose]` values: `escalate_after`, `withdraw_uncited_after` ([`configuration.md`](../configuration.md)) | premature on `clocks`' evidence, and — unlike `clocks` — not a kuten's to propose either: escalating a finding to a build failure is a gate change that enters through RFC-0024's layer (§7, row 5), and drafting a withdrawal is authorship §8 declines. Ships **named and unpopulated** | A2 |
| **policy** — proposed severities and overrides | premature: no corpus carries an override in `.yidam/policy/` — re-verified 2026-09-06 under #633, and it stands on its own terms, though on the same corrected denominator as `clocks` (the Rego layer shipped four days before A0) — ships as a **proposal with values**, through RFC-0024's layer, visible as an override | A2, §7 row 5 |
| **question-pressure** — what kind of question this corpus should open | not measurable (nothing existing creates it); settled in #572's negotiation | A3, §5 |

The verdict sentences are #572's verbatim: six varied and are real (phases, rubric, classes,
object, dialogue, skills); one is real and was specified wrongly (vocabulary); two are premature
rather than absent (the config values and the policy overrides). The table carries **eleven
rows**, which is A0's working count: the config-values verdict covers two families that differ in
what a kuten may say about them, not merely in which file holds them, and separating them is what
the earlier fold to ten lost. `clocks` proposes values because `due` is advisory —
it says a corpus is *owed*, and exits zero. `thresholds` cannot, because `escalate_after` decides
when a finding fails the build and `withdraw_uncited_after` licenses a drafted deletion; a kuten
reaches neither act except through the doors §8 names for them. Settled in Open questions 1.

> **As built (A5, 2026-09-06) — the `rubric` slot, which this RFC names in the table above and
> specifies nowhere else.** §5–§9 are silent about it and §9's surface list names three, none
> of them a score. What shipped:
>
> - **The slot declares criteria and no bands.** `rubric.criteria` is a list of ids and
>   nothing else. Every other populated slot carries intervals measured over eighteen corpora
>   before they were written down, and the profile's own header says *"not one of those four
>   was chosen"*. What was measured for a rubric is that each criterion **discriminates**
>   across ranges — not what a good reading of one is — so a band here would be the number this
>   layer exists to refuse.
> - **Its reader is `block`**, and the layer document's `Read by` column says so. The criteria
>   land in the `AGENTS.md` declaration, which is where an agent meets them at session start —
>   before the work, which is the only place naming them changes anything. They are
>   deliberately not a `check` reader: `check` measures a repository's whole history against
>   bands, and a contribution is a range somebody chose.
> - **`yidam score <range>` reads them, and reports rows only.** No overall verdict, no
>   `conforming` flag, no band. A single number over a range of commits names a person's
>   session; what is defensible is a reading per criterion with the commits and nodes it came
>   from, so a reader can disagree by looking. Exit zero however it reads, on `cmd/kuten.rs`'s
>   rule — a score that gated would be a gate decided by the kuten, which is exactly why
>   `thresholds` ships empty.
> - **Three criteria, each measured over 10-commit windows across the derived corpora before
>   it was kept.** `register` — the epistemic share **of the recognized subset** (n=63; min
>   0.00, median 0.50, max 1.00; undefined in 9 of 72). `landing` — the share of surviving
>   added nodes with an inbound edge, through `orphan-in`'s own resolution and its class
>   exemption (n=41; 0.00 / 0.67 / 1.00). `questions` — how many of those nodes are open
>   questions, through `claims::is_open_question` (n=41, median 0.64), reported as a number and
>   never thresholded, because whether a corpus *should* be opening questions is §5's slot.
> - **`register` reports `unmeasurable` on a zero denominator, never 0.00**, and that is the
>   single behavioural rule the criterion turns on. `classify_commit` is total — Operational is
>   the listed case and everything else falls through to Epistemic, which the Dafny spec proves
>   — so the naive share reads 1.00 for a corpus whose subjects are conventional commits with
>   no recognized verb. The two readings disagree by up to 0.90 over the same range.
> - **Legibility is a precondition reported beside `register` and never a scored row.** The
>   share of authored non-merge commits whose verb is recognized at all is bimodal *by
>   repository* — 42 of 72 windows at 0.00, 9 at 1.00 — so it separates corpora that adopted
>   the vocabulary from ones that never did, which is not a property of a contribution.
> - **Three candidates were measured and rejected**, recorded in the model as
>   `score::CONSIDERED_AND_REJECTED` rather than in a commit message. Out-degree — #286's *"did
>   new nodes enter the graph reachable"* — is `orphan-out`, an error-severity gate that 0 of
>   2,736 nodes across sixteen corpora trip, so scoring it would measure the gate. The presence
>   of an `open:` commit appears in 2 of 72 windows. The naive epistemic share is the inversion
>   above.
> - **The no-kuten arm is the default, not a fallback**, because 0 of 18 derived corpora hold a
>   kuten and `migrate` has no retrofit path. It runs the same criteria and says the selection
>   is the template's rather than that corpus's.
> - **The cross-revision refusal of §2 fires, and is tested.** The decision record is a
>   committed file, so the declaration at either end of a range is readable with `git show`;
>   where the two differ, `score` names both and stops before comparing anything. It is the one
>   non-zero exit, and it is a refusal to answer rather than a verdict on the work. #662 is why
>   an integration test drives a range across a revision change: the harness's own instance of
>   this refusal has been inert for a protocol version because nothing tested that case.
> - **Deferred, named:** `standing` — a directional claim delta; the components exist and there
>   is no per-window distribution yet. And `sourcing`, which measured as a floor (29 of 35
>   windows at zero) and is not computable at a past ref at all, because the corpus
>   reconstruction is pathspec'd to `.yidam/corpus` and `.yidam/catalog` is therefore not in
>   any reconstruction.

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

Two open children rewrite `cmd/phases.rs` — #575 its **type** half (~~the enumeration moves from
the binary into the vendored kuten~~) and #473 its **state** half (a phase gains an input-state
snapshot and a run record) — and until now neither issue named the other. This section is the
one place the composition is decided; #473's restatement (its thread, 2026-09-04) defers both
calls here explicitly.

**The kuten declares the valid phase types.** `PHASES.md` keeps the discipline prose — one
phase one branch, settle with a merge, bound phases, do not mix types — because none of that is
kuten-specific. What moves is the enumeration: `yidam phases` reads the vendored profile, and
~~a repository holding no kuten gets today's four types and reports that it is using the
default~~ (#575).

> **Erratum 1 — struck 2026-09-06, building A3 (#575).** *"The enumeration moves from the
> binary into the vendored kuten"* is **false in both directions.** There is no phase-type
> list in the binary: no `PhaseType`, no `PHASE_TYPES`, no phase-type constant anywhere in
> `yidam/cli/src` outside `#[cfg(test)]`, and `cmd/phases.rs` has no type concept at any of
> its 441 lines — `PhaseRow` is `{name, state, ref_name, owner, started, commits}` and `state`
> is a lifecycle word derived from ref namespaces. And the move it describes **already
> happened in A2**: `kuten/inquiry/kuten.yml` carries `types: [Investigation, Extraction,
> Synthesis, Assessment]` and `Profile` parses them.
>
> The default clause is struck for the stronger reason that implementing it is the only way to
> *create* the artefact this section objects to — a four-element phase-type list compiled into
> the binary. `render_block`'s no-kuten arm prints no types at all, and that is correct.
>
> What survived is a guard rather than a migration. The enumeration lives in more than one
> document, and the pair was checked in one direction only: every declared type had to appear
> somewhere in `PHASES.md`, with a `len() == 4` beside it as the only thing catching a dropped
> one. Adding a fifth heading to `PHASES.md` was green; deleting the section that enumerates
> them and leaving one bold sentence elsewhere in the file was green. A3 replaces both with
> set equality against the `## Phase types` **section**.
>
> Nothing in any repository is typed: across the eighteen derived corpora there are 60
> `phase/*` refs encoding no type, 301 `phase:` subjects naming none, and no file under any
> `.yidam/` recording one. The enforcing consumer stays where this section already puts it —
> #473's `phase start`, validating a declared type against the vendored list.

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

> **Erratum 2 — annotated 2026-09-06, building A3 (#575).** **The clause stands and A3 honours
> it**: nothing in #575 touches `cmd/phases.rs`, `git.rs` or `cmd/status.rs`. Its stated
> rationale no longer holds, and the difference matters for whoever reads this next. The
> sequencing was argued as *one issue rewrites the file; the other reads the result* — but the
> only thing #575 was named as needing from that rewrite was the declared enumeration, and
> Erratum 1 records that A2 discharged it. So the clause is now a boundary rather than a
> dependency: A3 stays out of the file because two issues editing one file concurrently is a
> merge problem, not because A3 is waiting for anything in it.

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

The register declaration: ~~the `object` slot names the object's paths~~; the corpus register is
`.yidam/**` plus whatever else the declaration claims for the corpus; everything in the object's
paths is the artifact register. `lint --commits` then reports `feat:` on the artifact as nothing
at all, and `establish:` on the corpus exactly as before. ~~**A commit touching both registers is
a conduct finding with its own message** — `PHASES.md`'s do-not-mix rule applied one level out —
not a vocabulary finding. Severity is the kuten's to propose and the corpus's to override, per
§7 row 5; the proposed default is Warn, the severity `unrecognized-verb` already carries,
because history cannot be rewritten to fix it.~~

> **Erratum 5 — struck 2026-09-06, building A3 (#575); the register survives, its home moves.**
> The slot cannot carry paths. A kuten is an **upstream-authored** profile vendored unchanged,
> and `inquiry` is deliberately one profile across six object shapes; the only thing a corpus
> writes is `.yidam/decisions/kuten.yml`, which is `{kuten, revision}`. There is no channel by
> which a corpus supplies paths to it, and paths are a fact about a repository rather than
> about a practice. **The live register is `[object] paths` in `.yidam/config.toml`**, on the
> precedent §9 already argues for the clocks — *"the kuten proposes values, never holds live
> ones."* What the slot declares is §6's direction, which is a property of the practice. The
> register split itself is unaffected and still belongs in `lint --commits`, under arm (b).

> **The mixed-register conduct finding is struck to its own issue (#643)**, 2026-09-06, on the
> measurement Open Question 3 pre-registered *"before A3 fixes the proposed severity"*. It
> produces ~710 findings across six corpora that are already 100% vocabulary-conformant —
> grindcore 138/247, audio-effect-design 61/122, bitrecover-bitwipe 61/157, bitlocker 31/82,
> hermetic-ch 27/73, allen-county-ohio 392/1267 — most often on `regen`, the corpus → `web/`
> export, which is the one act whose job is to cross the registers. A rule that fires hardest
> on the best-behaved repositories teaches readers to ignore the line, which is `due`'s own
> argument. It needs an exemption for the export act, and that exemption needs its own
> evidence.

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

> **As built (A3, 2026-09-06).** Under arm (b), as decided. `Registers` in
> [`kuten.rs`](../../yidam/cli/src/kuten.rs) reads `[object] paths` and the filter sits in
> `cmd/lint/commits.rs`, applied before `is_recognized_verb`; `classify_commit`, the parity
> fixtures and `subject.violations[].rule` are untouched. Three rules the section did not
> state, each settled by measurement:
>
> - **A commit spanning both registers is governed by the corpus** and raises no new finding,
>   per the strike above. Re-measured 2026-09-06 under the maximal object declaration — every
>   top-level path but `.yidam/` — the filter silences **30** of matt-huffman's 132
>   off-vocabulary commits and **72** of ohio-education-funding's 211. The problem table above
>   quotes A0's 24-of-32 and 107-of-111 for a different pair, and 40/75 for these two; the gap
>   is method, since `--name-only` lists nothing for a merge.
> - **A commit listing no paths is governed by the corpus.** `git log --name-only` prints
>   nothing for a merge, so every merge arrives with an empty list, and reading that as
>   artifact work would silence the verb check on the commits where two threads join.
>   Re-measured 2026-09-06: **94** of matt-huffman's 132 off-vocabulary commits have no paths,
>   92 of them merges, and **35** of ohio-education-funding's 211. Absence of evidence is not
>   a declaration of jurisdiction.
> - **`kuten check` is not scoped by the register, and `lint --commits` is.** Measured under
>   `Registers::corpus_only()` across all eight relevant corpora, **zero** commits change
>   register — none of the six defining repositories holds a `.yidam/config.toml` at all — so
>   scoping `Measurement::off_vocabulary_commits` buys nothing today, and it would make a
>   band-checked quantity settable from a corpus's own config. The counterfactual measures the
>   lever: declaring every top-level path but `.yidam/` as the object moves matt-huffman from
>   0.1671 to 0.1291 and ohio-education-funding from 0.4930 to 0.3248.
>
> **One asymmetry ships open.** ~~`yidam vocabulary --check` runs in the commit-msg hook, before
> the commit exists, and takes no paths — so the hook still reports `feat:` on an artifact
> while `lint --commits` is silent.~~ **Corrected 2026-09-07 under #693 — see below.** Closing
> it means changing `check_subject`, which is frozen in
> [`sdks/parity/mcp/tools.json`](../../yidam/prelude/sdks/parity/mcp/tools.json). Recorded
> rather than fixed, and filed as its own issue.

#### The asymmetry's consumers, corrected (2026-09-07, #693)

**This project ships no commit-msg hook, and never has.** `git ls-files` names no hook script,
no installer and no `.githooks/`; none of the four derived repositories that upgraded installs
one, and `core.hooksPath` is unset in all of them. RFC-0014's open questions record the stance
the set already held — *"conformance, not hooks"* (RFC-0004), with a local hook merely
*optional*.

The asymmetry is real and the mechanism above was wrong, which matters because it decides where
a fix would go. `yidam vocabulary --check` takes a subject line and nothing else, and it has
three consumers:

| consumer | how it asks |
|---|---|
| a contributor at a terminal | `docs/contributing.md` tells them to run `yidam vocabulary --check "fix(cli): …"` by hand |
| the VS Code commit box | [`vocabulary.ts`](../../yidam/editors/vscode/src/vocabulary.ts) shells to `yidam vocabulary --check <subject> --format json` |
| the MCP tool `check_subject` | frozen in [`tools.json`](../../yidam/prelude/sdks/parity/mcp/tools.json); its input schema takes `subject` and nothing else |

None of the three has paths to read, because none of them is looking at a commit — all three
ask about a subject line *before the act*, which is the whole point of the surface. So the
remedy proposed in #652 — *"the hook reads the staged paths"* via `git diff --cached
--name-only` — closes nothing: there is no hook, and a contributor typing a subject at a
terminal has no staged set that means anything about it.

What #652 gates itself on — *"measure whether it misleads anyone"* — is now answerable against
those three consumers rather than against a script nobody runs, and two derived repositories
declare `[object] paths` as of 2026-09-07.

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
  ([`GRAPH.md:482-483`](../../yidam/prelude/GRAPH.md#L482-L483)).

**#578 is unscheduled, on the record.** No track in the current iteration carries it. Two
conditions would schedule it, either sufficing: a second series-completing corpus appears at a
comparable vintage (the same bar scope decision 1 sets for a second kuten), or the measured
outlier's practice files a concrete upstream need. Until then the reserved `coverage` kind is
the whole of this epic's interface to it.

> **As built (A3, 2026-09-06) — the example rule above is not the rule that shipped.** This
> section's illustration — ~~*"this corpus has opened none in two hundred commits"*~~ — counts
> `open:` commits, and measured against the six repositories that produced every band in this
> profile it fires against two of them: **bitlocker and hermetic-ch have zero `open:` commits**
> while holding 27 and 15 open-tagged corpus files. §9's own obligation calls that a wrong
> extraction. So the pressure is measured over the corpus's **open questions**, through
> `claims::is_open_question` — the predicate `yidam open-questions`, `due`, `lint --history`
> and the MCP server already share, frozen in `sdks/parity/mcp/tools.json`. Re-measured with
> that predicate on 2026-09-06, all six hold open questions: 14, 26, 13, 51, 115 and 436,
> against 73, 82, 122, 157, 247 and 1,278 authored commits. `kind: coverage` parses and yields
> `unmeasurable` naming #578, and the divergent arm asks a question and writes nothing — a
> test compares the whole tree, byte for byte, either side of the check.

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
(#575), ~~the history-surface behaviour lands with them~~, and #582 closes when they do — not
before, because a decision a reader cannot yet see in a report is exactly the
surface-with-no-consumer failure this repository keeps finding.

> **As built (A3, 2026-09-06).** The slot declares `direction` and **nothing else** — see
> Erratum 5 in §4 for why it cannot carry the object's paths and where they live instead. The
> direction reaches two readers: `doctor`'s kuten line names the state, and the `AGENTS.md`
> block spells out for a `projected` corpus which surfaces stop answering. The
> *history-surface behaviour* did not land with them, and Erratum 4 in "What this does not do"
> records why: making the residence clocks answer differently is a change to `due`, which the
> non-goals forbid in the same document. The conflict is recorded there and deliberately left
> for an issue with a repository actually holding `projected` behind it.

### 7 — The invariant: five prohibitions, each guarded, each guard mutation-tested

A kuten may not widen the model. Transcribed from #572; the guards land with A3 (#575), and each
one is **mutation-tested** before it is trusted — a guard that greps a whole file is satisfied
by that file's own comments.

| Prohibited | Because | Instead |
|---|---|---|
| Add a commit verb | The closed vocabulary ([`GRAPH.md:449-450`](../../yidam/prelude/GRAPH.md#L449-L450)) is what makes `log --epistemic` decidable, and `classify_commit` is a parity function pinned by fixtures in three SDKs | Declare a **subset** and gloss it. A needed-and-absent verb is evidence for #292, not a patch |
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
   ([`GRAPH.md:580-582`](../../yidam/prelude/GRAPH.md#L580-L582)): *carriage and not synthesis,
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

That obligation went unchecked for eight days, because A0 published ranges and no
per-repository table, and two bands failed it from the day they shipped ("A0's bands,
corrected", above). It is checked now: the profile records the six measurements it was fitted
from, and a guard reads them back through the same `compare` the command runs. The obligation
is dated — a member that later changes its practice moves off a correctly fitted band, and
that is divergence, not a wrong extraction — so a re-fit is an act with a date on it, recorded
in `measured.fitted` beside the estimator it used.

And one transcription line, so `due` never grows a precedence rule:

> **`due` reads only `[due]` keys; the kuten proposes values, never holds live ones.**

`cmd/due.rs` declares its intervals *"never compiled in"* and reads them from
`.yidam/config.toml` ([`due.rs:218`](../../yidam/cli/src/cmd/due.rs#L218),
[`357`](../../yidam/cli/src/cmd/due.rs#L357),
[`445`](../../yidam/cli/src/cmd/due.rs#L445)); the kuten's clock slot is a proposal the
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
- ~~**`due` is unchanged** (§9).~~

  > **Erratum 4 — conflict recorded 2026-09-06, building A3 (#575); deliberately not
  > resolved.** This line and §6 cannot both be true. §6 says that under
  > `object.direction: projected` *"the residence clocks … report **not applicable by
  > declaration** rather than answering nothing"*, and the residence clocks are `cmd/due.rs`.
  > A clock that reads a vendored declaration and changes what it reports is `due` changed —
  > and §9's transcription line, *"`due` reads only `[due]` keys"*, was written to stop
  > exactly that kind of second input growing into it.
  >
  > **A3 changes neither**, and that is the decision rather than an omission. `due` is
  > untouched here; a projected corpus's clocks report today what they reported yesterday.
  > Resolving the conflict means choosing between two coherent positions — a projection-aware
  > `due` reading one more vendored fact, or a `due` that stays a function of `[due]` keys
  > while some other surface carries the declaration — and neither is A3's to pick while
  > building the slot the choice is about. What A3 owes is that the declaration exists, is
  > reported, and can be read: `doctor` names the direction and the `AGENTS.md` block spells
  > out which surfaces a projected corpus should not expect answers from. The behaviour change
  > §6 describes needs its own issue and its own evidence, from a repository actually holding
  > `projected`.

## Open questions

1. ~~**The slot count.**~~ **Settled 2026-09-05, reviewing A2 (#574).** The count is **eleven**,
   and §1's table now says so. The fold to ten combined the config values with the policy
   overrides and lost the `[lint]`/`[propose]` threshold family — `escalate_after`,
   `withdraw_uncited_after` — on the reasoning that both families live in `config.toml`. Which
   file holds a value is not what the slot table is enumerating: it enumerates what a kuten may
   say, and the two answer differently. `due` is advisory, so a kuten proposes intervals; the
   thresholds decide a build failure and a drafted deletion, so a kuten names the slot and
   populates nothing. Undoing the fold also stops the layer disowning the one quote it is built
   on — `escalate_after`'s *"a value compiled into the binary would be one corpus's answer
   imposed on every other"* ([`config.rs:54-55`](../../yidam/cli/src/config.rs#L54-L55)) is the
   argument for the kuten existing, and it was the only slot with no row. ~~`inquiry` leaves it
   unpopulated, as it leaves `object`, `rubric` and `question_pressure`.~~

   > **Erratum 3 — struck 2026-09-06, building A3 (#575).** That closing sentence describes
   > the **post-A2** state and was marked *Settled* while describing it, so as written it now
   > forbids what §5 and §6 require of A3: §6 makes `object.direction` and the `doctor` line
   > A3 acceptance criteria, and §5 puts the epistemic half of `question_pressure` in A3
   > standing alone. A3 populates both, and `inquiry` leaves `thresholds` and `rubric`
   > unpopulated. Nothing about the *count* is disturbed — it is eleven, and the reason
   > `thresholds` stays empty is unchanged and is the only part of that paragraph the
   > settlement was about.
   >
   > The wrapping is worth recording with it: this sentence could only be found with a
   > whitespace-collapsed search, because it breaks mid-phrase. A grep for `` `inquiry` leaves
   > it unpopulated `` matches nothing in this file.
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
