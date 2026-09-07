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
| `concept/low-flow.yml` | `missing-property` — `concept` declares `datum` as `required: true` and this instance omits it | **error, from a `warn` check** |
| `concept/mixing-zone.yml`, `concept/tailwater.yml` | `missing-property` — no `claim_tag` | warn |
| everything else | nothing — the control | — |

The errors are the ones that matter: they make `gate.passed` false, so the golden pins the
failing verdict as well as the shape. Two of the three are in the baseline — see below — so
the verdict it pins is a mixed one rather than a corpus with no history.

**The `missing-property` block is deliberately mixed, and that is the point of it.** The
check is declared `warn`; one of its three findings is `error`, because `Check::severity_of`
raises an omitted `required: true` property past the level its check is declared at. A
consumer reading `check.severity` where it must read `violation.severity` renders that
finding as advisory while it fails CI — which is what all four of them did, undetected,
until #655. Every violation in this fixture carried its check's level, so the wrong field
gave the right answer everywhere.

The escalated finding sits on `concept/low-flow.yml` on purpose: that node already carries
the `dangling-edge`, so the fixture gains a gating *finding* without gaining a gating
*node*, and the control nodes stay controls.

The two `claim_tag` findings are not fixture drift. Two of the three concepts deliberately
carry no evidence tag — one is open by its label instead, one is the control that trips
nothing — and a node making no tagged claim is a real state rather than a defect. The
declaration says nothing about requiring it, so the omission is reported and forgiven,
which is the arm `datum` no longer covers. Its sibling checks gate on the ontology being
contradicted — `unlicensed-edge` among them only where a class declared
`edge_policy: exhaustive`, since a non-empty `edges:` on its own never claimed to be the
complete vocabulary.

## What it is built to reach

Every property below was added because something downstream could not be exercised without
it, and each is asserted by a golden or by a test in the extension rather than merely being
present.

