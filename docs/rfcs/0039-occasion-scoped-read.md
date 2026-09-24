# RFC-0039 — A read is scoped to the occasion, not to the repository

- **Status:** Draft
- **Track:** G9
- **Relates to:**
  - RFC-0038 (the rules/evidence split, which took 6,687 words off the recurring read and 7,819 off the bootstrap path and stopped where the remaining weight is reference rather than essay — this RFC is the move its ceiling docstring said would be needed next)
  - RFC-0028 (the kuten layer, whose phase types are one of the two vocabularies an occasion can be named in)
  - RFC-0036 (an yidam-level change — the routes this RFC rewrites are vendored read-only into every derivation)
- **Versioning layers touched:** template only. `AGENTS.md`, `sadhana/root/AGENTS.md` and bootstrap step 1 are rewritten to name sections rather than files; `yidam/cli/tests/prelude_rules_and_evidence.rs` gains per-occasion ceilings and anchor resolution. **No contract bump, no CLI surface change.**
- **Downstream reference case:** the two recurring read routes and the bootstrap path, measured 2026-09-24 at 17,686, 22,993 and 26,323 words.

## Summary

#933 asked for the read an agent performs before substantive action to land near 3,000 words.
RFC-0038 answered with a form that moves essays out of the read and holds the reasoning with a
floor; applied to four files it took the recurring read from 24,019 to 17,686 and the bootstrap
path from 34,142 to 26,323. It cannot go further, because what is left is reference: the class
contract, the directory conventions, the commit vocabulary. None of it is an essay and all of it
is needed — *by someone, on some occasion*. The claim of this RFC is that the number #933 named
is unreachable for **the repository** and reachable for **an occasion**: an agent about to write
a node needs about 4,000 of the 17,686 words, an agent about to run a phase a different 4,000,
and a bootstrap about 19,000 of its 26,323. The route should say which, by naming sections rather
than files, and the gate should hold each occasion to its own ceiling.

## Problem

### What the split bought, and where it stopped

