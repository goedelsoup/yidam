# RFC-0038 — A rule is separable from the essay that justifies it

- **Status:** Draft
- **Track:** G8
- **Relates to:**
  - RFC-0036 (an yidam-level change — `.yidam/.vendor/` is read-only downstream, so the form fixed here is one every derivation inherits and none can adapt)
  - RFC-0037 (whose ratified `[open]` marker appears 13 times in the file this RFC splits, and which built no gate for a reason adjacent to why the ceiling here needs a floor)
  - RFC-0001 (the report contract, the origin of the interleaved rule-and-incident style this RFC keeps in `yidam/cli/tests/` and moves out of the prelude)
- **Versioning layers touched:** template only. `yidam/prelude/guidelines/agent-conduct.md` is rewritten rules-only and gains a sibling `agent-conduct.evidence.md`; one test file is added. **No contract bump, no CLI surface change, no version pin moves** — the gate is a test, not a command, and nothing in any corpus moves.
- **Downstream reference case:** the two recurring read routes — `AGENTS.md` here and `sadhana/root/AGENTS.md` in every derived repository — measured 2026-09-23 at 24,019 and 29,326 words, against four shipped example corpora that export to 8,243 tokens between them.

## Summary

The recurring read an agent is told to perform before substantive action is 24,019 words in this
repository and 29,326 in a derived one. Three files — `GRAPH.md`, `directories.md`,
`agent-conduct.md` — are 22,381 of it, and each interleaves a normative sentence with the
incident that produced it. This RFC fixes a form that separates the two without losing either:
rules in the file the read route names, evidence in a sibling `*.evidence.md` the route does not,
one section per rule, linked. It also fixes the gate, which is the harder half — a word-count
ceiling on the recurring read is easy to write and the cheapest way past it is to delete the
reasoning, which is the one outcome this work rules out.

## Problem

### What the routes cost

