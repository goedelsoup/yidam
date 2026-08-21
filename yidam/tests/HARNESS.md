# Test Harness

How the yidam template is tested and how regressions are detected across model versions.

This document describes yidam's own test infrastructure. It is **not** prelude — derived
repositories do not inherit it and the bootstrap agent does not read it. It lives beside the
harness it describes.

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

Steps 3 and 6 are design, not implementation. There is no domain owner agent — the scenario
is inlined into the bootstrap's prompt — and no judge is invoked; the run's transcript is
discarded rather than captured, so the criteria that read it cannot be scored. Steps 1, 2, 4,
5 and 7 are what the harness does today.

## Regression detection

A regression is any of:
- A structural check that previously passed now fails
- A judge quality score that drops by more than one band from the prior snapshot
- A new orphan node, missing genesis commit, or missing edge that wasn't present before

Only the first is implemented, and comparison is refused across bootstrap protocol versions
(see [VERSIONING.md](../../VERSIONING.md), Layer 3) rather than reported as a change in the
model.

`tests/results/` does not exist and no snapshot has been committed, so there is nothing to
diff against yet. Until a baseline is committed, this section describes an intended
mechanism rather than a working one.

## Cargo workspace

The harness lives in `tests/harness/` with its own `Cargo.toml` workspace root, independent
from the `crates/` template layer that derived repos inherit. See [directory conventions](../prelude/guidelines/directories.md).
