# Test Harness

How the yidam template is tested and how regressions are detected across model versions.

This document describes yidam's own test infrastructure. It is **not** prelude — derived
repositories do not inherit it and the bootstrap agent does not read it. It lives beside the
harness it describes.

Everything in `yidam/tests/` is held out from the worktree the bootstrap agent runs in, and
`the_scoring_layer_does_not_reach_the_worktree` asserts it over the real tree. That sentence
above was true only as an intention until protocol 0.2.0: `prepare_worktree` copied the whole
template, so the agent under evaluation ran in a directory holding the rubric, the judge's
criteria, and its scenario's `good_bootstrap_looks_like` — the reference description of a
good result for the domain it was about to be asked about. `judge.md` was worse than
available: it sat in `yidam/prelude/skills/`, the one directory a derived repo vendors, and
step 1 of the bootstrap skill told the agent to read it "so the genesis commit passes".

The quality bar the agent legitimately needs is in the skill's own steps and in the conduct
guidelines. The bands, the criteria and the reference descriptions are the instrument, and an
instrument the subject can read measures the reading.

## Design

Three agents participate in every test run:

| Agent | Role | Model |
|---|---|---|
| **Bootstrap** | The thing under test — runs the bootstrap skill against the scenario | Varies (the matrix dimension) |
| **Domain owner** | Simulates a human answering the bootstrap's ontology questions | Fixed (Haiku — cheap, credible) |
| **Judge** | Reads the resulting repo state and scores it against the rubric | Fixed (Opus — stable scorer) |

The domain owner is seeded with a [scenario](scenarios/) — sparse structured input
describing the domain well enough to answer questions believably, but not so fully specified
that it bypasses the bootstrap's interrogation loop. The [harness-side schema](harness/SCENARIO.md)
specifies what `scenario::load()` parses and how each field drives agent behavior.

## Execution

1. The Rust harness CLI creates an isolated git worktree from the yidam tree
2. The bootstrap agent is invoked in that worktree with `BOOTSTRAP.md` as its entry prompt
3. The bootstrap's interrogation turns are routed to the domain owner agent
4. When the bootstrap commits, the harness captures the resulting repo state
5. Structural checks run against the state (automated, in Rust)
6. The judge agent scores quality against the [rubric](rubric.md)
7. A result snapshot is written to `tests/results/<scenario>/<model>/<date>/`

Step 3 is design, not implementation. There is no domain owner agent — the scenario is
inlined into the bootstrap's prompt — so the bootstrap has nobody to interrogate.

Everything else runs. The transcript is captured to `transcript.jsonl`; the run record beside
the result carries the resolved model, turns, duration, cost, and any permission denials; and
step 6 scores the result against the Q criteria, writing a band and its evidence per criterion
to `quality.json`. Scoring is opt-in — it is a second model call — and `harness judge`
re-scores a captured result without paying for the bootstrap again.

**Q1 is the exception, and it follows from step 3.** An agent told that no domain owner is
present has nobody to ask, so the judge is shown that fact and told not to read the absence of
clarifying turns as a property of the model. Q1 becomes a real measurement when a responder
exists.

## Regression detection

A regression is any of:
- A structural check that previously passed now fails
- A judge quality score that drops by more than one band from the prior snapshot
- A new orphan node, missing genesis commit, or missing edge that wasn't present before

The first two are implemented — a band that drops between two scored snapshots is reported
with the rationale the judge gave for it. Comparison is refused across bootstrap protocol
versions
(see [VERSIONING.md](../../VERSIONING.md), Layer 3) rather than reported as a change in the
model.

## The committed baseline

[`tests/results/`](results/) holds captured runs. Each is a real bootstrap — the corpus the
agent produced, the history it wrote, the verdicts the checks returned, and the judge's bands
with the evidence behind them.

A baseline is a golden fixture, and the fixture is the corpus. It never changes again, so a
recomputed verdict that differs from the recorded one means the checks moved, not the model.
`no_baseline_has_drifted` asserts that on every run of the harness tests, and
`harness check --verify` reports the same comparison for a person. Neither needs a model or an
API key: the expensive half of an eval is producing the run, and a committed baseline has
already paid for it.

The gate asks whether the verdicts still match, not whether they all pass. A baseline records
what a run produced, failures included — and a check that starts *passing* is drift too. What
is not committed is the raw event stream: everything read from it is parsed once into
`structural.json`, and the stream is a couple of megabytes of mostly this repository's own
prelude echoed back through tool results.

Re-recording is a judgement, not a refresh. A baseline updated without a stated reason is a
baseline that follows the code rather than holding it to anything.

## Cargo workspace

The harness lives in `tests/harness/` with its own `Cargo.toml` workspace root, independent
from the `crates/` template layer that derived repos inherit. See [directory conventions](../prelude/guidelines/directories.md).