Every figure here is measured at `dc95e71` (#962, the skill split) with `wc -w`, the same count
the gate uses.

| path | before RFC-0038 | after (four files split) | cut |
|---|---:|---:|---:|
| `AGENTS.md` route (this repository) | 24,019 | 17,686 | −26% |
| `sadhana/root/AGENTS.md` route (a derivation) | 29,326 | 22,993 | −22% |
| bootstrap path (ten files, clone to first node) | 34,142 | 26,323 | −23% |

The split has a natural stopping point and it has been reached. The ratchet docstring on
`READ_CEILING` says why: `directories.md` is 7,295 words after its essays moved, and what remains
is what belongs in each of twenty directories, the catalog frontmatter shape, the capability
manifest shape, the authorship table. That is rule, not argument. A fifth split retires nothing.

### Where the remaining weight is

The bootstrap path, per file:

| file | words | share |
|---|---:|---:|
| `yidam/prelude/skills/bootstrap.md` | 7,358 | 28% |
| `yidam/prelude/guidelines/directories.md` | 7,295 | 28% |
| `yidam/prelude/GRAPH.md` | 5,401 | 21% |
| `yidam/prelude/guidelines/agent-conduct.md` | 2,998 | 11% |
| `yidam/prelude/PHASES.md` | 1,437 | 5% |
| `yidam/prelude/CONSTITUTION.md` | 982 | 4% |
| `yidam/prelude/GLOSSARY.md` | 334 | 1% |
| `BOOTSTRAP.md` | 210 | 1% |
| `yidam/prelude/IDENTITY.md` | 195 | 1% |
| `.claude/CLAUDE.md` | 113 | 0% |
| **total** | **26,323** | |

Two files are each larger than the whole target on their own, and the skill — the one document
on the path that is executed rather than consulted — is the largest.

### A census of what the bootstrap never touches

The skill names what it needs. Grepping it for each prelude concept and matching the hits to the
`##` sections that define them gives a census of sections **no step of the bootstrap refers to
at all**:

| file | sections the skill never names | words |
|---|---|---:|
| `directories.md` | `.yidam/catalog/`, `.yidam/tonpa.toml`, `.yidam/private-paths`, `.yidam/policy/`, `.yidam/bin/`, `.yidam/capabilities.toml`, `.yidam/authorship.yml` | 3,764 |
| `PHASES.md` | the whole file — a bootstrap runs no phase; the one sentence it needs ("phases run on `phase/<name>` branches") is in the skill | 1,437 |
| `agent-conduct.md` | the five cross-corpus sections — a claim leaving, arriving, resting beside, disagreeing; the base-rate rule | 1,345 |
| `GRAPH.md` | Residence time, The baseline and its own clock, Branches as inquiry | 840 |
| | | **7,386** |

That is **28% of the path**, read by every bootstrap, about a corpus that does not yet exist:
residence time on nodes that have not been written, the authorship table of a repository with
one author, the protocol for a claim arriving from another corpus when there is no corpus.
(`catalog` is named four times, all in step 9's *next steps* — the first catalog entry is the
first thing a user does *after* bootstrap, which is the point.)

The census has a known limit and it should be stated before it is used. It measures what the
skill *names*, not what a good bootstrap *needs*. `agent-conduct.md`'s "Mark claim confidence"
is 1,203 words the skill never mentions either — no step says to tag a seeded node `[open]` —
and that is at least as likely to be a gap in the skill as surplus in the read. The census can
retire the 7,386 above with confidence because each of those sections is about a state a
bootstrap cannot be in; it cannot by itself say the rest is all needed.

### The recurring read has the same shape

Post-genesis, the `AGENTS.md` route reads six files in full before *any* substantive action.
But the actions are not one occasion. An agent about to author a node needs the class contract
and the `.yidam/corpus/` conventions and the rule that an edge is a claim; it does not need the
catalog frontmatter, the four retrieval commands, or the resolution protocol. An agent about to
run a phase needs `PHASES.md` and the commit vocabulary; it does not need the class contract. An
agent about to cite another corpus needs exactly the five conduct sections a bootstrap never
does. The route hands all of them everything, and the ceiling — which is the right gate for a
*route* — can only ever ratchet the union.

### The floor is a single section

There is a second reason "near 3,000" is unreachable as a route figure, and it survives any
scoping. GRAPH.md's "The class contract" is 2,465 words as one `##` section; the skill is 7,358.
An occasion that needs the whole class contract — writing the first class, changing one — is
over 80% of the target before the glossary is opened. The number #933 named was a *route* figure
inferred from four files; no occasion that includes the contract lands on it, and the bootstrap
occasion is more than twice it before the prelude is opened.

## Proposal

### An occasion is named by what you are about to write

The two vocabularies the template already freezes both name occasions. GRAPH.md's commit
vocabulary says what act a commit records — `establish`, `revise`, `phase`, `decide`,
`synthesize`, `extract`, `compute` — and PHASES.md's phase types say what a bounded unit of
inquiry is. An agent that knows which verb it will write knows its occasion, and the verb is
something it must already know: the commit message requires it. So the route asks that, first,
and reads what that verb needs.

The occasions, and the read each one is measured to need, are below. **Core** is read on every
occasion: `GLOSSARY.md` (334), `IDENTITY.md` (195), GRAPH.md's preamble, *Encoding*, *Nodes* and
*Edges* (668), and agent-conduct's preamble and five short rules (286) — **1,483 words**.

| occasion | verbs | read beyond core | words | total | vs. 17,686 |
|---|---|---|---:|---:|---:|
| **Write or revise a node** | `establish` `revise` `withdraw` | GRAPH *The class contract*: lead, *What an edge rests on*, *Which keys hold prose*, *Prose in a property*, *Properties every class may carry* (1,165); directories *`.yidam/corpus/`* (509); conduct *Mark claim confidence*: lead, *An edge is a claim*, *A tag may be a field* (915) | 2,589 | **4,072** | −77% |
| **Run a phase** | `phase` `assess` `scope` `synthesize` `open` `close` | `PHASES.md` (1,437); GRAPH *Commits as events*, *Commit vocabulary*, *Branches as inquiry* (1,259) | 2,696 | **4,179** | −76% |
| **A claim crosses a corpus boundary** | `cites:` written or received; `revise` on a cited node | conduct *Prefer a base rate*, *When claims leave*, *arrive*, *rest beside*, *disagree* (1,345); directories *`.yidam/catalog/`* (1,104) | 2,449 | **3,932** | −78% |
| **Change a class** | `decide` on the ontology | GRAPH *The class contract* whole (2,465); directories *`.yidam/decisions/`* (123) | 2,588 | **4,071** | −77% |
| **Retrieve** | before any of the above, when the corpus is not small | `reading-the-corpus.md` (1,141) | 1,141 | **2,624** | −85% |
| **Bootstrap** | `genesis` `overlay` | the path minus the census above | | **18,937** | −28% |

Every post-genesis occasion lands between 3,900 and 4,200: not 3,000, and 77% below the route
it replaces. The bootstrap lands at −28%, and the section below says why that is its floor.

The table is the proposal's *measurement*, not its text. What ships is the route: `AGENTS.md`
gains one short heading per occasion, each a list of section links —

```markdown
## Before you write or revise a node

Read, in this order: [the class contract](yidam/prelude/GRAPH.md#the-class-contract) through
*Properties every class may carry*; [`.yidam/corpus/`](yidam/prelude/guidelines/directories.md#yidamcorpus);
[Mark claim confidence](yidam/prelude/guidelines/agent-conduct.md#mark-claim-confidence).
```

— and the "Full context" list that reads everything becomes the *reference* index it already is
in practice: the place an agent goes when its occasion is not one of the named ones.

### The bootstrap occasion

The skill is not scoped by this RFC, and the reason is structural rather than cautious. The
skill is 7,358 words and sequential; an agent at step 6 has no use for step 8's vendor commands,
so the same scoping applies in principle — one file per step, each naming the prelude sections
that step reads. But the skill is the most-consumed document in the template: seven test files
quote its fences, its numbered read list, its `rm` lines and its commit-sequence block, and the
harness reads its verbs to decide whether a history is a bootstrap's. Splitting it by step is a
consumer migration with a separate RFC's worth of contract, and it is the *second* move here,
not the first.

The first move is the census. Step 1 currently reads seven files; it should read the seven
files **less the 7,386 words above**, stated as section exclusions on the same list:

```markdown
5. `yidam/prelude/PHASES.md` — read *Phase types* only; a bootstrap runs no phase
7. `yidam/prelude/guidelines/directories.md` — skip `.yidam/catalog/` through `.yidam/authorship.yml`;
   those describe a corpus that exists, and step 9 names the first of them as the first thing to do after this
```

The ceiling on the path falls to the measured post-census figure, no slack, on the same
discipline as today.

### The gate

Three changes to `prelude_rules_and_evidence.rs`, each measured per item and never per total:

1. **A route is a set of occasions.** `READ_CEILING` becomes `(route, occasion, ceiling)`,
   measured by resolving each section link in the occasion's heading and counting the section's
   words, plus core. The no-slack discipline is unchanged; a raise names the section and the
   delta.
2. **Anchors resolve.** A section link whose fragment does not name a `##` or `###` heading in
   the target file is a failing link, not a warning — `every_why_link_resolves_to_a_section`
   already does this for evidence files and is generalized to any prelude link with a fragment.
   The docs site checks pages and not fragments, so this is the only place a dead anchor in a
   route would be caught.
3. **The converse.** Every `##` section in the seven prelude files is on at least one occasion's
   read, or is listed by name in a `REFERENCE_ONLY` set with a one-line reason. A section on no
   occasion's read and not in the set is a section nobody is ever told to read, which is the
   check that finds the *next* `.yidam/authorship.yml` — a section that exists, is vendored into
   every derivation, and is read by no one on any occasion.

### What "near 3,000" becomes

The target changes kind. It was a route figure and it becomes a per-occasion figure with a
measured floor: the smallest occasion is *Retrieve* at 2,624, the largest post-genesis occasion
is *Run a phase* at 4,179, and the bootstrap is 18,937. #933 is closed on those numbers, not
on 3,000, and the ceilings hold each of them separately.

## What this does not touch

- **The files.** No prelude file is split, moved or renamed by this RFC. Sections are addressed
  in place by anchor. The rules/evidence pairs and their floors are unchanged.
- **The evidence files.** They are reached by `[why]` links on every occasion and on none of the
  reads; RFC-0038's exclusion stands.
- **The skill's structure.** Step 1's list stays a list of seven files, with exclusions stated on
  the rows; its consumers see the same numbered rows and the same "seven files" phrasing. A
  per-step split is deferred to its own RFC.
- **The kuten layer.** A kuten profile may *narrow* an occasion's read — a profile that runs only
  extraction phases could name a smaller phase read — and RFC-0028 already gives it the place to
  say so. Nothing here requires it.
- **What the sections say.** No normative sentence changes. An agent that reads everything, as
  today, reads the same words in the same order.

## Migration & compatibility

**Template layer.** Under RFC-0036 the prelude is vendored read-only, and `mise run
yidam-vendor-update` replaces `.yidam/.vendor/prelude/` — but a derivation's `AGENTS.md` was
installed once, at genesis, from `sadhana/root/AGENTS.md`, and nothing re-installs it. So the
occasion headings reach **new** derivations at their next bootstrap and reach **existing** ones
only by hand, or by a REGEN block that does not exist yet (a generator is registered in four
places, and that is its own change). This is the honest cost of the proposal and the open
question below asks which. Nothing in any corpus moves. An agent that ignores the occasion
headings and reads "Full context" behaves exactly as before.

**Consumers of the routes.** `routes()` in the gate discovers a route by three file links; a
route rewritten to name sections still links the three files (as section links or in the
reference index), so discovery is unchanged. `prelude_glossary.rs` requires every route to name
the glossary; core does. `installed_layout_links.rs` resolves every template link inside the
vendored tree; a section link resolves to the same file.

**The bootstrap ceiling.** Falls, once, by the census figure, with the delta attributed to the
named sections — the same raise-and-lower discipline `BOOTSTRAP_CEILING`'s docstring already
records twice.

## Alternatives considered

- **A fifth split.** Retires nothing: the remaining sections are reference, and RFC-0038's own
  scope section says a pair for a document with no essays is overhead. Measured above.
- **Splitting the files by occasion instead of anchoring into them.** `GRAPH.md` would become
  `GRAPH.md` + `class-contract.md` + `commit-vocabulary.md`, and so on. It reads the same and
  costs a consumer migration for every file — ten CLI sources name `GRAPH.md` or
  `directories.md` by path (`cmd/cohort.rs`, `claims.rs`, `paths.rs`, `kuten.rs` among them),
  the docs vocabulary gate quotes them, and every derivation's `AGENTS.md` links them. An
  anchor is a link into a file that does not move. If a section later proves to be read on one
  occasion only and never as reference, moving it then is cheap and this RFC's ceilings say
  which those are.
- **A command that assembles the read** — `yidam read --for establish`, printing the sections.
  It is the cleanest mechanism and the one with the worst adoption evidence: `query`, `pack`
  and `estimate` ran zero times across ~170 derived-repository sessions, and MCP resources were
  fetched zero times in 1,638. Agents read files; a route that names sections meets them where
  they already are. The command is a good second step once the occasion table is stable and
  can be built from the same data.
- **Leaving the read alone and trusting the model to skim.** It does; the derived corpora were
  built with `grep`, and the retrieval layer the route describes at length was never called.
  A route that reads everything cannot say which part mattered, and an agent that skims
  decides that for itself. The route is the one instrument that can say *which* part.

## Open questions

- **Does a section link get read as a section?** The whole proposal rests on an agent handed
  `GRAPH.md#the-class-contract` reading the section and not the file. The derived-repository
  transcripts under `yidam/tests/results/` record every `Read` with its offset and limit; the
  falsifier is to count, over the routes' current file links, how often a read of `GRAPH.md` is
  whole-file versus ranged. If it is whole-file every time, the route must say *stop at* the
  next heading, in words, and the ceiling should be measured on the file the agent will actually
  open. Run this before writing the routes.
- **Is the census a gap in the skill?** "Mark claim confidence" is 1,203 words no step names.
  Either a bootstrap seeds no claim that needs a tag — in which case it is reference and comes
  off the bootstrap read — or step 6 is missing a sentence. The answer decides 1,203 words of
  the bootstrap ceiling and is a question for whoever last seeded a corpus.
- **Which vocabulary names the occasion?** The table uses commit verbs because every agent must
  choose one; PHASES.md's phase types are the alternative and RFC-0028's kuten profiles already
  speak that vocabulary. If the two disagree about where an act belongs, the verb wins here and
  the disagreement is a finding about the phase types.
- **How does an existing derivation get the headings?** Its `AGENTS.md` is its own file. The
  choices are a documented hand-edit in the upgrade notes, or a `<!-- REGEN: yidam routes -->`
  block that `yidam regen` fills from the vendored prelude — which puts the occasion table under
  a generator and makes it data rather than prose. The second is the better shape and the larger
  change; decide it before the routes are written, because the block's boundaries decide what
  hand-edited text survives a regen.
- **The per-step skill.** Deferred, not declined. The trigger is the bootstrap ceiling's next
  raise: if the skill grows again, the argument that it is one occasion has been lost.
