# Directory Conventions

Guidelines for what belongs in each directory of a yidam-derived repository.

After bootstrap, a derived repository has two tiers:

**Top-level** — domain work visible to collaborators and tooling:
- `crates/` — Rust domain computer (connectors, calculators, index)
- `web/` — optional web interface
- `agents/` — domain agent definitions *(created on first use)*
- `docs/` — repository documentation *(created on first use)*
- `packages/` — other-language packages in the same toolkit layer *(created on first use)*

**`.yidam/`** — yidam-managed infrastructure:
- `.yidam/catalog/` — provenance anchors for corpus knowledge
- `.yidam/corpus/` — the living knowledge graph
- `.yidam/decisions/` — structured records of choices made during this repo's life
- `.yidam/skills/` — domain-specific skills
- `.yidam/.vendor/` — inherited yidam prelude; not modified in derived repos
- `.yidam/bin/` — the `yidam` binary built from this repo's pin; git-ignored, see below
- `.yidam/sangha/` — collective resolution protocol *(collective governance only)*

**Created on first use.** Bootstrap does not scaffold `agents/`, `docs/`, or `packages/`.
An empty directory holding only a README that describes what it would contain is
indistinguishable from an abandoned one — and it stays empty: across the two repositories
derived from this template, `agents/` and `packages/` never received a single file and
`docs/` received exactly one. Create each the day something goes in it. The conventions
below say what belongs where when that day comes; the `yidam` CLI treats all three as
optional and its index commands are no-ops when the directory is absent.

---

## `agents/`

Agent definitions for agents that operate in this repository.

**What belongs here:** Agent definitions (system prompts, role descriptions, capability
declarations) for named agents whose purpose is specific to this domain. Generic agents
inherited from yidam live in `.yidam/.vendor/prelude/`; domain-specific agents live here.

---

## `crates/`

Rust crates implementing the retrieval and traversal toolkit — the computational layer that
makes the knowledge graph queryable.

**What belongs here:** Crates implementing the domain computer — the retrieval, calculation,
and feature engineering capabilities that agents use to work with the corpus. Each crate
should have a clear, narrow scope aligned to one of the three capability types below.

**The three capability types:**

*Connectors* — External-facing adapters. A connector fetches data from an API, database,
or external source and returns a validated domain model. Connectors are async, can fail,
and are cached — results are stored locally and refreshed on a TTL or on demand. Connectors
must support an offline mode (falling back to committed fixtures) so tests and analysis
remain hermetic. Name connectors by what they fetch (`nwis`, `echo`, `census`).

*Calculators* — Internal, deterministic transforms. A calculator takes domain models as
input and returns domain models as output. No network, no filesystem — pure functions.
Calculators are the right home for domain-specific computation: hydrological balance,
statistical estimation, unit conversion, graph traversal. They are fully testable without
mocking. Name calculators by what they compute (`lowflow`, `curve-number`, `et`).

*Feature engineering* — Transforms domain data into representations for retrieval and
machine learning. Takes structured corpus data (nodes, edges, extracted values) and produces
embeddings, feature vectors, or derived signals. Feature engineering bridges the corpus and
the index layer; it is distinct from calculators because its outputs are optimized for
retrieval quality, not domain correctness.

**The index layer:** A vector index (e.g., LanceDB) over corpus embeddings enables semantic
retrieval. The index is not the corpus; it is a derived representation of it. Maintaining an
accurate index significantly reduces token consumption: agents retrieve only the nodes
relevant to a phase rather than loading the full corpus.

**Conventions:** Standard Rust crate layout. Each crate exposes a library interface;
binaries are secondary. Prefer composability over monolithic capability.

**Committed fixtures are records, and git will edit them.** A connector's offline fixture is
a record of what a source said when it was asked. Git's line-ending normalization rewrites
that record on checkout, and for most fixtures nobody notices. For some, the line endings
*are* the property under test — a bulk export served with classic-Mac `CR` endings, a
register that serves `CRLF` — and normalizing them silently deletes the thing the fixture
exists to pin. The test then passes against what git produced rather than against what the
source served, which is the failure mode a hermetic fixture was supposed to prevent.