| Property | What it makes reachable |
|---|---|
| **Two classes** | Grouping in the corpus tree, above the arity at which any grouping implementation looks correct. |
| **One `required: true` property, omitted once and carried twice** | A violation whose severity is not its check's. `missing-property` is `warn` and this omission is `error`, so a client reading the check-level field renders a finding that fails CI as advisory — the defect #655 fixes at four call sites, none of which any golden or extension test could see while every violation in the corpus carried its check's level. Carried by two instances so the satisfied arm is pinned too, and so the report names one node rather than every concept. |
| **Both open-question arms** | `concept/low-flow.yml` is open through a declared `claim` property; `concept/mixing-zone.yml` is open through a `?` label. A corpus using one arm alone cannot tell an implementation reading both from one reading either — the defect the MCP cases were split to expose. |
| **A claim tag of each kind** | `[verified]`, `[inference]`, and a structural `open`, so `status`'s three counters are each non-zero. |
| **A mention that is not a use** | `concept/tailwater.yml` names `[open]` and `[verified]` in backticks. The counters and the open-question predicate must both ignore them. A corpus that never discusses its own vocabulary cannot tell a scanner reading claims from one reading bytes — and the byte reader published a verified claim against a true zero, inside a `REGEN` block, for four commits. |
| **A named source that is not cited** | `gauge/riffle-station.yml` writes the slug of the `obtained: false` catalog entry in prose and links nothing. `catalog-unobtained-but-cited` is Error severity and gates, so a checker matching the bare slug fails a build on a node that cites nothing — which is what it did in a derived repository, where the slug collided with a connector crate named after the source it fetches. The same node is what makes `verified-unsourced` reachable: it asserts `[verified]` while resting on nothing, which is the shape that check exists for, and naming a source in prose must not discharge it. It is deliberately **not** made to cite `stage-discharge.md` — a citation there would discharge the check and take the fixture's only instance of it with them. |
| **A source two nodes actually cite** | `catalog/stage-discharge.md` is the arm `gauge-record.md` cannot reach: it is obtained, and `concept/low-flow.yml` and `concept/tailwater.yml` both link to it in prose. Without it every `cited_by` in `catalog-audit` is empty, and a corpus view placing sources under the node that cites them has nothing to place. |
| **A `used-by` list wrong in both directions** | The same entry claims `mixing-zone.yml`, which cites nothing, and omits `tailwater.yml`, which cites it. One arm alone cannot tell a `drift` implementation reading both from one reading either. It is also what makes `rename`'s `unhandled` list non-empty in a golden: a hand-written `used-by` entry is exactly the reference a rename cannot safely rewrite, and until this entry existed that arm was reachable only from a unit test. |
| **An inbound edge two hops out** | The gauge authors `measured-by`, so the neighborhood panel has a direction to group by other than `out`. |
| **Two phase branches** | `phases` has rows. `ma/gauge-reader` is deliberately absent though the elector is registered, so `branch_present: false` is a golden rather than only a unit test. |
| **Three commits, one operational** | `diff HEAD~1..HEAD` has a range and a modified node, and the log goldens show the classifier splitting rather than a column of `[E]`. |
| **A baseline with one expired entry and one that still forgives** | `in_baseline: true`, and `expired_baseline_entries` non-empty. Both were `[]` in every golden, so the four counts the gate reports could not be told apart: a consumer that dropped the expired list entirely rendered a failing gate as `0 new · N inherited`, with nothing anywhere saying what was wrong (#657). See [The baseline](#the-baseline). |

## The baseline

`.yidam/lint-baseline.yml` is written by the last commit in `stage.toml`, not shipped in
`repo/`. It has to be, because `since:` is a commit sha and a sha is a function of the tree
its commit holds — a baseline inside the genesis tree would have to name itself. The recipe
writes `{{commit:1}}` and whichever runner staged the repository resolves it; naming a commit
that does not exist is an error rather than an empty string, because a `since` resolving to
nothing reports as *not expired* and would delete the arm below in silence.

| Entry | `since` | Reads as |
|---|---|---|
| `dangling-edge` on `concept/low-flow.yml` | the genesis commit | **expired** — it stood for 3 corpus-touching commits and `expire_after` is 2 |
| `resolution-elector-unregistered` on `sangha/resolutions/silt-budget.md` | none | inherited debt that still forgives — a hand-written entry has no clock and never expires |

The pair is the point. Both are `baselined_violations`, only one is an
`expired_baseline_entries`, so those two numbers are different and neither can be rendered
from the other. Until this existed the CLI's fifth gate field was `[]` in every golden and in
the fixture the extension is exercised on, which is how the extension's report type came to
omit it entirely: an expired entry reached the Health view as a red row with no children and
no stated cause, and the schema's `items` declaration for it was read by nothing (#657).

**Do not "fix" this by blessing it.** `--bless` carries `since` forward rather than
restamping it, deliberately — otherwise the command the clock constrains would be the command
that clears it. Blessing here rewrites the file, prints a reassuring line, and leaves the gate
exactly as red. That is the property `report-run.test.ts` asserts and the reason the Health
row does not offer a one-click Bless while an entry is out of time.

`expire_after: 2` against a three-commit history is small because the history is. The
arithmetic is in commits rather than days for the same reason the TTL section below gives:
a golden that moves with the clock is a golden nobody can compare.

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

Both also resolve `{{commit:N}}` in a written file to the sha of the recipe's Nth commit,
1-indexed. One file needs it — the baseline above — and the two implementations must agree:
the expired entry is what says so, because it is absent from a report the moment they do not.

## Why the JSON goldens are redacted

Three fields in the envelope are properties of the *run* rather than the corpus: the
absolute `root`, the binary's `version`, and the commit it was built from. They are
replaced with `<ROOT>`, `<VERSION>` and `<COMMIT>` before comparison. Everything else is
compared literally, including key order — a contract whose field order drifts is one that
breaks a consumer parsing it strictly.