Measured 2026-09-23, on a route discovered rather than listed (see
[Route discovery](#route-discovery)):

| route | files | words |
|---|---|---|
| `AGENTS.md` → "Full context" (this repo) | 6 | 24,019 |
| `sadhana/root/AGENTS.md` → "Before taking substantive action" (every derived repo) | 9 | 29,326 |

These figures are the gate's, measured by the discovery rule below rather than from a
hand-written list, and they differ slightly from `#954`'s (24,022 and 28,136) — the derived route
grew between the two measurements, and the gate's reading is the one CI will hold.

`#933` sets the scale of the problem by comparing this to the corpus it governs, quoting "~1,500
tokens as `llms.txt`". That figure is **one example, the smallest of four.** Exported with
`yidam export --format llms`:

| example | nodes | tokens |
|---|---|---|
| `streamflow` | 8 | 1,539 |
| `incidents` | 13 | 1,991 |
| `property` | 12 | 2,136 |
| `journalism` | 12 | 2,577 |
| **all four** | **45** | **8,243** |

The asymmetry is real and the specific ratio `#933` draws from it is not: the instructions
outnumber the demonstrated corpus by roughly 4:1 across all four examples, not 40:1. Nothing in
this RFC rests on the ratio, and it is corrected here so it is not quoted forward.

### The weight is in six sections, not three files

`#954` frames this as three long files. Measuring per `##` section says something more useful —
six sections of forty-two carry 11,487 words, **more than half of the 21,850** those three files
hold in sections:

| section | words |
|---|---|
| `GRAPH.md` → The class contract | 3,856 |
| `agent-conduct.md` → Mark claim confidence | 2,157 |
| `GRAPH.md` → Commit vocabulary | 1,693 |
| `directories.md` → `.yidam/catalog/` | 1,612 |
| `directories.md` → `.yidam/capabilities.toml` and `.yidam/runs/` | 1,169 |
| `directories.md` → `.yidam.toml` (repository root) | 1,000 |

This matters for sequencing: the work is not a rewrite of three documents but a restructuring of
about six sections, and the rest of each file is already close to rules-only.

### The house style already marks the sections, but not reliably the sentences

The objection to a rules-first rewrite is that the interleaving is deliberate — it is the same
style the gates in `yidam/cli/tests/` are written in, where a test's docstring carries the defect
it was built against. That objection is right, and it is also the argument *for* this form rather
than against it, because **the style already distinguishes the two**: a bold lead-in sentence
followed by its justification.

The distribution is the evidence. In `agent-conduct.md`, every section under ~350 words has zero
bold lead-ins; the marker appears **only** in sections that grew an essay:

| section | words | bold lead-ins |
|---|---|---|
| Commit deliberately | 64 | 0 |
| Link generously | 31 | 0 |
| Stay within scope | 30 | 0 |
| Preserve provenance | 34 | 0 |
| Mark claim confidence | 2,157 | 8 |
| When claims arrive from another repository | 577 | 7 |
| When your corpus disagrees with the one it cites | 608 | 5 |

So the form below is **discovered from the house style, not imposed on it**. The rules-first file
is what these documents are already trying to be in the places where they got long enough to need
it.

What it is not is a gate-ready marker, and the measurement says so in both directions. Of the 27
line-initial bold spans in the pre-split file, **4 are not rules** — the label `**Rules:**` and
three ordinary emphases that happen to open a paragraph (`**deployed to obtain**`, `**minimum tag
across the whole supporting chain**`, `**prelude**`) — so a scanner keyed on line-initial boldness
runs at 85% precision. Recall is worse: of the 12 bold spans that sit mid-line, **3 are rules**,
including `**A `[verified]` claim in a node that links no catalog entry is reported**`. The
typography is a reliable signal about which *sections* grew essays and an unreliable one about
which *sentences* are normative. The form therefore uses an explicit link per rule, and nothing in
the gate below counts bold spans.

### Why the obvious gate is the whole risk

`#933` asks for the recurring read to land near 3,000 words *without losing the reasoning*. A
word-count ceiling satisfies the first clause and actively rewards violating the second: the
cheapest edit that passes a ceiling is deleting an essay. Any gate that holds this has to
distinguish **shorter** from **thinner**, and that needed designing before prose moved.

### One thing the measurement turned up

`directories.md` is 9,375 words across 20 sections answering "where does this live", and it does
not mention `.yidam/tonpa/` once — the directory an installed corpus dependency lands in.
Across the *entire* vendored prelude the path appears in three files, two of which are SDK parity
fixtures no authoring agent reads (`sdks/parity/mcp/README.md` and `tools.json`). The only
agent-readable occurrences are two lines in `agent-conduct.md`, both path components inside
examples of something else — the outbound-citation rule and a `cites:` block.

A rules section enumerating the layout would have made that visible, which is roughly the argument
`#933` is making. It is not a reason to split the file, but it is a thing to fix while splitting
it.

## Proposal

### The form

A guideline that has grown essays becomes two files:

- `<name>.md` — **rules only.** Every normative sentence, every normative example (the `cites:`
  block, the `edge_claims:` block, the four-shape grammar table), and nothing else. This is the
  file a read route names.
- `<name>.evidence.md` — **one `##` section per rule**, slugged, holding the incident that
  produced it, the measurement that set its threshold, and the failure it was built against.

Each rule that has evidence ends with `[why](<name>.evidence.md#<slug>)`. The link is explicit
rather than inferred, for the precision reason above.

```
agent-conduct.md                          agent-conduct.evidence.md
──────────────────────────────────        ──────────────────────────────────
**A `[verified]` claim in a node          ## verified-unsourced
that links no catalog entry is
reported**, as `verified-unsourced`.      Until this check existed nothing echoed
The fix is a citation or a                it back: `catalog/` recorded provenance
demotion […]                              and three checks verified the catalog's
[why](…#verified-unsourced)               own bookkeeping, while no check asked
                                          whether a claim rested on anything.

                                          Over-counting evidence is the flattering
                                          error […] a mature corpus measured eight
                                          miscounts and **every one of them
                                          promoted**.
```

### Why a separate file, and not a second section or a footnote

Two forms were tried against the measurement before this one. Both keep the evidence in the same
file — rules above a divider with a `## Why` section below, or rules with the incidents in
footnotes — and both fail for the same reason: **the recurring read is measured per file, and a
route names files.** Evidence in the same file shrinks the read only if the agent chooses to stop
reading, and nothing enforces that choice. The ceiling would be measuring a number no reader is
held to.

Putting the evidence in a file the route does not name makes the reduction real, and it composes
with a prohibition that already exists: bootstrap step 1 says to read six named files "and **only**
these six files" and forbids the rest of the prelude. An evidence file is therefore **already out
of scope during bootstrap** with no edit to the skill — the bootstrap read shrinks as a
consequence of the form rather than as a second change to make.

### The gate: a ceiling paired with a floor

`yidam/cli/tests/prelude_rules_and_evidence.rs`. Nine checks, every one naming the item that
failed rather than reporting a total:

| check | fails when |
|---|---|
| `a_pair_is_discovered_and_both_halves_exist` | an evidence file has no rules file, or the scan finds no pairs at all |
| `every_why_link_resolves_to_a_section` | a rule links `#slug` and the evidence file has no `## slug` |
| `every_evidence_section_is_reached_by_a_rule` | evidence exists that no rule points at |
| `every_evidence_section_carries_an_argument` | a section is under 25 words — a heading left behind after its prose was deleted |
| `the_scan_sees_the_known_routes` | route discovery stops finding the two routes known to exist |
| `every_route_has_a_ceiling` | a new recurring read appears with no cost recorded |
| `the_recurring_read_stays_under_its_ceiling` | a route's read exceeds its ratcheted ceiling |
| `every_pair_has_a_floor` | a pair is split with no floor recorded |
| `a_split_pair_does_not_shrink` | rules + evidence together fall below the pair's floor |

The discriminator is the last two rows against the two before them. **The read must shrink; the
rules and their evidence together may not.** Moving prose from one to the other passes; deleting it
fails.

#### The floor must be the post-split measurement, and this is where it went wrong first

The floor was initially set to the *pre-split* word count — 4,723 for `agent-conduct.md` — which
looks conservative and is the bug. The pair totals 5,399 after splitting, because headings, two
file headers, and the connective sentence each moved essay needs once it no longer sits under the
rule it explains are real added text. The pre-split figure therefore leaves **676 words of slack**,
and a mutation that trimmed every evidence section to its first two sentences deleted 531 words of
reasoning and stayed green. Slack in this floor is exactly the room a thinning edit needs.

The floor is the measured total, with none. A copy-edit that genuinely retires a word turns it red,
and that is the intended cost rather than friction to design around: making the removal of
reasoning a recorded edit to a constant is what "without losing the reasoning" can be held to.

#### Route discovery

A route is tracked markdown outside `docs/` that markdown-*links* all three of `IDENTITY.md`,
`GRAPH.md` and `agent-conduct.md`, and its cost is the route file plus the prelude files linked
from the one `##` section that names all three. Each clause was forced by a wrong answer:

- **Outside `docs/`.** Eleven files under `docs/` name `agent-conduct.md`, this RFC among them.
  A document that argues *about* the read is not a document that prescribes it.
- **Links, not mentions.** Relaxing to mentions adds exactly one file: `yidam/prelude/skills/bootstrap.md`,
  whose step 1 names five of these files in backticks. That is a genuine read route, and it is
  deliberately not one of these two — its cost is the **one-time bootstrap read**, not the
  recurring per-session one, and the same words would otherwise be counted under both. It shrinks
  as a consequence of this form anyway, because step 1's existing "only these six files" excludes
  the evidence file.
- **All three, not two.** `IDENTITY.md` + `GRAPH.md` alone matches seven files: the two routes plus
  `yidam/README.md`, `sadhana/root/README.md`, `yidam/prelude/README.md`, `yidam/prelude/kuten/README.md`
  and the `inquiry` kuten profile — orientation documents that link the model without prescribing
  the conduct read. Requiring `agent-conduct.md` is what separates a route from a pointer.
- **A section, not the file.** `AGENTS.md` links `yidam/prelude/skills/bootstrap.md` from its
  *first* section, addressed to an agent bootstrapping a fresh clone. Counting every link in the
  file put those 8,824 words inside a figure that claims to be the per-session read and inflated it
  by 36%.

### What it measures, after the worked example

`agent-conduct.md` is split. `GRAPH.md` and `directories.md` follow against the proven form.

| | before | after | change |
|---|---|---|---|
| `agent-conduct.md` (in the read) | 4,723 | 2,998 | −36.5% |
| `agent-conduct.md` + its evidence | 4,723 | 5,399 | +14.3% — nothing deleted |
| `AGENTS.md` route | 24,019 | 22,294 | −7.2% |
| `sadhana/root/AGENTS.md` route | 29,326 | 27,601 | −5.9% |

**The near-3,000-word target in `#933` is not reachable, and the reason is measurable.** The
normative content of `agent-conduct.md` alone — every rule sentence and normative example, with
every essay removed — is 2,998 words, in the *smallest* of the three files. At the same ratio
`GRAPH.md` lands near 5,260 and `directories.md` near 5,953, putting the route near **15,800**: a
34% reduction, not an 88% one. Reaching 3,000 would mean deleting rules, which is a different
decision from this one and should not arrive disguised as a formatting change.

The projection is a projection. `directories.md` may well cut further than `agent-conduct.md` did,
because its rules are short ("`crates/` holds X") where its essays are long; `agent-conduct.md` is
the densest of the three in normative content per word, so 36.5% is plausibly the *floor* of the
three ratios rather than the average. The honest claim available today is the measured row, and the
rest is held open below.

## What this does not touch

- **The gates' own docstrings.** The interleaved style in `yidam/cli/tests/` is not changed and
  should not be. A test docstring has one reader, who is already reading the file for a reason; a
  prelude guideline has every agent in every derived repository, reading it before it knows which
  rule it needs. The cost of interleaving scales with the second and not the first.
- **`reading-the-corpus.md`, `upstream.md`, `PHASES.md`, `CONSTITUTION.md`.** All under 1,700
  words, none with an essay problem. A pair for a document that has no essays would be overhead.
- **What the rules say.** Every normative sentence in `agent-conduct.md` survives the split; the
  only prose changes are removals of argument that landed in the evidence file, verified by
  comparing the sentence sets of the pre- and post-split text.
- **Bootstrap step 1's read list.** Unchanged, and the evidence file is excluded by the existing
  prohibition rather than by a new clause.

## Migration & compatibility

**Template layer.** Under RFC-0036 this is an yidam-level change: `.yidam/.vendor/` is read-only
downstream, so the form chosen here is the form every derivation inherits and cannot adapt.

A derived repository gets the split at its next `mise run yidam-vendor-update`. Nothing breaks: the
route in `sadhana/root/AGENTS.md` names `agent-conduct.md`, which still exists at the same path,
and the evidence file arrives beside it. A repository that pinned a prelude commit sees no change
until it moves the pin.

No CLI surface changes and no version pin moves. The gate is a test, not a command.

### A split is a line-sliding edit, and `docs/` cites these files by line

The one cost that showed up only on contact. Splitting a prelude file moves every line in it, and
`docs/rfcs/` cites prelude files by line number: `agent-conduct.md` had two inbound `#L`
citations, both naming **L42** for the canonical `[inference]` spelling, and the split landed L42
on a blank line. `dead-line-citation` caught both, which is the gate from #899 doing exactly what
it was built for — nothing here had to be discovered by hand.

Two things that generalize to `GRAPH.md` and `directories.md`:

- **Re-pointing the fragment is half a repair.** `0013-node-model-close.md` cited the line with a
  label that only restated the coordinate, so it sat in the `unverified-line-citation` residue —
  checked for existence and nothing else. Re-pointing it to L49 would have made it green and left
  it able to slide again silently. It was repaired by *quoting the passage* instead, which is what
  moved it out of that population and dropped the ratchet from 109 to 108.
- **The next step is twelve times this.** `GRAPH.md` carries 10 inbound line citations and
  `directories.md` 2, across seven RFCs. Whoever splits them should expect to repair all twelve
  and should quote each passage while doing it, for the reason above.

## Alternatives considered

- **A word-count ceiling alone.** Rejected: it is satisfied by deleting the reasoning, which is the
  one outcome ruled out. It is the reason the floor exists.
- **Rules above a divider, evidence below, in one file.** Rejected: the read is measured per file
  and a route names files, so the reduction would be notional. See
  [Why a separate file](#why-a-separate-file-and-not-a-second-section-or-a-footnote).
- **Evidence as markdown footnotes.** Rejected for the same reason, plus a smaller one: footnote
  syntax renders inconsistently across the three surfaces that display these files (the docs site,
  the editor webview, a plain agent read).
- **Inferring rules from bold lead-ins.** Rejected at ~90% precision — measured, not assumed. An
  explicit `[why]` link costs a few words per rule and is exact.
- **Per-section floors instead of one floor per pair.** A table of 21 constants, tighter but
  rotting on every edit. The per-pair floor with zero slack catches the same mutations, and
  `every_evidence_section_carries_an_argument` localizes which section was emptied.
- **Leaving the three files alone and shortening the route instead.** Dropping files from the read
  list cuts the number without changing what an agent must know, and moves the cost to the first
  time it needs a rule it was not given.

## Open questions

- **Does the form survive `GRAPH.md`?** `agent-conduct.md` is a list of behavioral rules and splits
  cleanly. "The class contract" at 3,856 words is a *specification*, where the rule and the
  reasoning are more entangled — a paragraph explaining why a field exists is often also the
  statement of what it does. If the form does not hold there, the finding is about the form and
  should come back here rather than being worked around in the file.
- **Is 36.5% the floor or the average of the three ratios?** Held open deliberately; the answer is
  two more measurements, not an argument.
- **Does the evidence file get read when it should be?** The failure mode this form introduces is a
  rule applied without the hard case its evidence settles. Nothing here detects that, and the only
  instrument that would is the same one `#726` used for the retrieval surface: counting what agents
  actually open across derived-repository sessions. Worth re-running a quarter after this ships,
  with the prediction stated now — **if the evidence files are opened zero times in the next
  measurement window, this form failed and the reasoning should come back inline.**
- **`.yidam/tonpa/` in `directories.md`.** To be fixed when that file is split, not before; noted
  here so it is not lost with the measurement that found it.