Mark those paths `-text` in `.gitattributes`, and say in the comment which property is being
protected:

```gitattributes
# The SoS bulk exports use classic-Mac CR line endings and these fixtures exist to pin
# that. Normalizing them would quietly delete the property under test.
crates/committee-graph/data/*.csv -text
```

Found in a derived repository by a commit whose message says it exactly: *the fixtures were
being normalised, so the committed record was of what git did rather than what the register
said.*

---

## `docs/`

Documentation about this repository — its purpose, scope, domain conventions, and decisions
that shaped its structure.

**What belongs here:** Repository-level documentation written for contributors, agents, and
users of this domain. This is distinct from the corpus (which holds knowledge claims) and
the prelude (which holds yidam's model). Documentation here describes the *repository*,
not the domain.

**Prose that asserts a number about the corpus must be gated.** Every node is checked and
for a long time nothing checked the account of them — which is how a document here ends up
saying the corpus has 45 nodes across 4 classes when it has 84 across 13, in a sentence that
was true when it was written. The number does not rot loudly. It sits in a table looking
exactly like a number that is still true.

Two mechanisms, in order of preference:

- **A REGEN block**, where the figure is one the CLI already computes. It is regenerated
  rather than checked, so it cannot drift at all. See `yidam status`, `corpus-index`,
  `catalog-audit`.
- **A test**, where the prose is narrative and a REGEN block would flatten it. Read the
  document, parse the figures it publishes, and assert them against the corpus — along with
  the existence of every repo-relative path it links. A derived repository does this for its
  roadmap after three stale records surfaced in three sessions.

State the limit where you build the second one, because it has one: this catches **drift** —
a figure that was right when written and stopped being right while the sentence stayed put.
It cannot catch a false claim about *work*. "These four nodes have been rewritten" is a
sentence about what somebody did, not about what is true of the tree, and no assertion over
the working tree can tell it from a true one. That still needs a reader.

---

## `packages/`

Other-language packages in the same retrieval and traversal toolkit layer as `crates/`.

**What belongs here:** Python, TypeScript, or other runtime packages implementing any of the
three capability types — connectors, calculators, or feature engineering — in a language
better suited to the task than Rust.

**When to use packages/ over crates/:** Ecosystem access (ML frameworks, embedding model
SDKs, geospatial libraries, statistical packages) often determines the language. Prefer
Rust for performance-critical retrieval and index maintenance; prefer Python or TypeScript
for ML pipelines, embedding generation, and connector targets where the upstream SDK is
already Python-native.

---

## `web/`

Web interface layer, if applicable.

**What belongs here:** A frontend or API surface for interacting with the domain computer —
browsing the corpus, issuing retrieval queries, visualizing the graph, or surfacing synthesis.
Optional; add only when direct programmatic access to the crates/packages layer is
insufficient for the intended use.

---

## A directory these conventions do not name

The list above is what yidam knows how to scaffold and check. It is not a closed set, and a
domain will eventually need something not on it.

The rule is about the corpus's boundary, not about the count of directories. **A new
top-level directory is fine. Widening `.yidam/corpus/` to hold something that is not a
documentary claim is not.** The corpus's discipline — every node a claim, every claim
tagged, `graph-check` and `lint` over all of it — is worth exactly as much as the narrowest
thing admitted to it.

The worked case is a repository that took on an advocacy purpose and needed somewhere to
*argue*, which the corpus forbids by construction. Putting argument in the corpus would have
meant relaxing the rule against asserting intent. It created a `dossier/` alongside the
corpus instead, with its own gate crate checking every assertion there back against the
corpus claims beneath it, and the corpus's semantics were untouched. The new purpose made
the evidentiary rules *harder*, because the output became public.

When you add one:

- Say in `AGENTS.md` what the directory is for and what rule it does *not* get to relax
- Record it in `.yidam/decisions/` — a new top-level directory is a structural choice
- Give it a gate. A directory outside `graph-check` is a directory nothing checks; if what
  it holds derives from the corpus, the gate is the derivation

---

## `.yidam/catalog/`

Tracks data sources, allowing corpus nodes to reference them with shallow edges rather than
embedding source metadata inline.

**What belongs here:** One file per data source — datasets, papers, APIs, databases, external
knowledge bases, tool outputs, or any external artifact the corpus draws on. A catalog node
describes the source, not the knowledge derived from it.

**Catalog node conventions:**

- Filename is a stable identifier for the source: author-year for papers (`pearl-2009.md`),
  slug for datasets and APIs (`world-bank-gdp.md`, `openai-embeddings-api.md`)
- Frontmatter carries the structured fields; the body carries prose about the source

**Catalog entries are indexed.** `yidam embed` walks this directory alongside the corpus,
composing each entry's name, type, description, location descriptions and body into one
retrievable document. That is not incidental: in a derived repository the catalog was 51.3%
of the indexable text against the corpus's 41.9%, and for a long time none of it was walked
— so "what is searchable" was being decided by a tool boundary rather than by anyone. If a
source's body holds material that should not be retrievable, `yidam embed --no-catalog`
turns the whole directory off; there is no per-entry opt-out, because a rule somebody has
to remember per file is a rule that gets forgotten.

```yaml
---
name: Pearl 2009
description: Causality — models, reasoning, inference.
type: paper                  # paper | dataset | api | database | other
obtained: true               # absent means true; see below
location:
  - kind: url                # url | url_template | address | file
    value: https://example.org/pearl-2009
    description: publisher's copy   # required only when there are several locations
used-by:
  - ../corpus/concept/confounding.yml
---
```

- **`obtained: false`** declares a source registered ahead of the extraction that will use
  it. It exempts the entry from `catalog-uncited` — which is the honest reason for a source
  nothing draws on yet. The exemption costs something: a node citing a source nobody has
  retrieved is an error (`catalog-unobtained-but-cited`), because either the flag is stale
  or the citation rests on something unread.
- **`obtained: true` means fetched, not read.** The flag is about retrieval and nothing
  else, and an entry can be honestly marked retrieved while every claim inside the document
  has gone unexamined. A derived repository audited all 23 of its entries and found three
  such documents — one located, fetched, cited by nothing, summarized nowhere — while the
  `[open]` claims those documents answered had been carried for months. Nothing detects
  this: the flag is true, the entry is well-formed, and every check passes. So write the
  body to close the gap. **When an entry is created for one fact, say in its body what else
  the document holds, or say plainly that nobody has looked.** The second sentence is the
  useful one; it is the only thing that distinguishes an unread source from a read one.
- **`used-by`** is optional and hand-maintained, so it can drift; the citations cannot.
  Both are kept so the disagreement is visible rather than averaged away
  (`catalog-used-by-drift`). Declaring a list asserts it is current.

Run `yidam schema` to emit JSON Schema for this shape (and for corpus nodes and class
definitions) into `.yidam/schemas/`, then `yidam schema --settings` for the editor mapping
that validates them as you type.

**Relationship to `.yidam/corpus/`:** Corpus nodes link to catalog nodes as edges. A corpus
node on a concept that draws on a source writes `[Pearl 2009](../../catalog/pearl-2009.md)`
rather than embedding a full citation.

---

## `.yidam/corpus/`

The corpus is the primary knowledge store — the body of nodes that constitute the domain graph.

**What belongs here:** Domain concepts, named relationships, artifacts, open questions, and
synthesis notes. Each file is one node. Content should be written to stand alone and be
traversed in any order.

**What does not belong here:** Implementation notes, agent prompts, skill definitions, code,
or anything that describes how the repo operates rather than what it knows.

**Node conventions:**

- One concept per file; one file per concept
- Filenames are kebab-case, descriptive, and stable — renaming a node severs edges, so choose
  well. Do not include dates in filenames; the git history has dates.
- Size: 2–10 sentences is often right. If a node grows beyond a screen, decompose it.
- Every node must have at least one outgoing edge. Orphan nodes do not belong in the corpus.
- If a concept is uncertain or under investigation, mark it: prefix the title with `?` or
  open a branch. Uncertainty is valid; unlabeled speculation is not.

**Node kinds — authored vs. generated:**

*Authored nodes* are written through deliberate knowledge work — by a human or agent
reasoning about the domain. They are stable, permanent, and not regenerable from any source.

*Generated nodes* are produced by a pipeline from a primary source — extracted, computed,
or assembled automatically. They are regenerable if the pipeline is re-run against the same
source.

Both are committed permanently. But their commit semantics differ — generated node commits
are **operational events**; authored node commits are **epistemic events**. Do not mix the
two kinds in a single commit; the log must remain readable as a knowledge record.

---

## `.yidam/decisions/`

Structured records of choices made during this repository's life — from the genesis
bootstrap onward.

**What belongs here:** One YAML file per decision. A decision is any choice that shaped the
repository's structure, ontology, or direction — confirmed ontology sketches, approved
implied edges, connector and calculator selections, governance resolutions.

**Format:**

```yaml
id: <slug>
summary: <one line — what was decided>
context: |
  <what the choice was about>
decision: |
  <what was chosen>
rationale: |
  <why this, not alternatives considered>
```

**Lifecycle:** Written during bootstrap for genesis-level choices; written by agents or the
sangha for subsequent choices. Decision files are permanent records — they are not updated
when a decision is superseded, but a new decision may reference a prior one by `id`.

---

## `.yidam/sangha/` (collective governance only)

Present only in repositories bootstrapped as `governance: collective`. A single-elector
repository does not have this directory and does not need it — see
[CONSTITUTION.md](../CONSTITUTION.md) for what it would govern, and adopt it by scaffolding
this directory if a second elector ever appears.

The collective resolution protocol. Encodes how multiple participants (agents and humans)
maintain individual positions and synthesize them into shared understanding.

**What belongs here:** Protocol documents and the record of governance — not domain
knowledge. `PROTOCOL.md` (resolution algorithm), `electors.md` (recognized participants),
`positions/` (what each elector argued, per question), `resolutions/` (records of past
resolution events). Knowledge lives in the corpus; sangha is the governance layer above it.

**Ref store:** Sangha's live state is in git refs, not in files. `refs/heads/ma/<elector>`
tracks each participant's working position; `refs/heads/rigpa/<evolution>` records settled
collective evolutions. See [GRAPH.md](../GRAPH.md) for the full encoding model.

**The refs hold the corpus, not the argument.** This directory listed protocol documents
only for the whole of the template's early life, on the reasoning that a position is a
branch and a branch is a ref. That is right about which nodes an elector holds and wrong
about why they hold them — and a resolution turns on the why. Once the resolution merges,
an unwritten argument is gone into the merge base, and Articles III and IV have nothing left
to be satisfied by. `positions/<elector>-<question>.md` is where the argument is durable.
A derived repository accumulated 24 of them across 12 resolutions before the conventions had
a slot for them.

**A position is authored on a branch and lives on the baseline.** It is written on the
elector's own `ma/*` branch and then carried here by a `transport:` commit, verbatim.
Authorship and residence are separate questions, and conflating them cost the same derived
repository four corpus nodes whose citations resolved for their author and for nobody else,
plus two resolutions standing on the baseline whose arguments were not. Files under
`positions/` are therefore expected to arrive by transport rather than to be authored in
place — and never to be edited by anyone but the elector whose name they carry.

---

## `.yidam/private-paths` (optional)

Paths whose content must not sit in a public repository. One per line; `#` comments and
blank lines ignored. Absent, nothing is declared private and the CI job that reads it passes
immediately.

```
# Worked lines of attack and material the publication gate computes as [open].
dossier/
```

The CI job fails when the repository is public and any listed path holds a file other than
its `README.md` or `.gitkeep`. It reads `github.event.repository.private` from the event
payload the runner already has — no API call, so CI stays hermetic.

**Why a file rather than a convention.** A repository whose privacy is load-bearing usually
has that fact written in a decision record and nowhere else, which makes it an assumption:
true, relied upon, and unenforced. An assumption about access control that looks enforced
and is not is worse than one everybody knows is manual, because nobody checks the second
kind by hand. Declaring the paths is what turns the assumption into a gate.

### What this does not cover

**This is access control over material at rest. It says nothing about data leaving at
runtime.** Every question it answers is about somebody arriving: can a stranger reach a
file that is committed here. A repository designing a search feature found the other half:

> Every verification in the spike concerns inbound access — that a stranger cannot reach
> the endpoint. None asks what the endpoint does with what it receives. […] That machinery
> is all pointed at people arriving. This channel is the site departing.

The case was a search box forwarding query text to a third-party encoder. Nothing in the
corpus left the repository; the *queries* did, and for a research corpus the queries are
the research agenda — a plainly-worded list of what is being investigated and about whom.
No gate here looks at that, and none can: an egress check would have to know every network
call the domain computer makes, and CI is hermetic precisely so that it makes none.

So it is named rather than gated. The channels a yidam repository typically opens, each of
which is the reader's responsibility and not this file's:

- **Connectors** send whatever they query to the source. A search term, an identifier, or
  a person's name in a request URL discloses that you are asking about it.
- **A deployed web shell** sends every request its reader makes to whoever hosts it, and
  its logs are a record of what was searched and what was opened.
- **A hosted encoder or model API** receives the text it is asked to embed or complete —
  queries at minimum, and the corpus itself if embedding is done remotely.
- **Anything with telemetry**, including tools run against the repository.

Apply the same rule the private-paths gate applies to access: **unknown is not proof of
protected.** A channel nobody has examined is not a channel known to be safe, and a comment
asserting that an exposure does not exist is worse than no comment, because it stops the
next reader from looking. Where a channel matters, examine it and record what you found in
`.yidam/decisions/` — including, honestly, what remains unobtained.

---

## `.yidam/bin/`

The `yidam` binary this repository runs, installed by `mise run yidam-build` from the commit
`.yidam.toml` pins. Git-ignored — it is build output, and it is rebuilt by the same command
that updates the pin.

**Beside the pin, and not in `~/.cargo/bin`.** `cargo install` with no `--root` writes to one
location per *machine*, while the pinned commit is one per *repository*. On a machine with
two yidam repositories that is last-writer-wins: repository A builds its pin, repository B
builds its own, and A now runs a binary that does not match its own vendored prelude with
nothing anywhere saying so. The guarantee the build task exists to keep — that the binary and
the prelude agree — held for exactly one repository at a time.

It was not theoretical. Three separate builds displaced the machine-wide binary during the
session that fixed this, one of them with a copy predating the `--format` flag entirely, which
would have left every JSON report unreadable.

`mise.yidam.toml` puts `.yidam/bin` first on `PATH` for anything mise runs, so the shell and
the editor resolve the same binary. **If your shell does not run mise, add it yourself** — a
`yidam` from somewhere else will otherwise answer for this repository. The VS Code extension
checks `.yidam/bin/yidam` ahead of `PATH` for the same reason.

---

## `.yidam/authorship.yml` (optional)

What in this repository is not authored here.

Checks that read prose walk directories, and a walk cannot tell authored material from
material that merely landed there. `broken-prose-link` shipped knowing about exactly one such
directory, `.yidam/.vendor/`, on a rationale that generalizes perfectly: a defect in the
prelude is fixed upstream and adopted by re-vendoring, so reporting one here hands this
repository a finding it cannot act on.

A consumer that is not a vendoring repository met the same wall from the other side. Its
first gated run produced **43 broken prose links under `docs/` at error severity, and fifteen
of them were inside a directory whose own README says it is a frozen, unmodified copy of an
upstream project at a fork point.** Editing those to satisfy a linter falsifies the record
the directory exists to keep. It baselined them and moved on, which is the wrong instrument:
a baseline records *we accept this violation*, when the truth is *this file is not ours*. The
two decay differently — a baselined entry that is later repaired fails the build, by design,
so for a frozen import the gate came to depend on nobody ever re-syncing it.

```yaml
generated:
  - path: .yidam/reports/
    by: yidam report

imported:
  - path: docs/reference/upstream/
    from: acme/gis at the fork point

excluded:
  - path: docs/scratch/
    why: working notes, deliberately unmaintained
```

Paths are repo-root-relative and cover everything beneath them, matched on path components —
`docs/ref` does not cover `docs/reference/`. The first declaration wins an overlap.

**Each kind requires the field that names who can act on a finding inside it,** and a region
declared without one does not parse. That is the whole weight of the mechanism: `generated`
and `imported` are claims about where material came from, and a claim with no addressee is a
request for silence wearing a provenance label.

| Kind | What it asserts | What the gate does |
|---|---|---|
| `generated` | Written by this repository's own tooling. | Reported at info severity, addressed to the generator. Fix the generator — the file is rewritten by the next build, so fixing the file does not persist. |
| `imported` | Copied from elsewhere and not modified. | Reported at info severity, addressed upstream. |
| `excluded` | Neither. | Not read at all. |

Only `excluded` produces silence, and it is named so that a reviewer meeting it in the
manifest sees the escape hatch as one. The other two are still real defects. They are simply
somebody else's, and a finding that says whose is worth more than one that says nothing —
the generated half in the case above was the same defect twice, a generator emitting
corpus-relative link targets into a file that landed in a sibling directory.

`.yidam/.vendor/` is built in as `imported` and is not declared here: it is yidam's claim
about a directory yidam manages, not this repository's.

**A declaration that matches nothing is reported** — `authorship-region-stale` — because a
manifest permitted to be wrong drifts exactly as a lint baseline does, and the entry that
outlives its directory quietly excuses a path somebody later creates under the same name.
`generated` regions are exempt: they are written by a build and are frequently git-ignored,
so absence on a fresh clone carries no information. The check is Warn rather than Error, for
the reason the file exists at all — an imported region is re-synced upstream on somebody
else's schedule, and gating on it would make this build's colour depend on that.

---

## `.yidam/skills/`

Reusable capabilities available to agents in this repository.

**What belongs here:** Domain-specific skills — structured procedures agents can invoke
when working in this repo. Generic skills inherited from yidam live in `.yidam/.vendor/prelude/`;
skills that require knowledge of this domain's corpus or toolkit live here.

---

## `.yidam/.vendor/`

The inherited yidam prelude, moved here by the `vendor:` commit during bootstrap.

**What belongs here:** `prelude/` and nothing else. The vendor step moves `yidam/prelude/`
to `.yidam/.vendor/prelude/` and deletes the rest of the template.

**What deliberately does not belong here:** yidam's CLI source, its bootstrap test harness,
its design notes, and its docs site. None of them are readable, runnable, or updatable from
inside a derived repo. A vendored copy of the CLI is a fork that will never be rebuilt — the
`yidam` binary is installed from the pinned origin (`mise run yidam-build`), not compiled
from a snapshot. A vendored copy of the harness brings `HARNESS.md`, whose links point at
scenario files the derived repo does not have.

**Read-only.** Do not modify anything under `.yidam/.vendor/` in the course of domain work.
An edit here is silently discarded the next time the prelude is re-vendored, and until then
it is a local divergence nobody can see. A defect in the prelude is fixed upstream in yidam
and adopted by re-vendoring. That is also why `yidam lint` reports a broken link
here at info severity rather than as a violation of this repository — see
`.yidam/authorship.yml` above, of which this directory is the built-in instance.

**Note:** Paths to inherited skills and agents use `.yidam/.vendor/prelude/` — for example,
the bootstrap skill lives at `.yidam/.vendor/prelude/skills/bootstrap.md` after genesis.

---

## `.yidam.toml` (repository root)

The provenance pin: which yidam this repository was derived from. Written by `yidam clone`
or `yidam overlay`, confirmed by the bootstrap vendor step, and updated by
`mise run yidam-vendor-update`.

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "4f2a…"      # the resolvable pin — what re-vendor and CI check out
template  = "v0.1.0"     # release tag at that commit, or "untagged"
committed = "2026-08-08" # that commit's date — how old this prelude is
```

`commit` is the field that does the work. `template` is a semantic version and is only
meaningful once the origin is tagged; a pin that records a version but no commit points at
nothing. `committed` is the *upstream* commit's date, not the date this repo last ran the
vendor step — it answers how old the prelude is, which is what staleness turns on. See
[VERSIONING.md](https://github.com/goedelsoup/yidam/blob/main/VERSIONING.md) for the three
release layers.

**Re-vendoring.** The prelude is not frozen at the repository's birth. Corrections made
upstream reach a derived repo when it re-vendors:

```
mise run yidam-vendor-status    # what you are pinned to, and what is newer
mise run yidam-vendor-update    # fetch, replace prelude/ and mise.yidam.toml, re-pin .yidam.toml
```

The update replaces `.yidam/.vendor/prelude/` wholesale, rewrites `.yidam.toml`, and
replaces `mise.yidam.toml`. It touches nothing else — `corpus/`, `catalog/`, `decisions/`,
`skills/`, `crates/`, and every other top-level file are domain-owned and are never
overwritten by an update.

`mise.yidam.toml` is on that list because it is inherited, not domain-owned: it is the task
layer, as much yidam's to correct as the prelude is, and it sits at the repo root only
because mise has to find it there. It was omitted originally on the reasoning that the
update should touch nothing outside `.yidam/` — which left it with no update path at all. A
derived repository froze the copy it was born with permanently, including a prescribed
commit verb this project later found its own lint rejects. **Keep domain tasks in
`mise.toml`; anything written into `mise.yidam.toml` is replaced on the next update.**

Review the resulting diff and commit it as its own event:

```
git commit -m "vendor: re-vendor prelude at <commit> — <what changed>"
```

Re-vendor deliberately, not reflexively. A prelude change can alter what the graph gate
accepts; adopting one is a decision worth its own commit and, if it changes conventions the
corpus depends on, its own record in `.yidam/decisions/`.

**Staleness is invisible from the inside, so it is reported from the outside.** The derived
repo CI prints how far behind the pin is on every run, and escalates to a warning when
`GRAPH.md` has moved — that is where the closed commit vocabulary lives, it is the part of
the prelude this repository has already written history against, and history cannot be
rewritten to match a verb. `yidam-vendor-update` prints which verbs were added or removed
between the old pin and the new one, for the same reason.

Neither gates. A repository is entitled to stay pinned, and a build that goes red because
upstream moved is a build somebody switches off.

### Sending a finding back

Re-vendoring carries corrections **downstream**. Nothing carries findings **up**, and a
derived repository is where most defects in this template are actually discovered — it is
the only place the conventions meet a real corpus.

The cost of having no return path is not hypothetical. On one day, upstream added a verb to
the closed vocabulary, citing a derived repository's use of it as the evidence the
vocabulary had a gap; that same repository spent four commits the same day removing the
verb, on the correct reasoning that no gap existed in the prelude *it* could see. Both were
right about their own evidence. Neither could see the other, and the derived repo ended
further from upstream than it started.

So: when domain work here runs into a defect in the prelude, the CI, the CLI, or these
conventions, **open an issue upstream** rather than working around it locally. A local
workaround is invisible to every other derived repository and is discarded at the next
re-vendor.

```
gh issue create --repo goedelsoup/yidam --label "from:derived-repo"
```

What makes such a report actionable, in rough order of value:

- **The file and line in the prelude or CLI**, not just the symptom.
- **What it cost here** — the commits spent, the checks that passed while wrong, the
  workaround now in place. This is the part upstream cannot reconstruct and the part that
  decides priority.
- **The pin you are on**, from `.yidam.toml`. A defect already fixed upstream is a
  re-vendor, not an issue.
- **Whether you worked around it**, and where, so the workaround can be removed when the
  fix lands.

Do not send corpus content. A finding is about the template; the domain material that
exposed it usually should not leave the repository, and often may not.

---

## `samudaya/` (transient — present only before and during bootstrap)

A transient bootstrap influence layer, consumed and committed away as part of the genesis
event. See [samudaya/README.md](https://github.com/goedelsoup/yidam/blob/main/samudaya/README.md) for the full
protocol — the directory itself is deleted at genesis, so a relative link to it is
broken in every derived repository.

**Presence after genesis is an error state.**

---

## `sadhana/` (transient — present only during bootstrap)

The scaffold template layer. Provides the initial content for each derived-repo directory.
Bootstrap reads these templates, creates the derived-repo structure from them, then deletes
this directory. Like samudaya, it does not survive genesis.

**Presence after genesis is an error state.**
