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
- `.yidam/index.lock` — which computed artifacts are in which vault, and which store holds
  each; committed, and written by `yidam vault push --index`
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

**A type declared here is a claim about the domain, and `yidam check-diff a..b` asks about
it.** The ontology is a contract that every check reads from the corpus side; this reads it
from the code side, comparing the type and enum names a diff *adds* under `crates/` against
the classes, properties and relationships `.ont.yml` declares. `RatingCurve` where nothing is
named `rating-curve` is a question — is this a concept the corpus should model, or a helper
the ontology has no reason to know about? — and the answer is yours. It never gates and never
will: the fix is a new class or a decision not to have one, and both are judgement.

Expect it to be quiet. Across the three repositories derived from this template the median
code-touching commit introduces no new type at all, and it is the commit that lands ten that
you want to be asked about. Where it is *not* quiet, the vocabulary is what to look at: one
of those repositories declares fifteen classes while its code defines two hundred and
seventy-five types, and a 7% match rate is not a naming problem.

Do not exclude test or fixture code by inventing a path convention. `.yidam/authorship.yml`
is the vocabulary for that, and it is the same one every prose check uses — a region declared
`imported` or `generated` is still reported, at info severity, naming who can act on it;
only `excluded` is silent.

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
retrieved: 2026-08-22        # optional; when it was last actually fetched
ttl_days: 3650               # optional; how long this record may stand
location:
  - kind: url                # url | url_template | address | file
    value: https://example.org/pearl-2009
    description: publisher's copy   # required only when there are several locations
used-by:
  - ../corpus/concept/confounding.yml
artifacts:                     # optional; what was actually obtained
  - sha256: 9f2c8e…            # 64 lowercase hex — the content address
    bytes: 4194304
    media_type: application/pdf
    retrieved: 2026-08-22
    from: 0                    # which `location` it came from, or a URL
    vault: sources             # optional; overrides the route its kind takes. `none` = local only
    redistributable: false     # whether they may leave this machine at all
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
- **`ttl_days` says how long this record may stand**, and `retrieved` says what to count
  from. Both are optional and absent means nothing expires. Declare it per entry, because a
  gauge record and a statute do not age at the same rate — Pearl 2009 will say what it says
  in ten years, and an inventory pulled from an API will not. A corpus whose sources mostly
  age alike can set one default instead, under `[catalog] ttl_days` in `.yidam/config.toml`;
  an entry's own value always wins.

  **Days, not commits.** Every other clock in yidam counts commits, because a corpus-state
  finding must be a function of `HEAD` rather than of when you ran the report. This one is
  different in kind: a statute does not become stale because you committed, and a gauge
  record does not stay fresh because you did not.

  Without `retrieved`, the date is read from the commit that last touched the entry's file —
  which counts a typo fix as a refresh, so it errs in the flattering direction. `yidam lint`
  says which of the two it used, every time. **Expiry is reported, never enforced**, and it
  does not claim the source changed: nothing here reads upstream and `doctor` does no
  network. It claims nobody has looked. Refreshing a source is a knowledge event, and it is
  yours to own.
- **`used-by`** is optional and hand-maintained, so it can drift; the citations cannot.
  Both are kept so the disagreement is visible rather than averaged away
  (`catalog-used-by-drift`). Declaring a list asserts it is current.

- **`artifacts`** is what makes `obtained: true` *demonstrable*. The flag says a source was
  fetched; until this existed, nothing anywhere held what was fetched, so an entry marked
  obtained and an entry marked obtained falsely were the same observation and every check
  passed on both. A digest is the difference.

  The **bytes are not in the repository**. They live in a vault — a content-addressed store,
  configured under `[vault.…]` in `.yidam/config.toml` — and what is committed here is the
  record of them. That split is the whole design: a stale vault cannot lie, because the digest
  is in the commit; and losing a vault costs no knowledge claim, only the time to re-fetch.
  `yidam vault --help` lists the commands.

  Optional, and absent on every entry written before it existed. **Adopting it is a corpus
  deciding to record what it holds, not a requirement arriving in a build** — the two checks
  that read it fire only on entries that declare it, so a corpus that has not adopted the field
  sees no new findings at all.

  - `sha256` is required per record and is **64 lowercase hex characters**. Lowercase because
    hex is case-insensitive and a content-addressed store is not: two spellings would be two
    keys for one artifact. `catalog-artifact-malformed` reports the rest — a digest of the
    wrong length or alphabet, or a `from:` index naming a location the entry does not declare.
  - `vault` says **where these bytes may be kept**, and is optional. Omitted, the artifact
    goes wherever the config routes its kind: a lone vault that declares no `holds` takes
    everything, and where there are several, the one whose `holds` lists `catalog` takes it.
    Naming a vault here **overrides that route** — the specific assertion outranks the general
    one — and must name a store `.yidam/config.toml` declares, or `none`. `none` is a route,
    the local cache and nowhere else, spelled rather than omitted so that *nobody has decided*
    and *decided to keep it here* are different states. `catalog-artifact-unroutable` reports a
    name nothing declares; it may gate because both sides are committed, so it answers
    identically in every clone. An artifact whose *kind* no vault claims is reported by
    `yidam vault push` and `yidam doctor` rather than by lint, because the defect is in the
    config and blaming the catalog entry would point at the wrong file.
  - `redistributable` is a **licensing fact about the source**, and it is deliberately not
    folded into `vault`. A route is edited casually — somebody reorganising storage moves a
    dozen entries between stores in an afternoon — and a licence is not something that edit is
    allowed to undo. Keeping it separate means the reorganisation meets a refusal instead of
    publishing a paper.

  **What no check here can tell you is whether the bytes are present or correct.** That is a
  fact about the machine asking rather than about `HEAD`, and every check in `yidam lint` reads
  the working tree and nothing else — a gate whose verdict depended on which machine ran it is
  one a corpus could not reason about. `yidam vault verify` answers it, per machine, which is
  the only place the answer means anything.

