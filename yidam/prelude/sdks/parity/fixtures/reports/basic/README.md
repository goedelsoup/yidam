# reports/basic

The first fixture in RFC-0001's `reports/` family, and the golden that pins RFC-0016's
Phase 0 contract.

`repo/` is a small derived repository: four corpus nodes across two classes, one catalog
entry, and a sangha. `stage.toml` says how it becomes a git repository. `expected/` holds
the exact output of each report in each format.

It is deliberately **not** a corpus that trips every check. A fixture where everything
fails cannot show that a passing check passes, and one carrying sixteen findings produces a
golden nobody reads.

| Node | Trips | Severity |
|---|---|---|
| `concept/low-flow.yml` | `dangling-edge` — an edge to a file that is not there | error |
| `concept/tailwater.yml` | `orphan-in` — nothing points at it | info |
| `gauge/riffle-station.yml` | `orphan-in` — the ontology declares no edge into a gauge | info |
| everything else | nothing — the control | — |

The error is the one that matters: it makes `gate.passed` false with an empty baseline, so
the golden pins the failing verdict as well as the shape.

## What it is built to reach

Every property below was added because something downstream could not be exercised without
it, and each is asserted by a golden or by a test in the extension rather than merely being
present.

| Property | What it makes reachable |
|---|---|
| **Two classes** | Grouping in the corpus tree, above the arity at which any grouping implementation looks correct. |
| **Both open-question arms** | `concept/low-flow.yml` is open through a declared `claim` property; `concept/mixing-zone.yml` is open through a `?` label. A corpus using one arm alone cannot tell an implementation reading both from one reading either — the defect the MCP cases were split to expose. |
| **A claim tag of each kind** | `[verified]`, `[inference]`, and a structural `open`, so `status`'s three counters are each non-zero. |
| **An inbound edge two hops out** | The gauge authors `measured-by`, so the neighborhood panel has a direction to group by other than `out`. |
| **Two phase branches** | `phases` has rows. `ma/gauge-reader` is deliberately absent though the elector is registered, so `branch_present: false` is a golden rather than only a unit test. |
| **Three commits, one operational** | `diff HEAD~1..HEAD` has a range and a modified node, and the log goldens show the classifier splitting rather than a column of `[E]`. |

## `stage.toml`

The reports cannot run against a bare directory, so every harness builds a repository out of
`repo/` first. Seven did, in seven copies, and they disagreed — the goldens staged three
commits and two branches while five of the extension's test files staged one commit and no
branch, so `expected/` described a repository the extension was never exercised on. The
recipe now has one copy and both runners read it: `apply_recipe` in `report_goldens.rs`, and
`test/stage.ts` in the extension.

## Why the JSON goldens are redacted

Three fields in the envelope are properties of the *run* rather than the corpus: the
absolute `root`, the binary's `version`, and the commit it was built from. They are
replaced with `<ROOT>`, `<VERSION>` and `<COMMIT>` before comparison. Everything else is
compared literally, including key order — a contract whose field order drifts is one that
breaks a consumer parsing it strictly.
