# Directory Conventions

Guidelines for what belongs in each directory of a yidam-derived repository.

**How to read this file.** Every rule below is a sentence you can act on. The incident that
produced a rule, the measurement that set its threshold, and the failure it was built against
are in [directories.evidence.md](directories.evidence.md) — one section per rule, reached by
the `[why]` link beside it. Read it when a rule surprises you, when you are about to argue with
one, or when you need to know how far a number can be pushed. You do not need it in order to
comply, and it is not part of the recurring read.

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
- `.yidam/tonpa.toml` and `.yidam/tonpa/` — the corpora this one depends on, and where they land
- `.yidam/decisions/` — structured records of choices made during this repo's life
- `.yidam/capabilities.toml` — what may run here, what it reads and what it writes
- `.yidam/runs/` — one receipt per capability: what ran, against what, producing which bytes
- `.yidam/computed/` — what this repository worked out about itself; committed, and read back
- `.yidam/skills/` — domain-specific skills
- `.yidam/.vendor/` — inherited yidam prelude; not modified in derived repos
- `.yidam/bin/` — the `yidam` binary built from this repo's pin; git-ignored, see below
- `.yidam/index.lock` — which computed artifacts are in which vault, and which store holds
  each; committed, and written by `yidam vault push --index`
- `.yidam/sangha/` — collective resolution protocol *(collective governance only)*