Run `yidam schema` to emit JSON Schema for this shape (and for corpus nodes and class
definitions) into `.yidam/schemas/`, then `yidam schema --settings` for the editor mapping
that validates them as you type.

**Relationship to `.yidam/corpus/`:** Corpus nodes link to catalog nodes as edges. A corpus
node on a concept that draws on a source writes `[Pearl 2009](../../catalog/pearl-2009.md)`
rather than embedding a full citation.

**A citation is a link that resolves to the entry** — either a markdown link in the prose or
a `links:` target. Naming the slug in a sentence is not one, and the checks now agree with
that. They used to match the bare slug anywhere in a node's bytes, so a node that merely
mentioned a source was reported as citing it. Under `catalog-unobtained-but-cited`, which is
Error severity and gates, that failed a build on a node containing no citation at all.

The collision that surfaced it is not exotic: these conventions recommend naming connectors
after what they fetch (`nwis`, `echo`, `census`), and a catalog entry for the source those
connectors fetch from carries the same slug by design. Any node discussing the crate tripped
the check.

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
- Size: 2–10 sentences is often right. If a node grows beyond a screen, decompose it. A
  class may make that checkable by declaring `max_lines:` in its `.ont.yml`, and
  `node-too-long` then reports an instance over it. No class carries a default, and that is
  measured rather than timid: the bootstrap rubric caps a node at 40 lines, and across five
  real corpora 335 of 410 nodes exceed it — 86%, 86% and 97% in the three mature ones — while
  the same corpora at their genesis commits run to a median of 35, where 40 is right for
  three of four. So 40 is a *genesis* norm that a corpus grows out of, and growing out of it
  is what a corpus doing its job looks like. There is no knee in the distribution to put a
  steady-state number at; it runs smoothly from 20 to 534. The length an instance should be
  is a question about its class — a statutory obligation quoting the text it arises from is
  not the length of a person — so the class is where the number lives, if a corpus wants one.
- A class may name the type that implements it, with `implemented_by:` in its `.ont.yml`.
  `unimplemented-class` then **gates** when the tree defines no `struct` or `enum` of that
  name: the class stated a fact about `crates/`, and a missing type contradicts it rather
  than merely omitting something. A class that omits the field is not checked at all, and
  that default is measured rather than timid — across twelve derived corpora 129 of 157
  declared classes have no type bearing their name, and matching traits, aliases and every
  language in the tree makes it worse, 165 of 186. Five of those corpora match nothing at
  all, and they are not behind: an ontology models a domain while `crates/` models the
  pipeline that gathers evidence about it, so a class without a type is the ordinary case.
  Name the Rust type as Rust spells it rather than expecting the class name to be derived —
  `HTTPServer` and `HttpServer` are two types and one kebab-case name.
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

### Derived artifacts inherit the privacy of what they were derived from

`.yidam/index/` and `.yidam/embeddings/` are not files that happen to sit next to the corpus.
They are a re-encoding of it: each indexed row carries the node's own text, composed from
`.yidam/corpus/` **and** `.yidam/catalog/`. A bundle carries the index inside it.

So a path declared private that overlaps either directory makes the index private too, and
`yidam vault push --index` refuses on exactly that ground, naming the path. The refusal is the
same rule `sadhana/github/workflows/release.yml` applies to a bundle and for the same reason —
*the artifact outlives the access* — extended to the channel a vault opens.

A declared directory holding only a `README.md` or a `.gitkeep` is a statement of intent rather
than material, and does not refuse. Declaring the path before there is anything in it is the
order this file asks you to work in, and it should not cost you a push.

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

**And through invocation, which needs no mis-installation at all.** The paragraph above is
about where a binary gets *written*. The same hazard arrives from where one gets *found*: a
`yidam` left in `~/.cargo/bin` by any earlier install — including one predating the `--root`
convention — shadows `.yidam/bin/yidam` for any process whose `PATH` puts cargo's directory
first. That is not a mistake anyone made; it is what a shell sourcing a Rust environment does
by default.

Your `mise.toml` puts `.yidam/bin` first on `PATH` for anything mise runs, so the shell and
the editor resolve the same binary:

```toml
[env]
_.path = [".yidam/bin"]
```

**In `mise.toml`, not `mise.yidam.toml`.** The inherited layer is a mise *task file*, where
`[env]` declares a task named `env` and `_.path` is an unknown field — which orphans every
task in the file. This paragraph asserted the opposite for as long as the declaration sat in
the file that could not hold it, so the guarantee named here was one nothing delivered.

**If your shell does not run mise, add it yourself** — a `yidam` from somewhere else will
otherwise answer for this repository. The VS Code extension checks `.yidam/bin/yidam` ahead
of `PATH` for the same reason.

That guards a human shell. It does not guard a script, a CI step, or an agent that assembles
`PATH` itself, and those are increasingly what runs these commands. **Wherever you build
`PATH` by hand, put `.yidam/bin` first** — the ordering is load-bearing, not a convenience.

The failure is quiet in the way that matters. An older binary lacking a subcommand exits with
`unrecognized subcommand 'regen'`, and inside a script with output redirected — which is how a
regen step is usually written — that is indistinguishable from success. `regen --check` is a
real backstop, but it fires on the next full run; between the no-op and that run the
repository holds a stale generated block and the command that refreshes it reports nothing
wrong.

So the binary now says so itself. When `.yidam/bin/yidam` exists and is not the executable
running, every command warns on stderr naming both paths, and an unrecognized subcommand adds
which binary refused it. Stderr, so a `--format json` consumer reading stdout is unaffected.

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
