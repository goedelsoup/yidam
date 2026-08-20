# reports/basic

The first fixture in RFC-0001's `reports/` family, and the golden that pins RFC-0016's
Phase 0 contract.

`repo/` is a minimal derived repository: two corpus nodes, one class definition, and one
catalog entry. `expected/` holds the exact output of each report in each format.

It is deliberately **not** a corpus that trips every check — it trips three, one of each
severity, which is what makes it useful as a golden. A fixture where everything fails
cannot show that a passing check passes, and one carrying sixteen findings produces a
golden nobody reads.

| Node | Trips | Severity |
|---|---|---|
| `concept/tailwater.yml` | `orphan-in` — nothing points at it | info |
| `concept/low-flow.yml` | `dangling-edge` — an edge to a file that is not there | error |
| everything else | nothing — the control | — |

The error is the one that matters: it makes `gate.passed` false with an empty baseline, so
the golden pins the failing verdict as well as the shape.

## Why the JSON goldens are redacted

Three fields in the envelope are properties of the *run* rather than the corpus: the
absolute `root`, the binary's `version`, and the commit it was built from. They are
replaced with `<ROOT>`, `<VERSION>` and `<COMMIT>` before comparison. Everything else is
compared literally, including key order — a contract whose field order drifts is one that
breaks a consumer parsing it strictly.
