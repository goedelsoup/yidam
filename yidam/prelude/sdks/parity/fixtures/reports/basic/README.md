# reports/basic

The first fixture in RFC-0001's `reports/` family, and the golden that pins RFC-0016's
Phase 0 contract.

`repo/` is a small derived repository: four corpus nodes across two classes, two catalog
entries, and a sangha. `stage.toml` says how it becomes a git repository. `expected/` holds
the exact output of each report in each format.

It is deliberately **not** a corpus that trips every check. A fixture where everything
fails cannot show that a passing check passes, and one carrying sixteen findings produces a
golden nobody reads.

| Node | Trips | Severity |
|---|---|---|
| `concept/low-flow.yml` | `dangling-edge` — an edge to a file that is not there | error |
| `catalog/stage-discharge.md` | `catalog-used-by-drift` — its `used-by` list is wrong in both directions | warn |
| `concept/tailwater.yml` | `orphan-in` — nothing points at it | info |
| `gauge/riffle-station.yml` | `orphan-in` — the ontology declares no edge into a gauge | info |
| `gauge/riffle-station.yml` | `verified-unsourced` — it asserts `[verified]` and links no catalog entry | warn |
| every `concept` | `missing-property` — `concept` declares `datum` and no instance carries it | warn |
| `concept/mixing-zone.yml`, `concept/tailwater.yml` | `missing-property` — and no `claim_tag` either | warn |
| everything else | nothing — the control | — |

The error is the one that matters: it makes `gate.passed` false with an empty baseline, so
the golden pins the failing verdict as well as the shape.

The `missing-property` findings are the reason that check does not gate, and they are not
fixture drift. Two of the three concepts deliberately carry no `claim_tag` — one is open by
its label instead, one is the control that trips nothing — and a node making no tagged
claim is a real state rather than a defect. The property declaration has no `required`
field to tell *every instance has this* from *an instance may have this*, so gating on
omission would fail this corpus for being exactly what it was written to be. Its sibling
checks gate on the ontology being contradicted — `unlicensed-edge` among them only where a
class declared `edge_policy: exhaustive`, since a non-empty `edges:` on its own never
claimed to be the complete vocabulary.

## What it is built to reach

Every property below was added because something downstream could not be exercised without
it, and each is asserted by a golden or by a test in the extension rather than merely being
present.

| Property | What it makes reachable |
|---|---|
| **Two classes** | Grouping in the corpus tree, above the arity at which any grouping implementation looks correct. |
| **Both open-question arms** | `concept/low-flow.yml` is open through a declared `claim` property; `concept/mixing-zone.yml` is open through a `?` label. A corpus using one arm alone cannot tell an implementation reading both from one reading either — the defect the MCP cases were split to expose. |
| **A claim tag of each kind** | `[verified]`, `[inference]`, and a structural `open`, so `status`'s three counters are each non-zero. |
| **A mention that is not a use** | `concept/tailwater.yml` names `[open]` and `[verified]` in backticks. The counters and the open-question predicate must both ignore them. A corpus that never discusses its own vocabulary cannot tell a scanner reading claims from one reading bytes — and the byte reader published a verified claim against a true zero, inside a `REGEN` block, for four commits. |
| **A named source that is not cited** | `gauge/riffle-station.yml` writes the slug of the `obtained: false` catalog entry in prose and links nothing. `catalog-unobtained-but-cited` is Error severity and gates, so a checker matching the bare slug fails a build on a node that cites nothing — which is what it did in a derived repository, where the slug collided with a connector crate named after the source it fetches. The same node is what makes `verified-unsourced` reachable: it asserts `[verified]` while resting on nothing, which is the shape that check exists for, and naming a source in prose must not discharge it. It is deliberately **not** made to cite `stage-discharge.md` — a citation there would discharge the check and take the fixture's only instance of it with them. |
| **A source two nodes actually cite** | `catalog/stage-discharge.md` is the arm `gauge-record.md` cannot reach: it is obtained, and `concept/low-flow.yml` and `concept/tailwater.yml` both link to it in prose. Without it every `cited_by` in `catalog-audit` is empty, and a corpus view placing sources under the node that cites them has nothing to place. |
| **A `used-by` list wrong in both directions** | The same entry claims `mixing-zone.yml`, which cites nothing, and omits `tailwater.yml`, which cites it. One arm alone cannot tell a `drift` implementation reading both from one reading either. It is also what makes `rename`'s `unhandled` list non-empty in a golden: a hand-written `used-by` entry is exactly the reference a rename cannot safely rewrite, and until this entry existed that arm was reachable only from a unit test. |
| **An inbound edge two hops out** | The gauge authors `measured-by`, so the neighborhood panel has a direction to group by other than `out`. |
| **Two phase branches** | `phases` has rows. `ma/gauge-reader` is deliberately absent though the elector is registered, so `branch_present: false` is a golden rather than only a unit test. |
| **Three commits, one operational** | `diff HEAD~1..HEAD` has a range and a modified node, and the log goldens show the classifier splitting rather than a column of `[E]`. |

## Why this fixture declares no TTL

`catalog-expired` appears in the goldens with no violations, and that is deliberate. A TTL is
the one clock in yidam measured in **days**, so a fixture that declared one would produce a
golden that changed every morning — green on the day it was written and failing by the end of
the week, for no reason anyone could act on.

So the golden pins the *passing* state, which is what a corpus that has not opted in looks
like, and the expiry arithmetic is held by unit tests that pass their own `today` in. Do not
add `ttl_days` here.

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