**Created on first use.** Bootstrap does not scaffold `agents/`, `docs/`, or `packages/`.
Create each the day something goes in it. The conventions below say what belongs where when that
day comes; the `yidam` CLI treats all three as optional and its index commands are no-ops when
the directory is absent. [why](directories.evidence.md#created-on-first-use)

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
and feature engineering capabilities that agents use to work with the corpus. Each crate should
have a clear, narrow scope aligned to one of the three capability types below.

**The three capability types:**

*Connectors* — External-facing adapters. A connector fetches data from an API, database, or
external source and returns a validated domain model. Connectors are async, can fail, and are
cached — results are stored locally and refreshed on a TTL or on demand. Connectors must
support an offline mode (falling back to committed fixtures) so tests and analysis remain
hermetic. Name connectors by what they fetch (`nwis`, `echo`, `census`).

*Calculators* — Internal, deterministic transforms. A calculator takes domain models as input
and returns domain models as output. No network, no filesystem — pure functions. Calculators
are the right home for domain-specific computation: hydrological balance, statistical
estimation, unit conversion, graph traversal. They are fully testable without mocking. Name
calculators by what they compute (`lowflow`, `curve-number`, `et`).

*Feature engineering* — Transforms domain data into representations for retrieval and machine
learning. Takes structured corpus data (nodes, edges, extracted values) and produces
embeddings, feature vectors, or derived signals. Feature engineering bridges the corpus and the
index layer; it is distinct from calculators because its outputs are optimized for retrieval
quality, not domain correctness.

**The index layer:** A vector index (e.g., LanceDB) over corpus embeddings enables semantic
retrieval. The index is not the corpus; it is a derived representation of it. Maintaining an
accurate index significantly reduces token consumption: agents retrieve only the nodes relevant
to a phase rather than loading the full corpus.

**Conventions:** Standard Rust crate layout. Each crate exposes a library interface; binaries
are secondary. Prefer composability over monolithic capability.

**Committed fixtures are records, and git will edit them.** A connector's offline fixture is a
record of what a source said when it was asked, and git's line-ending normalization rewrites
that record on checkout. Where the line endings *are* the property under test, mark those paths
`-text` in `.gitattributes`, and say in the comment which property is being protected:

```gitattributes
# The SoS bulk exports use classic-Mac CR line endings and these fixtures exist to pin
# that. Normalizing them would quietly delete the property under test.
crates/committee-graph/data/*.csv -text
```

[why](directories.evidence.md#committed-fixtures)

**A type declared here is a claim about the domain, and `yidam check-diff` asks about it.** Run
bare it reads this branch's work — the merge-base with `main` — and it takes an explicit range
too. It compares the type and enum names a diff *adds* under `crates/` against the classes,
properties and relationships `.ont.yml` declares, and reports a name the ontology does not know —
`RatingCurve` where nothing is named `rating-curve`. The answer is yours: a new class, or a
decision not to have one. It never gates and never will, because both are judgement.

**Expect it to be quiet.** Where it is *not* quiet, the vocabulary is what to look at.
[why](directories.evidence.md#check-diff-is-quiet)

Do not exclude test or fixture code by inventing a path convention. `.yidam/authorship.yml` is
the vocabulary for that, and it is the same one every prose check uses — a region declared
`imported` or `generated` is still reported, at info severity, naming who can act on it; only
`excluded` is silent.

---

## `docs/`

Documentation about this repository — its purpose, scope, domain conventions, and decisions
that shaped its structure.

**What belongs here:** Repository-level documentation written for contributors, agents, and
users of this domain. This is distinct from the corpus (which holds knowledge claims) and the
prelude (which holds yidam's model). Documentation here describes the *repository*, not the
domain.

**Prose that asserts a number about the corpus must be gated.** The number does not rot loudly.
It sits in a table looking exactly like a number that is still true.
[why](directories.evidence.md#gated-prose-numbers)

Two mechanisms, in order of preference:

- **A REGEN block**, where the figure is one the CLI already computes. It is regenerated rather
  than checked, so it cannot drift at all. See `yidam status`, `corpus-index`, `catalog-audit`.
- **A test**, where the prose is narrative and a REGEN block would flatten it. Read the
  document, parse the figures it publishes, and assert them against the corpus — along with the
  existence of every repo-relative path it links.

State the limit where you build the second one: it catches **drift**, a figure that was right
when written and stopped being right while the sentence stayed put. It cannot catch a false claim
about *work* — "these four nodes have been rewritten" is a sentence about what somebody did, and
no assertion over the working tree can tell it from a true one. That still needs a reader.
[why](directories.evidence.md#drift-not-falsehood)

---

## `packages/`

Other-language packages in the same retrieval and traversal toolkit layer as `crates/`.

**What belongs here:** Python, TypeScript, or other runtime packages implementing any of the
three capability types — connectors, calculators, or feature engineering — in a language better
suited to the task than Rust.

**When to use packages/ over crates/:** Ecosystem access (ML frameworks, embedding model SDKs,
geospatial libraries, statistical packages) often determines the language. Prefer Rust for
performance-critical retrieval and index maintenance; prefer Python or TypeScript for ML
pipelines, embedding generation, and connector targets where the upstream SDK is already
Python-native.

---

## `web/`

Web interface layer, if applicable.

**What belongs here:** A frontend or API surface for interacting with the domain computer —
browsing the corpus, issuing retrieval queries, visualizing the graph, or surfacing synthesis.
Optional; add only when direct programmatic access to the crates/packages layer is insufficient
for the intended use.

---

## A directory these conventions do not name

The list above is what yidam knows how to scaffold and check. It is not a closed set, and a
domain will eventually need something not on it.

The rule is about the corpus's boundary, not about the count of directories. **A new top-level
directory is fine. Widening `.yidam/corpus/` to hold something that is not a documentary claim
is not.** The corpus's discipline — every node a claim, every claim tagged, `graph-check` and
`lint` over all of it — is worth exactly as much as the narrowest thing admitted to it.
[why](directories.evidence.md#a-directory-not-named)

When you add one:

- Say in `AGENTS.md` what the directory is for and what rule it does *not* get to relax
- Record it in `.yidam/decisions/` — a new top-level directory is a structural choice
- Give it a gate. A directory outside `graph-check` is a directory nothing checks; if what it
  holds derives from the corpus, the gate is the derivation

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
retrievable document. If a source's body holds material that should not be retrievable,
`yidam embed --no-catalog` turns the whole directory off; **there is no per-entry opt-out**, because a
rule somebody has to remember per file is a rule that gets forgotten.
[why](directories.evidence.md#catalog-is-indexed)

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

- **`obtained: false`** declares a source registered ahead of the extraction that will use it.
  It exempts the entry from `catalog-uncited` — which is the honest reason for a source nothing
  draws on yet. The exemption costs something: a node citing a source nobody has retrieved is
  an error (`catalog-unobtained-but-cited`), because either the flag is stale or the citation
  rests on something unread.
- **`obtained: true` means fetched, not read.** The flag is about retrieval and nothing else,
  and nothing detects an entry honestly marked retrieved whose document has gone unexamined. So
  write the body to close the gap: **when an entry is created for one fact, say in its body what
  else the document holds, or say plainly that nobody has looked.**
  [why](directories.evidence.md#obtained-means-fetched)
- **`ttl_days` says how long this record may stand**, and `retrieved` says what to count from.
  Both are optional and absent means nothing expires. Declare it per entry, because a gauge
  record and a statute do not age at the same rate. A corpus whose sources mostly age alike can
  set one default instead, under `[catalog] ttl_days` in `.yidam/config.toml`; an entry's own
  value always wins.

  **Days, not commits** — the one clock in yidam that does not count commits. Without
  `retrieved`, the date is read from the commit that last touched the entry's file, and `yidam
  lint` says which of the two it used, every time. **Expiry is reported, never enforced**, and it
  does not claim the source changed: it claims nobody has looked.
  [why](directories.evidence.md#ttl-days-days-not-commits)
- **`used-by`** is optional and hand-maintained, so it can drift; the citations cannot. Both are
  kept so the disagreement is visible rather than averaged away (`catalog-used-by-drift`).
  Declaring a list asserts it is current.

  **Omitting the key and writing `used-by: []` are different declarations.** No key at all says
  nothing about what cites the entry, and nothing checks it — that is the honest state for an
  entry whose list you do not intend to maintain. An empty sequence says *nothing cites this*,
  which is a claim, and it is checked like any other: once a node cites the entry, the claim is
  false and `catalog-used-by-drift` reports it. `catalog-reconcile` repairs an empty list for
  the same reason it repairs a wrong one, and still leaves an absent key alone.

- **`artifacts`** is what makes `obtained: true` *demonstrable*: the flag says a source was
  fetched, and a digest is what makes the claim checkable.
  [why](directories.evidence.md#artifacts-demonstrable)

  The **bytes are not in the repository**. They live in a vault — a content-addressed store,
  configured under `[vault.…]` in `.yidam/config.toml` — and what is committed here is the
  record of them. `yidam vault --help` lists the commands.

  Optional, and absent on every entry written before it existed. The two checks that read it fire
  only on entries that declare it, so a corpus that has not adopted the field sees no new
  findings at all.

  - `sha256` is required per record and is **64 lowercase hex characters** — hex is
    case-insensitive and a content-addressed store is not.
    `catalog-artifact-malformed` reports the rest: a digest of the wrong length or alphabet, or a
    `from:` index naming a location the entry does not declare.
  - `vault` says **where these bytes may be kept**, and is optional. Omitted, the artifact goes
    wherever the config routes its kind: a lone vault that declares no `holds` takes
    everything, and where there are several, the one whose `holds` lists `catalog` takes it.
    Naming a vault here **overrides that route**, and must name a store `.yidam/config.toml`
    declares, or `none`. `none` is a route — the local cache and nowhere else — spelled rather
    than omitted so that *nobody has decided* and *decided to keep it here* are different
    states. `catalog-artifact-unroutable` reports a name nothing declares, and may gate because
    both sides are committed. An artifact whose *kind* no vault claims is reported by
    `yidam vault push` and `yidam doctor` rather than by lint.
    [why](directories.evidence.md#vault-route-override)
  - `redistributable` is a **licensing fact about the source**, and it is deliberately not
    folded into `vault`. [why](directories.evidence.md#redistributable-separate)

  **What no check here can tell you is whether the bytes are present or correct.** That is a
  fact about the machine asking rather than about `HEAD`; `yidam vault verify` answers it, per
  machine. [why](directories.evidence.md#bytes-present-is-per-machine)

Run `yidam schema` to emit JSON Schema for this shape (and for corpus nodes and class
definitions) into `.yidam/schemas/`, then `yidam schema --settings` for the editor mapping that
validates them as you type.

**Relationship to `.yidam/corpus/`:** Corpus nodes link to catalog nodes as edges. A corpus
node on a concept that draws on a source writes `[Pearl 2009](../../catalog/pearl-2009.md)`
rather than embedding a full citation.

**A citation is a link that resolves to the entry** — either a markdown link in the prose or a
`links:` target. Naming the slug in a sentence is not one, and the checks agree with that.
[why](directories.evidence.md#a-citation-is-a-link)

---

## `.yidam/corpus/`

The corpus is the primary knowledge store — the body of nodes that constitute the domain graph.

**What belongs here:** Domain concepts, named relationships, artifacts, open questions, and
synthesis notes. Each file is one node. Content should be written to stand alone and be
traversed in any order.

**What does not belong here:** Implementation notes, agent prompts, skill definitions, code, or
anything that describes how the repo operates rather than what it knows.

**How to read it:** with `yidam query`, `neighbors` and `pack` — not with `grep`. This is a
directory of files and it is also a typed graph, and only the second answers *what is one hop
from here* or *does this corpus cover that*. See [Reading the corpus](reading-the-corpus.md).

**Node conventions:**

- One concept per file; one file per concept
- Filenames are kebab-case, descriptive, and stable — renaming a node severs edges, so choose
  well. Do not include dates in filenames; the git history has dates.
- Size: 2–10 sentences is often right. If a node grows beyond a screen, decompose it. A class
  may make that checkable by declaring `max_lines:` in its `.ont.yml`, and `node-too-long` then
  reports an instance whose **prose** runs over it — every field the ontology declares as
  prose, which is `description` plus whatever the class or `universal.yml` names. The prose and
  not the file. **No class carries a default**, and the length an instance should be is a
  question about its class — a statutory obligation quoting the text it arises from is not the
  length of a person — so the class is where the number lives, if a corpus wants one.
  [why](directories.evidence.md#max-lines-measures-prose)
- A class may name the type that implements it, with `implemented_by:` in its `.ont.yml`.
  `unimplemented-class` then **gates** when the tree defines no `struct` or `enum` of that
  name: the class stated a fact about `crates/`, and a missing type contradicts it rather than
  merely omitting something. **A class that omits the field is not checked at all.** Name the
  Rust type as Rust spells it rather than expecting the class name to be derived —
  `HTTPServer` and `HttpServer` are two types and one kebab-case name.
  [why](directories.evidence.md#implemented-by)
- Every node must have at least one outgoing edge. Orphan nodes do not belong in the corpus.
- If a concept is uncertain or under investigation, mark it: prefix the title with `?` or open a
  branch. Uncertainty is valid; unlabeled speculation is not.

**Node kinds — authored vs. generated:**

*Authored nodes* are written through deliberate knowledge work — by a human or agent reasoning
about the domain. They are stable, permanent, and not regenerable from any source.

*Generated nodes* are produced by a pipeline from a primary source — extracted, computed, or
assembled automatically. They are regenerable if the pipeline is re-run against the same source.

Both are committed permanently. But their commit semantics differ — generated node commits are
**operational events**; authored node commits are **epistemic events**. Do not mix the two
kinds in a single commit; the log must remain readable as a knowledge record.

---

## `.yidam/tonpa.toml` and `.yidam/tonpa/`

The corpora this one depends on, and where they land. Absent in a repository that draws on no
other corpus, which is the ordinary state.

`.yidam/tonpa.toml` is the declaration — one entry per dependency, naming it and where it comes
from. `.yidam/tonpa/` is where the fetched ones are unpacked, one directory per package, each
holding the `bundle.yiz` it came from alongside the corpus extracted out of it.
`.yidam/tonpa/tonpa.lock` pins what was resolved, and **is committed**: it is the record of
which bytes this repository read.

**Two kinds of dependency, and the difference is only where the corpus is read from:**

- **Fetched** — a `.yiz` bundle downloaded, hashed against the lock, and unpacked into
  `.yidam/tonpa/<name>/`. Reproducible, pinned, and stale by construction: it is whatever was
  published, until someone updates it.
- **Path** — a sibling repository read where it sits. Not fetched, not hashed, not locked,
  because hashing a working tree that changes under you records nothing. This is the only form
  that supports a development loop: an edit in the producer is visible in the consumer without
  cutting a release.

```
mise run tonpa-install    # fetch what tonpa.toml declares; re-runnable
```

It is safe to re-run: anything already unpacked is verified against `tonpa.lock` and left
alone. **A green `mise install` is not proof the corpora arrived** — mise logs a failing
postinstall hook as a warning and exits 0 regardless, so run `mise run tonpa-install` on its
own and read its exit code. [why](directories.evidence.md#tonpa-install-exits-quietly)

**Reading a dependency needs no network, and the light build can do it.** The `tonpa` feature
buys resolving, fetching and locking. `query --across`, `neighbors` and `get_node` read a
corpus already on disk, and a path dependency is never on disk under `.yidam/tonpa/` at all —
its declaration is the only record it exists.

**A `cites:` into a dependency is a claim about text at a pin, and cannot be checked by
reading.** The package may be installed at a different pin than the one you are about to write,
and a `span:` is a claim about text no read-tool checks. `yidam lint` decides it; the
`citing-a-dependency` skill and the MCP `check_citation` tool answer it before you write it.

**Whether `.yidam/tonpa/` should be committed is not settled here.** The gitignore template
does not name it, and no derived repository has yet declared a dependency.
[why](directories.evidence.md#tonpa-ignore-unsettled)

---

## `.yidam/decisions/`

Structured records of choices made during this repository's life — from the genesis bootstrap
onward.

**What belongs here:** One YAML file per decision. A decision is any choice that shaped the
repository's structure, ontology, or direction — confirmed ontology sketches, approved implied
edges, connector and calculator selections, governance resolutions.

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
sangha for subsequent choices. Decision files are permanent records — they are not updated when
a decision is superseded, but a new decision may reference a prior one by `id`.

---

## `.yidam/sangha/` (collective governance only)

Present only in repositories bootstrapped as `governance: collective`. A single-elector
repository does not have this directory and does not need it — see
[CONSTITUTION.md](../CONSTITUTION.md) for what it would govern, and adopt it by scaffolding
this directory if a second elector ever appears.

The collective resolution protocol. Encodes how multiple participants (agents and humans)
maintain individual positions and synthesize them into shared understanding.

**What belongs here:** Protocol documents and the record of governance — not domain knowledge.
`PROTOCOL.md` (resolution algorithm), `electors.md` (recognized participants), `positions/`
(what each elector argued, per question), `resolutions/` (records of past resolution events).
Knowledge lives in the corpus; sangha is the governance layer above it.

**Ref store:** Sangha's live state is in git refs, not in files. `refs/heads/ma/<elector>`
tracks each participant's working position; `refs/heads/rigpa/<evolution>` records settled
collective evolutions. See [GRAPH.md](../GRAPH.md) for the full encoding model.

**The refs hold the corpus, not the argument.** A position is a branch and a branch is a ref —
which is right about which nodes an elector holds and wrong about why they hold them, and a
resolution turns on the why. `positions/<elector>-<question>.md` is where the argument is
durable. [why](directories.evidence.md#refs-hold-the-corpus-not-the-argument)

**A position is authored on a branch and lives on the baseline.** It is written on the elector's
own `ma/*` branch and then carried here by a `transport:` commit, verbatim. Authorship and
residence are separate questions. Files under `positions/` are therefore expected to arrive by
transport rather than to be authored in place — and never to be edited by anyone but the
elector whose name they carry.
[why](directories.evidence.md#authorship-and-residence-are-separate)

---

## `.yidam/private-paths` (optional)

Paths whose content must not sit in a public repository. One per line; `#` comments and blank
lines ignored. Absent, nothing is declared private and the CI job that reads it passes
immediately.

```
# Worked lines of attack and material the publication gate computes as [open].
dossier/
```

The CI job fails when the repository is public and any listed path holds a file other than its
`README.md` or `.gitkeep`. It reads `github.event.repository.private` from the event payload the
runner already has — no API call, so CI stays hermetic.

**Why a file rather than a convention.** An assumption about access control that looks enforced
and is not is worse than one everybody knows is manual, because nobody checks the second kind
by hand. Declaring the paths is what turns the assumption into a gate.
[why](directories.evidence.md#private-paths-why-a-file)

### Derived artifacts inherit the privacy of what they were derived from

`.yidam/index/` and `.yidam/embeddings/` are a re-encoding of the corpus: each indexed row
carries the node's own text, composed from `.yidam/corpus/` **and** `.yidam/catalog/`. A bundle
carries the index inside it.

So a path declared private that overlaps either directory makes the index private too, and
`yidam vault push --index` refuses on exactly that ground, naming the path — the same rule
`sadhana/github/workflows/release.yml` applies to a bundle, extended to the channel a vault
opens. [why](directories.evidence.md#derived-artifacts-inherit-privacy)

A declared directory holding only a `README.md` or a `.gitkeep` is a statement of intent rather
than material, and does not refuse.

### What this does not cover

**This is access control over material at rest. It says nothing about data leaving at runtime.**
Every question it answers is about somebody arriving: can a stranger reach a file that is
committed here. So the channels below are named rather than gated, each the reader's
responsibility and not this file's. [why](directories.evidence.md#egress-is-not-covered)

- **Connectors** send whatever they query to the source. A search term, an identifier, or a
  person's name in a request URL discloses that you are asking about it.
- **A deployed web shell** sends every request its reader makes to whoever hosts it, and its
  logs are a record of what was searched and what was opened.
- **A hosted encoder or model API** receives the text it is asked to embed or complete — queries
  at minimum, and the corpus itself if embedding is done remotely.
- **Anything with telemetry**, including tools run against the repository.

Apply the same rule the private-paths gate applies to access: **unknown is not proof of
protected.** A channel nobody has examined is not a channel known to be safe, and a comment
asserting that an exposure does not exist is worse than no comment, because it stops the next
reader from looking. Where a channel matters, examine it and record what you found in
`.yidam/decisions/` — including, honestly, what remains unobtained.

---

## `.yidam/policy/` (optional)

The rules this repository writes about itself, as Rego. Absent in most repositories: the `yidam`
binary carries a complete default policy, and this directory exists only to supersede part of
it.

**What belongs here:** `*.rego` files declaring a package the binary already defines — today
`yidam.disclose.at_rest`, `yidam.disclose.record`, `yidam.disclose.derived` — and `*_test.rego`
files holding this repository's own cases. `yidam policy check` lists the decisions and where
each came from.

**Why a file rather than a config field.** A rule compiled into the binary is one repository's
judgement applied to every other, and a quorum threshold or a corpus's own reading of what may
leave cannot be a field somebody ships.

**The local rule decides.** A file here supersedes the default for its package outright,
including by permitting what the default refused.
[why](directories.evidence.md#policy-local-rule-decides)

Which is why nothing here is quiet. `yidam policy check` names every override and the file it
came from; `yidam policy test` runs the *inherited* cases against your rule and reports which
expectations it no longer meets; `yidam lint` reports each override as `policy-override` at info
severity, so it reaches the editor; and `yidam doctor` counts them and fails outright on a rule
that does not compile. None of those gates — the repository decided. An override is a decision;
record why in `.yidam/decisions/`. [why](directories.evidence.md#policy-nothing-is-quiet)

**Not the vendored copy.** The default policy is readable at `.yidam/.vendor/prelude/policy/`,
which is read-only and re-vendored like the rest of the prelude. Read it there; write here.

---

## `.yidam/bin/`

The `yidam` binary this repository runs, installed by `mise run yidam-build` from the commit
`.yidam.toml` pins. Git-ignored — it is build output, and it is rebuilt by the same command that
updates the pin.

**Beside the pin, and not in `~/.cargo/bin`.** `cargo install` with no `--root` writes to one
location per *machine*, while the pinned commit is one per *repository*. On a machine with two
yidam repositories that is last-writer-wins, and the guarantee the build task exists to keep —
that the binary and the prelude agree — held for exactly one repository at a time.
[why](directories.evidence.md#bin-beside-the-pin)

**And through invocation, which needs no mis-installation at all.** A `yidam` left in
`~/.cargo/bin` by any earlier install shadows `.yidam/bin/yidam` for any process whose `PATH`
puts cargo's directory first. [why](directories.evidence.md#bin-invocation-shadowing)

Your `mise.toml` puts `.yidam/bin` first on `PATH` for anything mise runs, so the shell and the
editor resolve the same binary:

```toml
[env]
_.path = [".yidam/bin"]
```

**In `mise.toml`, not `mise.yidam.toml`.** The inherited layer is a mise *task file*, where
`[env]` declares a task named `env` and `_.path` is an unknown field — which orphans every task
in the file. [why](directories.evidence.md#bin-in-mise-toml-not-yidam)

**If your shell does not run mise, add it yourself** — a `yidam` from somewhere else will
otherwise answer for this repository. The VS Code extension checks `.yidam/bin/yidam` ahead of
`PATH` for the same reason.

That guards a human shell. It does not guard a script, a CI step, or an agent that assembles
`PATH` itself, and those are increasingly what runs these commands. **Wherever you build `PATH`
by hand, put `.yidam/bin` first** — the ordering is load-bearing, not a convenience.

**The failure is quiet in the way that matters.** So the binary now says so itself: when
`.yidam/bin/yidam` exists and is not the executable running, every command warns on stderr
naming both paths, and an unrecognized subcommand adds which binary refused it. Stderr, so a
`--format json` consumer reading stdout is unaffected.
[why](directories.evidence.md#bin-quiet-failure)

---

## `.yidam/capabilities.toml`, `.yidam/runs/` and `.yidam/computed/` (optional)

What a pipeline may do in this repository, and the record that it did it.

The commit vocabulary in [GRAPH.md](../GRAPH.md) names acts a *pipeline* performs rather than
acts a person performs — `extract`, `refresh`, `compute`, `reconcile`. A repository with no
manifest has none of them, which is the ordinary state: the agent invokes a calculator by hand
and a person writes the subject line afterwards. **That is the moment provenance is invented
rather than recorded**, and a manifest is how a repository says which of those acts it can
actually perform. [why](directories.evidence.md#capabilities-provenance-invented)

```toml
[capability.travel-tier]
kind   = "calculator"          # or `connector`
run    = ["sh", ".yidam/capabilities/travel-tier.sh"]
reads  = [".yidam/corpus/**", ".yidam/capabilities/**"]
writes = [".yidam/computed/**"]
verb   = "compute"

[capability.disclosure-envelope]
kind   = "calculator"
run    = ["sh", ".yidam/capabilities/disclosure-envelope.sh"]
reads  = [".yidam/computed/travel-tier.yml", ".yidam/capabilities/**"]
writes = [".yidam/computed/**"]
verb   = "compute"
after  = ["travel-tier"]       # optional
```

`yidam run travel-tier` then checks the declared `reads` out of `HEAD` into a scratch directory,
invokes the step there, and lands what it wrote — plus a receipt — as one commit. `yidam run`
with no step runs the whole manifest; `yidam run --dry-run` resolves the plan, reports what is
stale and why, and writes nothing.

**`reads` and `writes` are load-bearing rather than documentation.** The step is given exactly
what `reads` resolves to and nothing else from the repository, so it cannot depend on a file it
did not declare; and a step that writes outside `writes` is refused, with nothing committed.
[why](directories.evidence.md#reads-writes-load-bearing)

A consequence worth knowing before you meet it: because the step stands in that scratch tree and
nowhere else, **a capability must declare its own implementation**. Both entries above read
`.yidam/capabilities/**` for that reason, and reading the script there is what puts it in the
input state — so editing a calculator is what makes its step stale.
[why](directories.evidence.md#capability-declares-its-own-implementation)

### `after` — what must be up to date first

`after` names steps this one waits for. It is resolved rather than advisory: a run plans the
transitive closure, orders it so every dependency precedes what declares it, and invokes each
stale step against the commit the one before it landed. So `disclosure-envelope` above reads a
file that exists only because `travel-tier` committed it, and `yidam run disclosure-envelope`
against a repository where `travel-tier` has never run does the right thing.

A cycle is refused with the cycle named, and nothing runs. **A step may not come `after` one
that declares an epistemic verb.** [why](directories.evidence.md#after-epistemic)

### `ageing_days` — when an unchanged input state is not enough

A step is **stale** when what it reads, or what it declares, is not what its committed receipt
was computed from. That is an equality check rather than a guess, and for a calculator it is the
whole answer: an unchanged corpus computes an unchanged result, so a fresh step is skipped
rather than invoked.

It is not the whole answer for something that reads a world this repository does not hold.
`ageing_days = 30` says *re-run this if it has not run in thirty days, however little moved*, and
the age is measured from the commit that last carried the receipt.
[why](directories.evidence.md#ageing-days)

**Declare the interval; never expect one.** There is no default and no interval compiled into
the binary. A capability with no `ageing_days` is decided by its input state alone, which is
right for anything that is a function of what it reads.

**The verb decides where a run's commit goes, and nothing else does.** An operational verb —
`compute`, `extract`, `refresh`, `reconcile` — advances the branch you invoked the run from. An
epistemic verb — `establish`, `revise`, `resolve` — lands on `propose/<head>` instead, and your
branch does not move. So a run *can* open a question about a number, and the proposal branch is
where to look for what it produced: merge it to accept it, delete it to reject it. Nothing merges
itself.

**There is no path, and no config value, by which a run advances the current branch with an
epistemic commit.** [why](directories.evidence.md#epistemic-runs-have-no-escape-hatch)

**Credentials are named, never carried.** A capability declares which secret it needs by name;
the value arrives from the environment. A committed file is not a place for a secret — the same
rule `.yidam/vault` states for bytes.

### `.yidam/runs/`

One receipt per capability, at `.yidam/runs/<step>.yml`, committed in the same commit as the
output it describes. It records the commit the inputs came from, a digest of the manifest and
the config that governed the run, every input file by digest, and every output by digest. The
series is the git history of that file, which is where a repository's series of anything already
lives.

**A receipt carries no timestamp.** The commit it lands in has a committer date, which is the
real one — and it is what an `ageing_days` interval is measured against.
[why](directories.evidence.md#receipt-has-no-timestamp)

A step re-run under an ageing rule whose answer had not moved is the one case that commits
without changing an output: the receipt names a new input commit, and that commit is the record
that somebody looked. [why](directories.evidence.md#receipt-records-a-look)

**Where the output goes is your decision and it is a real one.** Writing a derived figure onto a
node asserts, silently and for every instance, that the figure is a measurement. Prefer a
directory of computed artifacts that carry their method, and record the choice in
`.yidam/decisions/`. [why](directories.evidence.md#computed-output-placement)

### `.yidam/computed/`

What this repository worked out about itself — one file per calculator result. Committed, unlike
`.yidam/index/`, because a computed quantity is an assertion this repository is making and the
commit that landed it is the record of what it was computed from.

**A file here is read as a signal table when it says it is one.** That means a top-level
`format_version: 1` and a `signals:` list whose every row names a node:

```yaml
format_version: 1
method:
  rule: |
    A derived assertion travels only as far as the weakest claim beneath it.
signals:
  - node: gage/canyon-outlet
    travels_as: open
    downgraded: true
```

Every other key in a row is a **signal** about that node, and `yidam embed` attaches it to that
node's embedding record — which is what turns a computed answer into something a search can
filter on rather than a file somebody has to open. A file carrying no `format_version` is listed
and not read, so a calculator whose output is a report rather than a table stays legal and stays
visible. [why](directories.evidence.md#computed-declares-its-own-readability)

**A row is keyed in the reference grammar and in nothing else.** `gage/canyon-outlet`,
`node/gage/canyon-outlet`, or the absolute `yidam://<corpus>/node/<path>` form naming this
corpus — the grammar every SDK's `parse_reference` already implements. A revision pin
(`gage/canyon-outlet@abc1234`) is refused rather than ignored: a signal attached at a past commit
is not a signal about the node as it stands.
[why](directories.evidence.md#computed-keyed-by-the-reference-grammar)

**A signal name is repository-wide, and a collision is refused rather than resolved.** Two
calculators both emitting `tier` is one name meaning two things, and a reader that picked a
winner would make the other silently absent. `yidam doctor` reports the collision and names both
files. [why](directories.evidence.md#computed-signal-names-are-repository-wide)

**Nothing creates this directory.** A repository declaring no calculator has nothing to put in
it, and a run is the only thing that writes here. `yidam doctor` asks what is in it and whether
it still stands — a computed file whose inputs have moved, or whose bytes are not the ones its
receipt recorded, is a stale answer that reads exactly like a current one.

---

## `.yidam/authorship.yml` (optional)

What in this repository is not authored here.

Checks that read prose walk directories, and a walk cannot tell authored material from material
that merely landed there. A defect in an inherited or generated region is not this repository's
to fix, and reporting one as though it were hands you a finding you cannot act on.
[why](directories.evidence.md#authorship-why)

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
declared without one does not parse — a claim with no addressee is a request for silence wearing
a provenance label. [why](directories.evidence.md#authorship-requires-an-addressee)

| Kind | What it asserts | What the gate does |
|---|---|---|
| `generated` | Written by this repository's own tooling. | Reported at info severity, addressed to the generator. Fix the generator — the file is rewritten by the next build, so fixing the file does not persist. |
| `imported` | Copied from elsewhere and not modified. | Reported at info severity, addressed upstream. |
| `excluded` | Neither. | Not read at all. |

Only `excluded` produces silence, and it is named so that a reviewer meeting it in the manifest
sees the escape hatch as one. The other two are still real defects — simply somebody else's.

`.yidam/.vendor/` is built in as `imported` and is not declared here: it is yidam's claim about
a directory yidam manages, not this repository's.

**A declaration that matches nothing is reported** — `authorship-region-stale` — because the
entry that outlives its directory quietly excuses a path somebody later creates under the same
name. `generated` regions are exempt: they are written by a build and are frequently git-ignored,
so absence on a fresh clone carries no information. The check is Warn rather than Error.
[why](directories.evidence.md#authorship-region-stale)

---

## `.yidam/skills/`

Reusable capabilities available to agents in this repository.

**What belongs here:** Domain-specific skills — structured procedures agents can invoke when
working in this repo. Generic skills inherited from yidam live in `.yidam/.vendor/prelude/`;
skills that require knowledge of this domain's corpus or toolkit live here.

---

## `.yidam/.vendor/`

The inherited yidam prelude, moved here by the `vendor:` commit during bootstrap.

**What belongs here:** `prelude/` and nothing else. The vendor step moves `yidam/prelude/` to
`.yidam/.vendor/prelude/` and deletes the rest of the template.

**What deliberately does not belong here:** yidam's CLI source, its bootstrap test harness, its
design notes, and its docs site. None of them are readable, runnable, or updatable from inside a
derived repo. [why](directories.evidence.md#vendor-excludes-cli-and-harness)

**Read-only.** Do not modify anything under `.yidam/.vendor/` in the course of domain work. An
edit here is silently discarded the next time the prelude is re-vendored, and until then it is a
local divergence nobody can see. A defect in the prelude is fixed upstream in yidam and adopted
by re-vendoring — which is why `yidam lint` reports a broken link here at info severity rather
than as a violation of this repository. See `.yidam/authorship.yml` above, of which this
directory is the built-in instance.

**Note:** Paths to inherited skills and agents use `.yidam/.vendor/prelude/` — for example, the
bootstrap skill lives at `.yidam/.vendor/prelude/skills/bootstrap.md` after genesis.

---

## `.yidam.toml` (repository root)

The provenance pin: which yidam this repository was derived from. Written by `yidam clone` or
`yidam overlay`, confirmed by the bootstrap vendor step, and updated by
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
nothing. `committed` is the *upstream* commit's date, not the date this repo last ran the vendor
step — it answers how old the prelude is, which is what staleness turns on. See
[VERSIONING.md](https://github.com/goedelsoup/yidam/blob/main/VERSIONING.md) for the three
release layers.

**Re-vendoring.** The prelude is not frozen at the repository's birth. Corrections made upstream
reach a derived repo when it re-vendors:

```
mise run yidam-vendor-status    # what you are pinned to, and what is newer
mise run yidam-vendor-update    # fetch, replace prelude/ and mise.yidam.toml, re-pin .yidam.toml
```

The update replaces `.yidam/.vendor/prelude/` wholesale, rewrites `.yidam.toml`, and replaces
`mise.yidam.toml`. It touches nothing else — `corpus/`, `catalog/`, `decisions/`, `skills/`,
`crates/`, and every other top-level file are domain-owned and are never overwritten by an
update.

**`prelude/domains/` is the one part of that replacement that is not wholesale**, because its
contents are a decision rather than a copy. The update reads `prelude_domains` from
`.yidam/decisions/proposals.yml` and prunes to it, and where no such declaration exists it keeps
exactly what this repository already vendored. Adding a name to that field and re-running the
update is how a domain is vendored later; it is also the only thing that survives the next one.
[why](directories.evidence.md#prelude-domains-not-wholesale)

`mise.yidam.toml` is on that list because it is inherited, not domain-owned: it is the task
layer, as much yidam's to correct as the prelude is, and it sits at the repo root only because
mise has to find it there. **Keep domain tasks in `mise.toml`; anything written into
`mise.yidam.toml` is replaced on the next update.**
[why](directories.evidence.md#mise-yidam-toml-is-inherited)

Review the resulting diff and commit it as its own event:

```
git commit -m "vendor: re-vendor prelude at <commit> — <what changed>"
```

Re-vendor deliberately, not reflexively. A prelude change can alter what the graph gate accepts;
adopting one is a decision worth its own commit and, if it changes conventions the corpus
depends on, its own record in `.yidam/decisions/`.

**Staleness is invisible from the inside, so it is reported from the outside.** The derived repo
CI prints how far behind the pin is on every run, and escalates to a warning when `GRAPH.md` has
moved. `yidam-vendor-update` prints which verbs were added or removed between the old pin and
the new one, for the same reason.

**Neither gates.** A repository is entitled to stay pinned.
[why](directories.evidence.md#staleness-reported-from-outside)

### Sending a finding back

Re-vendoring carries corrections **downstream**. Nothing carries findings **up**, and a derived
repository is where most defects in this template are actually discovered — it is the only place
the conventions meet a real corpus.

So: when domain work here runs into a defect in the prelude, the CI, the CLI, or these
conventions, **open an issue upstream** rather than working around it locally. A local
workaround is invisible to every other derived repository and is discarded at the next
re-vendor. [why](directories.evidence.md#sending-a-finding-back)

```
gh issue create --repo goedelsoup/yidam --label "from:derived-repo"
```

What makes such a report actionable, in rough order of value:

- **The file and line in the prelude or CLI**, not just the symptom.
- **What it cost here** — the commits spent, the checks that passed while wrong, the workaround
  now in place. This is the part upstream cannot reconstruct and the part that decides priority.
- **The pin you are on**, from `.yidam.toml`. A defect already fixed upstream is a re-vendor,
  not an issue.
- **Whether you worked around it**, and where, so the workaround can be removed when the fix
  lands.

Do not send corpus content. A finding is about the template; the domain material that exposed it
usually should not leave the repository, and often may not.

**Everything above is the shape of a defect** — a rule that is wrong everywhere, which one
repository is enough to demonstrate. A rule that is right in general and wrong *here*, because
it collides with a fact about this domain, is a different report asking for a different repair.
[upstream.md](upstream.md) has that form.

---

## `samudaya/` (transient — present only before and during bootstrap)

A transient bootstrap influence layer, consumed and committed away as part of the genesis event.
See [samudaya/README.md](https://github.com/goedelsoup/yidam/blob/main/samudaya/README.md) for
the full protocol — the directory itself is deleted at genesis, so a relative link to it is
broken in every derived repository.

**Presence after genesis is an error state.**

---

## `sadhana/` (transient — present only during bootstrap)

The scaffold template layer. Provides the initial content for each derived-repo directory.
Bootstrap reads these templates, creates the derived-repo structure from them, then deletes this
directory. Like samudaya, it does not survive genesis.

**Presence after genesis is an error state.**
