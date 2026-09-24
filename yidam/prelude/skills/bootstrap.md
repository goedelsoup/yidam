---
name: bootstrap
description: Initialize yidam in a repository — empty, near-empty, or an existing codebase
---

# Skill: bootstrap

Invoked when an agent enters a yidam repository with [BOOTSTRAP.md](https://github.com/goedelsoup/yidam/blob/main/BOOTSTRAP.md)
as its entry prompt. Produces a fully scaffolded, ontology-grounded, corpus-seeded repository
with a legible genesis commit. Works in two modes:

- **Empty-repo mode** — the repo has no commits. Steps proceed in full.
- **Existing-repo mode** — the repo has commits but `.yidam/` is absent. Yidam is applied
  as a knowledge-graph overlay on the existing structure. Step 1.5 explores what is already
  present before the ontology dialogue; steps 3, 6, and 8 adapt accordingly.

**How to read this file.** Every instruction below is one you can execute. The incident that
produced an instruction, the measurement that set its number, and the failure it was built
against are in [bootstrap.evidence.md](bootstrap.evidence.md) — one section per instruction,
reached by the `[why]` link beside it. Read an instruction's evidence when it seems wrong for
the repository in front of you, when you are about to deviate from it, or when you are
changing this skill. You do not need it in order to bootstrap, and it is not one of step 1's
seven files.

## Pre-flight

Before any step, run:

```
git log --oneline
ls .yidam/ 2>/dev/null && echo EXISTS
```

Determine the mode:

- **No commits** → empty-repo mode. Proceed to Step 0. The only file you should have read
  is `BOOTSTRAP.md`. Do not read any other file before completing Step 0.
- **Commits exist, `.yidam/` absent** → existing-repo mode. Proceed to Step 0, then follow
  the existing-repo variants noted in steps 1.5, 3, 6, and 8.
- **`.yidam/` present** → already bootstrapped. Do not re-run.

In either active mode, do not invoke any other skill, workflow, or tool until the genesis
commit is written in step 8. If the user's opening message contains domain context, hold it
as seed material for the ontology dialogue in step 2.

## Steps

### 0. Read samudaya (if present)

Before anything else, check whether `samudaya/` exists. If it does, list its contents first:

```
ls samudaya/
```

If only `README.md` and `examples/` are present, there are no seeds — skip to step 1.

If seeds are present and a `yidam` binary is already on `PATH` (`command -v yidam`), run:

```
yidam samudaya-audit
```

It validates each seed's frontmatter — the `kind:` value, a title, the
`constitutional: true|false` flag on augmentations — and prints a `[review]` line for each
constitutional augmentation; trust its flags over re-deriving them by hand. The binary is
usually not there yet — step 8.5 installs it — so when the command is absent, skip it and
read the files directly; do not install the CLI early just to run the audit.

Whether or not the audit ran, read every file present — excluding `samudaya/examples/` and `samudaya/README.md`,
which are not seeds. Note each seed file's `kind` frontmatter field.
- **`axiom`** files: treat these concepts as pre-committed — they must appear in the corpus.
  Hold them in working memory as required nodes going into the ontology-discovery dialogue.
- **`hint`** files: treat these as candidate relationships or directions to surface during
  discovery. They are not guaranteed — if the user's answers don't support them, discard.
- **`constraint`** files: enforce these as hard boundaries during scaffolding. Do not deviate
  from them without surfacing the constraint and asking explicitly.
- **`augmentation`** files: read the `constitutional:` frontmatter flag. `constitutional:
  true` marks a constitutional extension — a domain-specific article that adds to
  [CONSTITUTION.md](../CONSTITUTION.md) — and must be committed into the derived repo
  permanently: append it to the repo's copy of the constitution as part of the genesis
  scaffolding. `constitutional: false` marks a general guideline — additional prelude for
  this run only, gone once samudaya is consumed. A file missing the flag is malformed
  (`samudaya-audit` reports it as an issue); surface it to the user and ask, rather than
  classifying the content yourself.

Samudaya does not replace the dialogue. It seeds it.

### 1. Internalize the prelude

Read these seven files — and **only** these seven files, in this exact order, using their
exact paths. Do **not** run `ls`, `find`, or any directory enumeration of `yidam/prelude/`
at any point during bootstrapping. Do not read any other file in `yidam/prelude/` (including
`SCRIPTURE.md` or any file surfaced by enumeration). Do not read
`yidam/prelude/skills/bootstrap.md` — it is the skill you are currently executing, not a
file to internalize here.

1. `yidam/prelude/GLOSSARY.md` — the borrowed vocabulary the other six use without explaining
2. `yidam/prelude/IDENTITY.md` — what kind of knowledge artifact this repo is
3. `yidam/prelude/GRAPH.md` — the graph model: nodes, edges, commit types, branch semantics
4. `yidam/prelude/CONSTITUTION.md` — the governance rules that constrain what you may do
5. `yidam/prelude/PHASES.md` — the named phases of inquiry
6. `yidam/prelude/guidelines/agent-conduct.md` — specific conduct norms
7. `yidam/prelude/guidelines/directories.md` — where things live and what belongs in each

`GLOSSARY.md` is first and is deliberately the shortest: the six files below use its words
without defining them. [why](bootstrap.evidence.md#glossary-first)

Three kinds of read reach a named path under `yidam/prelude/` for a stated purpose, and they
are the only exceptions: **step 2** lists `yidam/prelude/kuten/` and reads two fields out of
the profile the user confirms; **step 5** lists `yidam/prelude/domains/` to see what a
calculator could call into; and a `[why]` link in this skill names one section of
`bootstrap.evidence.md`, which you may follow when an instruction seems wrong for the
repository in front of you and need never follow otherwise. Each names the path it reads and
the field it takes from it. Nothing else under `yidam/prelude/` is opened at any point.
[why](bootstrap.evidence.md#named-reads-are-not-wandering)

`yidam/tests/` is deliberately absent from this list, and absent from the repository you are
working in. What you need in order to do the work well is in these seven files and in the
steps below. [why](bootstrap.evidence.md#tests-are-not-curriculum)

After reading all seven, output the synthesis as a **standalone message** — do not append
questions or any other content to it. Wait for the user to acknowledge before opening the
Step 2 dialogue. This gives the user the opportunity to correct any misread before questions
begin.

> **Prelude internalized.** Graph model: [one sentence]. Key constraints I'll honor: [two or
> three bullet points from CONSTITUTION and agent-conduct]. Directory layout: [one sentence].

### 1.5. Explore the existing repository — existing-repo mode only

Skip this step in empty-repo mode.

Before opening the ontology dialogue, read the existing repository to ground the class sketch
in what is actually present. The goal is a one-paragraph inventory, not a full audit.

1. `ls -la` — note top-level directories. Flag which ones match sadhana's expected layout
   (`agents/`, `crates/`, `docs/`, `packages/`, `web/`) — step 3 will skip creating those.
2. Read `README.md` if present. Extract the domain framing in one sentence.
3. Identify the primary artifact type and read enough to understand the structure:
   - **Code repo**: find `Cargo.toml`, `go.mod`, `package.json`, or similar; skim one or two
     key source files to understand what the repo does.
   - **Data / research repo**: look for notebooks, dataset directories, experiment configs.
   - **Documentation / knowledge repo**: scan the docs structure and any existing index files.
4. Note any existing `sadhana/`, `samudaya/`, or `yidam/` directories — if absent, the
   transient-layer consume steps in step 8 do not apply.

Output a one-paragraph inventory before opening the step 2 dialogue. The ontology classes
should reflect the existing artifact structure where natural — a Rust repo might yield `Crate`,
`Module`, and `Trait`; a data repo might yield `Dataset`, `Experiment`, and `Model`.

### 2. Discover the ontology

Do not scaffold anything yet. Instead, open a dialogue to discover the core ontology of this
repository. This is an iterative loop — ask, receive, refine — until you have a stable sketch.

Begin with orienting questions:

- What is the domain or subject of this repository?
- What is the central question or problem it exists to investigate?
- What are the first 3–5 concepts that feel irreducible to this domain?

For each concept surfaced, probe further:
- What does this relate to? What does it depend on? What does it oppose?
- Is it atomic, or does it decompose?
- What would a node for this concept need to say?

Continue until you can draw a coherent sketch of the initial graph: a small set of named
nodes and the edges between them. Confirm this sketch with the user before proceeding.

Present the sketch in this format:

**Nodes**

| Node | What it is |
|------|------------|
| `name` | one-line description |

**Edges**

```
source →[relationship]→ target
```

One row per node; one line per edge. No prose — the format is the signal that the sketch is
ready to confirm.

After the user confirms the sketch, present the foundational ontology alignment choice before
writing any files. Describe each option concretely using 2–3 nodes from the confirmed sketch
as examples:

**BFO (Basic Formal Ontology)** — organizes entities along one axis: do they *persist through
time* or *unfold through time*?  Things that exist at a moment and have no temporal parts are
**continuants** (material entities, qualities, dispositions, sites). Things that happen over
an interval and have temporal parts are **occurrents** (processes, events, process boundaries).
Each class gets a `foundational_type:` field in its `.ont.yml`, with `ontology: bfo`. Best fit
for scientific, empirical, and
physical-process domains where the object/event distinction carries analytical weight (e.g.,
distinguishing a machine from the machining process it performs).

**UFO (Unified Foundational Ontology)** — organizes entities around rigidity and relationality.
A **Kind** is what something necessarily is (if it stops being one it ceases to exist as that
thing). A **Role** is what something contingently plays in a relational context (the same entity
may play different roles in different relationships). A **Relator** is a first-class node that
mediates a relationship with its own identity and properties — rather than a bare edge, a
relator carries the history and terms of the connection. Each class gets a `foundational_type:`
field, with `ontology: ufo`.
Best fit for institutional, enterprise, and process-modeling domains where the same entity plays
different roles and where relationships themselves carry meaning worth querying.

**None** — no foundational alignment. Classes are typed by domain convention only. Choose this
if foundational ontology alignment is not a goal of the corpus, or if you want to commit later.

Ask the user to choose one. Then compute a default seed count from the confirmed sketch:
**two instances per class, with a floor of 13** — count the rows in the Nodes table
confirmed above. State the computed number, not a fixed one, when you ask:

> **How many seed instances should the initial corpus contain?** [default: `<2 × class
> count, minimum 13>`]

The user may give a different number or press enter to accept the computed default. Record
it as `corpus_depth` in the decision record — step 6 distributes instances across classes to
reach this target. [why](bootstrap.evidence.md#seed-count-default)

Finally, ask the governance question — by name, not by count:

> **Name the people or independently-directed agents who will maintain this repository.**
>
> If that's just you, with agents acting on your behalf, name yourself and stop — you are
> the sole elector. Phases run on `phase/<name>` branches off the baseline. This is the
> common case.
>
> If you can name a second elector — someone who will actually hold and reconcile a
> position that is expected to diverge from yours — name them too. This activates the
> sangha: each named elector keeps a `ma/<elector>` branch, and resolution events synthesize
> them into `rigpa/<evolution>` baselines under the constitution.

A person who cannot name a second elector has answered the question: record
`single-elector` and move on. Record the answer as `governance: single-elector |
collective`, with the named electors listed under `electors:`. Do not choose `collective`
because it sounds more capable. If only one name comes up, take `single-elector`; the sangha
can be adopted later by scaffolding `.yidam/sangha/` when a second elector actually appears
and can be named. [why](bootstrap.evidence.md#single-elector-is-an-answer)

Then write the ontology decision record, including the chosen alignment, corpus depth, and
governance mode, before proceeding to step 3:

```
.yidam/decisions/ontology.yml
```

```yaml
id: ontology
summary: <one line — the domain, class count, and chosen foundational alignment>
corpus_depth: 26              # target instance count; default is 2 per class, min 13; user-configurable
governance: single-elector    # single-elector | collective
electors: [<name>]            # the electors named when the governance question was asked
context: |
  <what the ontology discovery dialogue surfaced; key choices made; examples used to explain
  the alignment options>
decision: |
  <the confirmed class list and edges; the chosen foundational ontology (bfo | ufo | none);
  the governance mode and what the user said about who maintains this>
rationale: |
  <why these classes; what was considered and discarded; why this alignment was chosen>
```

**One confirmation remains, and it is the only one in this step that is not about the
domain.** Everything above says what this repository is *about*. A **kuten** says what its
work is *aimed at* — the phase types it runs, the shape of corpus it accretes, the share of
its commits that settle something. It narrows the loop and may not widen the model, and it
binds nobody: divergence from it is a question for a person, not a defect.

List the profiles this template ships and read the one you are about to name:

```
ls yidam/prelude/kuten/
```

Each profile directory holds `kuten.yml`, the declaration a tool reads, and `KUTEN.md`, the
document a person reads. Take the profile's `gloss:` and its `revision:` from `kuten.yml`
directly — quote the gloss rather than paraphrasing it, and copy the revision rather than
typing a number you remember.

**With one profile present this is a confirmation, not a menu.** State it and take the answer:

> **This corpus's practice — its kuten — is `<name>`.** <the profile's `gloss:`, verbatim.>
> It is vendored at genesis and recorded with the revision that was vendored, and it changes
> afterwards only by a `decide:` commit carrying a superseding record. Confirm it, or say
> that no profile here describes what this corpus is for — **holding no kuten is a supported
> state**, and `yidam kuten check` reports it as one and exits zero.

If the listing turns up more than one profile, give each its own line — name and gloss, in
the profile's own words — and ask which. Do not rank them and do not recommend one.
[why](bootstrap.evidence.md#do-not-rank-profiles)

If the directory is absent or the listing is empty — an existing repository that was overlaid
without the template tree — there is no profile to confirm and nothing to vendor. Say so, ask
nothing, and write no record.

Then write the kuten decision record. If the user declined a kuten, write no file and say so
in step 9 — an absent record is the state `doctor` and `yidam kuten` both already report, and
a record naming a profile the user did not adopt is worse than none.

```
.yidam/decisions/kuten.yml
```

```yaml
id: kuten
summary: <one line — the profile adopted, and the revision vendored with it>
kuten: inquiry                # the profile confirmed above, by its directory name
revision: 2                   # that profile's own `revision:`, copied — not typed from memory
decision: |
  <the profile adopted, and what its gloss says the practice is aimed at>
rationale: |
  <what the user said when it was stated; if the listing held another profile and it was
  declined, name it and say why>
```

`kuten:` and `revision:` are the two fields a tool reads; the rest is for a person and for
`yidam decisions-log`. [why](bootstrap.evidence.md#revision-is-copied)

### 3. Orient to and scaffold the derived-repo structure

The sadhana directory (`sadhana/`) holds the template content for this derived repo. In this
step, read the templates and create the derived-repo directory structure from them.

**First, read the sadhana templates:**

```
ls sadhana/
```

Then read each template file in `sadhana/`:

- `sadhana/catalog/README.md`
- `sadhana/corpus/README.md`
- `sadhana/crates/README.md`
- `sadhana/skills/README.md`
- `sadhana/web/README.md`
- `sadhana/root/README.md`, `sadhana/root/AGENTS.md`, `sadhana/root/CLAUDE.md`, `sadhana/root/mise.toml`,
  `sadhana/root/gitattributes`, `sadhana/root/gitignore`, `sadhana/root/PRACTICE.md`
- every file in `sadhana/github/workflows/` — run `ls sadhana/github/workflows/` and read
  each one; do not assume a fixed list (#589)
- `sadhana/sangha/README.md` (and PROTOCOL.md, electors.md, resolutions/, positions/) —
  **only if `governance: collective`**; skip these five reads entirely in single-elector mode

`sadhana/agents/`, `sadhana/packages/`, and `sadhana/docs/` are deliberately not read here.
They are templates for directories created on first use, not at genesis — see below.

**Then create the derived-repo structure:**

Top-level directories (created directly from sadhana templates):
```
crates/README.md
web/README.md
```

`.yidam/` directories (created from sadhana templates):
```
.yidam/catalog/README.md
.yidam/corpus/README.md
.yidam/decisions/          ← new, empty; written to in steps 2 and 5
.yidam/skills/README.md
```

**Create on first use, not now:** `agents/`, `packages/`, and `docs/`. Their sadhana
templates exist and are the right content — but scaffold them the day something goes in
them, not at genesis. Note them in step 9 instead, so the user knows they exist as
conventions. The `yidam` CLI treats all three as optional — `agents-index` and
`packages-index` are no-ops when the directory is absent.
[why](bootstrap.evidence.md#create-on-first-use)

**`.yidam/sangha/` — only if `governance: collective`.** Read the governance mode recorded
in `.yidam/decisions/ontology.yml` in step 2:

- **`single-elector`** — do not create `.yidam/sangha/`. Do not copy `sadhana/sangha/`.
  The constitution is vendored with the rest of the prelude and lies dormant; it governs
  resolution events, and there will be none.
- **`collective`** — create `.yidam/sangha/` with all files from `sadhana/sangha/`, and fill
  `electors.md` with the participants the user named.

Repository-root files. `sadhana/root/` is not a directory mirror — each file installs to a
specific path, **overwriting yidam's own copy**:

```
sadhana/root/README.md            → README.md            (overwrites yidam's)
sadhana/root/AGENTS.md            → AGENTS.md            (overwrites yidam's)
sadhana/root/CLAUDE.md            → .claude/CLAUDE.md    (overwrites yidam's)
sadhana/root/mise.toml            → mise.toml            (overwrites yidam's)
sadhana/root/gitattributes        → .gitattributes       (overwrites yidam's)
sadhana/root/gitignore            → .gitignore           (overwrites yidam's)
sadhana/root/PRACTICE.md          → PRACTICE.md          (yidam keeps no copy)
```

Overwrite all six now. Do not merge yidam's content into them. `PRACTICE.md` installs new
rather than overwriting; step 8.5's `yidam regen` fills its block.
[why](bootstrap.evidence.md#overwrite-do-not-merge)

**`.github/workflows/` — replace the directory, do not overwrite files inside it.** In
existing-repo mode the target's own workflows are there and are not this scaffold's; in
template mode `yidam clone` now excludes `.github/` outright, so the directory arrives
empty. Either way the instruction is the same one, and it is a replacement rather than a
merge. [why](bootstrap.evidence.md#replace-the-workflows-directory)

Delete the directory entirely and replace it wholesale:

```
rm -rf .github/workflows/
mkdir -p .github/workflows/
cp sadhana/github/workflows/*.yml .github/workflows/
```

**Enumerate `sadhana/github/workflows/`; do not name its files in prose.** `ls
sadhana/github/workflows/` is the source of truth for what belongs at genesis. Whatever the
directory holds when this step runs is what the derived repository gets, in full, and
nothing of yidam's own remains beside it. [why](bootstrap.evidence.md#enumerate-the-workflows)

`gitattributes` and `gitignore` are spelled without their dots, and install with them.
[why](bootstrap.evidence.md#dotless-template-names)

`.gitignore` is the one of the six overwrites most easily mistaken for generic, and it is not:
overwrite it too. [why](bootstrap.evidence.md#gitignore-is-not-generic)

Each README may contain a `<!-- TEMPLATE -->` comment block marking fields that need
domain-specific content. Fill every such block now, before proceeding. These are the only
edits made to the scaffolded content in this step — do not add or remove files beyond what
sadhana provides.

**Existing-repo mode**: if `sadhana/` is absent, skip the template reads and directory
creation for top-level dirs — they either already exist or are not applicable to this repo.
Create only the `.yidam/` subdirectories that are missing. Do not overwrite any existing
file; if a target path already exists with content, leave it and note the conflict.

### 4. Formalize the ontology

Render each node from the confirmed sketch as a domain class definition in `.yidam/corpus/`:

```
.yidam/corpus/<domain-class>.ont.yml
```

Each file defines what that class of thing is — its properties and its edge participation.
One file per class; the filename matches the class name exactly.

One class per file, and one *concept* per class. If a class name needs an "and" or an "or",
it is two classes. [why](bootstrap.evidence.md#one-concept-per-class)

```yaml
class: <name>
label: <Human-Readable Label>
foundational_type:           # omit this field entirely if alignment is "none"
  ontology: bfo | ufo
  type: <value>              # BFO: continuant | occurrent | quality | disposition | role | ...
                             # UFO: kind | subkind | role | phase | relator | mode | quality | event | situation
  iri: <url>                 # optional — the IRI that type has in that ontology, e.g.
                             #   BFO: http://purl.obolibrary.org/obo/BFO_0000002
                             #   UFO: https://purl.org/nemo/gufo#Relator
                             # `export-rdf` emits it as skos:exactMatch. Omit it if you have
                             # not looked it up; the alignment still exports without it.
description: |
  <one sentence — what this class of thing is and why it is irreducible>
properties:
  - name: <field>
    type: string | date | ref | text
    description: <one line>
edges:
  - relationship: <verb phrase>
    target: <class name>
    direction: out | in
    description: <one line>
```

These files are the schema layer of the corpus. They define what kinds of things exist, not
specific instances. Every class in the confirmed sketch gets a file. Do not add classes not
in the confirmed sketch.

### 5. Identify implied edges, connectors, and calculators

Definitions for this step — three distinct concepts:

- **Implied edge** — an edge type between two classes that is warranted by the domain but
  not yet declared in any `.ont.yml` file. An implied edge is a *proposal*, not an edge.
  It becomes a `links:` entry in instance files only after the user approves it and it is
  wired in step 7. Do not add anything to any file in this step.
- **Connector** — a retrieval bridge to an external data source (see `directories.md`:
  `crates/` holds connectors). A connector is identified here as a *proposal*. Approved
  connectors are invoked opportunistically during seeding (step 6) when demand is clear;
  any that were not invoked during seeding are stubbed in step 7. Do not create any files
  in this step.
- **Calculator** — a domain computation that derives a value or relationship from corpus
  data. A calculator is identified here as a *proposal*. Approved calculators are run
  during step 7 if enough seeded instances exist to make the result meaningful; otherwise
  they are stubbed. Do not create any files in this step.

Before seeding any objects, read the full set of `.ont.yml` class definitions and reason
about what the schema implies at the domain level. Then present a structured report to the
user for confirmation:

**Implied edges** — edge types warranted by the domain but absent from the current `.ont.yml` files:

| From | Relationship | To | Basis |
|------|--------------|----|-------|
| `class` | verb phrase | `class` | one line — why this edge is implied |

**Connectors** — external data sources that could feed this corpus, and the crate adapter each would require:

| Name | Source | Feeds | Notes |
|------|--------|-------|-------|
| `name` | external system or dataset | which classes | one line |

**Calculators** — domain computations that follow naturally from the class structure:

| Name | Computes | Reads | Returns | Prelude domain |
|------|----------|-------|---------|----------------|
| `name` | what it derives | which classes/edges | what it produces | `<domain>`, or — |

Before proposing a calculator, list `yidam/prelude/domains/` and see whether one of them
already computes what it needs. That layer holds small pure functions — means and variances,
centrality, entropy, geodesic distance — implemented identically in Rust, TypeScript and
Python and pinned to each other by shared fixtures. The last column names the domain a
calculator would draw on, or `—` if none fits.

This is the only point in the bootstrap where that layer is visible, and the selection has a
consequence in step 8: **only the domains named here are vendored.** A repository that names
none gets no `domains/` directory. Do not name a domain because it sounds adjacent. The
question is whether a calculator in this table would call a function in it.
[why](bootstrap.evidence.md#only-named-domains-are-vendored)

This step produces a report only. Do not modify any file. Wait for the user to confirm,
modify, or discard individual items. Only what the user approves is carried into step 7.

After the user confirms, write the proposals decision record:

```
.yidam/decisions/proposals.yml
```

```yaml
id: proposals
summary: <one line — what was approved>
context: |
  <the full set of proposals presented>
decision: |
  <what the user approved, modified, or discarded — item by item>
prelude_domains: []          # domains selected for vendoring in step 8; [] is the common case
rationale: |
  <any rationale provided; gaps or domain logic behind approvals; for each domain named,
  which calculator would call into it>
```

### 6. Seed corpus objects

Read `corpus_depth` from `.yidam/decisions/ontology.yml` (if absent, default to two
instances per class with a floor of 13, per step 2). This is
the target total instance count for the genesis corpus. Create a class directory for each
class, then distribute instances across classes to reach the target:

```
.yidam/corpus/<class>/README.md       — describes the class in prose; links to the .ont.yml
.yidam/corpus/<class>/ACTIONS.md      — operations, queries, and skills applicable to this class
.yidam/corpus/<class>/<instance>.yml  — a concrete object of this class
```

**`README.md`** — one paragraph describing what this class of thing is in the context of
this domain. Link to the class definition (`../<class>.ont.yml`). Written for a domain
contributor, not a schema reader.

**`ACTIONS.md`** — a list of operations meaningful for this class: queries that retrieve
instances, transitions an instance can undergo, skills that act on it, or calculators that
derive from it. Stub entries are fine; this file grows over time.

**`<instance>.yml`** — a concrete object. Structure:

```yaml
class: <class-name>
label: <Human-Readable Instance Name>
description: |
  <one or more sentences — what this specific thing is>
properties:
  <field>: <value>
links:
  - target: ../<other-class>/<other-instance>.yml
    relationship: <verb phrase>
  - target: ../<class>.ont.yml
    relationship: instance-of
```

Each instance must carry at least one outgoing link to another node. Be specific enough to
be wrong — a vague placeholder is not an object.

An edge is a claim that two things are related, and the `relationship` says how. The
`instance-of` link to `../<class>.ont.yml` is structural and does not discharge this — every
instance needs at least one edge to another *instance*.
[why](bootstrap.evidence.md#a-link-is-not-an-edge)

Keep the seed set at one level of abstraction.
[why](bootstrap.evidence.md#one-level-of-abstraction)

**Distribution** — allocate instances across classes to hit `corpus_depth` total, with a
minimum of 1 per class **where the sources support one**. Give more instances to hub classes
(those with the most edge participation) and fewer to peripheral classes. Seed from root
nodes down so link targets exist when referenced. Prefer depth over breadth: a well-linked
instance with real content is worth more than several shallow stubs.

**When the material for a class does not exist, leave the class empty.** It keeps its
`.ont.yml`, its directory, its README and its ACTIONS file, and it holds no instance nodes.
Do not invent one to satisfy the minimum, and do not stop and wait — ask for the material
once, and if it is not forthcoming, seed what the sources support and record the shortfall.
An empty class is a gap anyone can see; a fabricated instance passes every check this
repository runs and has to be *found* before it can be removed.

`corpus_depth` is a target and not a quota, and it is not revised here — leave it as written,
and let the record below say what was actually seeded and why the two numbers differ.
[why](bootstrap.evidence.md#leave-the-class-empty)

**Record the shortfall.** If the seeded count is short of `corpus_depth`, or any class is
empty, write this before moving on:

```
.yidam/decisions/seed-scope.yml
```

```yaml
id: seed-scope
summary: <one line — N instances seeded against a target of M; which classes are empty>
context: |
  <what material was available; what was asked for and not supplied; which classes each
  source was sufficient to seed>
decision: |
  <the seeded count and the classes that hold no instances; that nothing was fabricated to
  reach the target — name what was not invented, so the record is falsifiable>
rationale: |
  <why an empty class was preferred to a plausible one here; what the gap costs downstream —
  in particular, name any calculator approved in step 5 that now has nothing to read>
```

Do not leave the last clause out. [why](bootstrap.evidence.md#name-the-starved-calculator)

**Existing-repo mode**: instances may represent existing repository artifacts directly. Add
a `source_path:` property pointing to the relevant existing file or directory, and link to
it as an evidence anchor. For example, a `Crate` instance for a Rust workspace member would
carry `source_path: crates/my-crate/Cargo.toml`. This is how the corpus models what already
exists rather than only what the bootstrap creates.

**Opportunistic retrieval**: While seeding, watch for the demand threshold — five or more
instances that share a missing property attributable to a single approved connector source.
When that threshold is met, invoke the connector inline rather than deferring it: fetch the
missing data, populate the instances, and commit the result as part of the seed with
`extract:` — structured data pulled from a primary source. Respect rate limits: pause
between requests; do not batch-hammer a source. Record what was fetched in the commit
message.

### 7. Wire implied edges and scaffold connectors and calculators

After all objects are seeded, read the full corpus — every `.ont.yml` class file, every
class directory, and every instance. Then act on what the user approved in step 5:

**Implied edges** — add each approved edge as an entry in the `links:` field of the
relevant instance `.yml` files. An implied edge resolves a missing relationship between
specific objects; it does not add new content to instances.

Add only the edges you can defend. An edge is a claim — see
[agent-conduct](../guidelines/agent-conduct.md), "An edge is a claim". For each edge before
you write it: could you say, in one sentence, why this relationship holds in this domain? If
yes, write it, and put that sentence in the node body. If the honest answer is that these two
things are associated but you could not say how, use the weakest relationship that is true
rather than the most interesting one that might be. If you cannot do either, do not write the
edge — and if it was approved in step 5, say so in the report rather than quietly dropping
it. [why](bootstrap.evidence.md#only-edges-you-can-defend)

**Connectors** — for each approved connector not already invoked during seeding, scaffold
a crate stub in `crates/`:

```
crates/<connector-name>/
```

The stub should name the external source, describe what corpus classes it feeds, and define
the retrieval interface. Connectors invoked during seeding need no stub — their invocation
and the resulting epistemic commit are the record.

**Calculators** — for each approved calculator: if the seeded corpus contains enough
instances to produce a meaningful result, run it now and commit the output with `compute:`
— a calculator run and its output committed. Otherwise write a stub in `.yidam/skills/`:

```
.yidam/skills/<calculator-name>.md
```

The stub should describe what it computes, which corpus nodes it reads, and what it returns.

**Do not commit anything in this step.** The implied edges are an `establish:` and the
remaining stubs are an `implement:`; both are written in step 8, after the genesis commit.
This step's job is to leave the working tree in the state those two commits describe.
[why](bootstrap.evidence.md#after-genesis-not-before)

### 8. Write the genesis commit and consume transient layers

**The commit sequence.** Everything a bootstrap writes, in order. This block is the whole
list — no step writes a commit that is not here, and the harness reads these verbs to decide
whether a history is a bootstrap's:

```
genesis     the root commit — schema, instances, decision records, .yidam/ structure
establish   the implied edges wired in step 7 — omit if none were approved
implement   the connector and calculator stubs from step 7 — omit if none remained
consume     samudaya
consume     sadhana
vendor      the prelude, into .yidam/.vendor/
regen       the generated blocks, from step 8.5
```

Two of the seven are conditional and the rest are not. `establish:` and `implement:` are
skipped when step 5 approved nothing of that kind — that is a corpus with no implied edges
and no stubs, not a deviation.

**If this step is interrupted before `genesis:` is committed** — the session ends, the
agent is stopped, anything short of the commit landing — the repository is left with a
full corpus on disk, everything staged or ready to stage, and `HEAD` still unborn. To
resume, run `yidam doctor` first — it names this state as "bootstrapped but never
committed" — then re-enter this step at the top and write the commit sequence above from
`genesis:` forward. [why](bootstrap.evidence.md#resume-from-doctor)

`establish:` and `implement:` come *after* `genesis:` and not before, which is the opposite
of the order their steps appear in. [why](bootstrap.evidence.md#after-genesis-not-before)

**Genesis commit** — stage and commit all class definitions (`.ont.yml`), seed instances,
decision records, and the `.yidam/` directory structure as a single genesis commit. Do not
include `sadhana/` or `samudaya/` in this commit, and do not include the step 7 stubs — they
are the `implement:` commit below.

The message should name the domain, summarize the class schema, and describe what seed
objects were created and how they connect — naming at least one specific relationship, not
just that relationships exist. Neither a list of filenames nor a paragraph that would fit
any domain will do. [why](bootstrap.evidence.md#genesis-message-is-testimony)

In existing-repo mode, open the message with `overlay:` instead of `genesis:` and note the
pre-existing commit count: `overlay: <domain> — yidam applied to N-commit repository; M
classes; K instances seeded`. This marks the graph's origin without misrepresenting the repo
history.

**`establish:` and `implement:`** — write them now, in that order, if step 7 produced
anything for them. The `establish:` message says which edges were wired and why each holds;
the `implement:` message names each stub and what it is a stub *for*. If an edge approved in
step 5 was not written because it could not be defended, say so here rather than letting it
disappear.

**Consume samudaya** — after those commits are written, delete `samudaya/`. Skip if
`samudaya/` does not exist (typical in existing-repo mode).

First try the tracked path:

```
git rm -r samudaya/
git commit -m "consume: samudaya — ..."
```

If `git rm` fails because samudaya files were never staged (they are untracked), delete
the directory directly and record the event as an empty commit:

```
rm -rf samudaya/
git commit --allow-empty -m "consume: samudaya — ..."
```

Do not ask the user to run either command manually — the deletion is part of the bootstrap
protocol and must complete before step 9.

The deletion message should record what samudaya contained and what it influenced. If no
seeds were present (only `README.md` and `examples/`), say so explicitly: "no seeds present;
directory removed."

**Consume sadhana** — immediately after consuming samudaya, delete `sadhana/`. Skip if
`sadhana/` does not exist (typical in existing-repo mode).

```
git rm -r sadhana/
git commit -m "consume: sadhana — scaffold template consumed; derived structure in place"
```

If sadhana files were untracked:

```
rm -rf sadhana/
git commit --allow-empty -m "consume: sadhana — scaffold template consumed; derived structure in place"
```

**Vendor the prelude** — immediately after consuming sadhana, move the inherited prelude
into the `.yidam/` infrastructure namespace and delete the rest of the template. Skip if
`yidam/` does not exist (typical in existing-repo mode; the vendor step only applies when
bootstrapping from the yidam template).

**Vendor exactly one directory.** `yidam/prelude/` is what a derived repo inherits; nothing
else under `yidam/` is. [why](bootstrap.evidence.md#vendor-exactly-one-directory)

Because `yidam/` was not staged in the genesis commit (it is untracked), use filesystem
operations and stage the result directly:

```
mkdir -p .yidam/.vendor
mv yidam/prelude .yidam/.vendor/prelude
rm -rf yidam/
```

**Then drop the domain libraries this corpus did not ask for.** `prelude/domains/` is fifteen
domain libraries in three languages each, and a derived repository can build none of them.
[why](bootstrap.evidence.md#prune-the-domains) Read `prelude_domains` out of
`.yidam/decisions/proposals.yml` (step 5) and keep only what it names:

```
cd .yidam/.vendor/prelude/domains
ls -d */ | grep -vE '^(README.md|parity|<selected>)/' | xargs rm -rf
cd -
```

If `prelude_domains` is empty — the common case — remove the whole directory:

```
rm -rf .yidam/.vendor/prelude/domains
```

`prelude/sdks/` stays whole — the prelude's own README and `agent-conduct.md` link into it, so
it is read from inside a derived repository even though it is not built there.

`prelude_domains` is why this deletion survives. `mise run yidam-vendor-update` replaces the
vendored prelude wholesale, so it copies all fifteen back every time; it then reads the field
out of the decision record and prunes to what this repository declared. That is the whole
mechanism — write the field even when it is empty, because the empty list is a decision and
the task reads it as one (#808).

Keep `domains/README.md` when any domain is kept: it is the index that says what the layer is
and how a domain is wired into `crates/Cargo.toml` when the domain computer exists.

**Then delete the two template files bootstrap itself used.** `README.md`, `AGENTS.md`,
`.claude/CLAUDE.md`, `mise.toml`, `.gitattributes`, and `.gitignore` were already
overwritten in step 3, and `.github/workflows/` was already replaced wholesale from
`sadhana/github/workflows/`; what remains is:

```
rm -f BOOTSTRAP.md VERSIONING.md
```

`BOOTSTRAP.md` is the entry prompt for a repo that has not been bootstrapped — this one now
has. `VERSIONING.md` documents how yidam releases its own three layers. Keep `LICENSE` and
`mise.yidam.toml`: the first is generic and the second is the inherited task layer that
`mise.toml` includes. `.gitignore` and `.gitattributes` are already this repository's own —
step 3 overwrote both from `sadhana/root/`.

**Nothing else of yidam's is here to delete, and that is enforced rather than promised.** If
a path does turn up here that this step does not name, the guard in `template_root.rs` is
where the answer belongs, not a longer `rm -f` line.
[why](bootstrap.evidence.md#template-root-is-enforced)

**Confirm the provenance pin.** `.yidam.toml` records which yidam this repo came from; `yidam
clone` and `yidam overlay` write it. Check that it exists and carries a real commit:

```
cat .yidam.toml
```

If the file is missing — the template was copied by hand rather than by `yidam clone` — write
it now, with `commit = "unknown"` if the source SHA is genuinely unavailable. Do not guess a
commit. An honest `unknown` can be repaired by hand; a wrong SHA silently upgrades the repo
against the wrong baseline.

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "<40-char sha, or unknown>"
template  = "untagged"
committed = "<YYYY-MM-DD — the date of that commit>"
```

Then commit the whole vendor step as one event:

```
git add -A
git commit -m "vendor: yidam prelude into .yidam/.vendor/; template files removed"
```

Do not ask the user to run any of this manually — the vendor step is part of the bootstrap
protocol and must complete before step 9.

### 8.5. Run the gate this repository will be gated by

Everything up to here was written and none of it has been checked. The repository now has a
CI workflow, a local gate, and a CLI it can install — and no step has run any of them.
[why](bootstrap.evidence.md#run-the-gate)

**Install the binary this repository pins.** Nothing before this point required the CLI —
step 0's audit runs only when a binary happens to be present — so it is usually not there
yet:

```
mise run yidam-build
```

**Then run the gate, in this order.** Each answers a different question and the order is the
order a failure is cheapest to fix in:

```
mise run graph-check          # is the graph well-formed
yidam regen                   # refresh every generated block
yidam lint --init-baseline    # record what the corpus starts with
yidam lint                    # and read what it says
```

`yidam regen` is the one that is easy to skip and cannot be. The scaffold installed in step 3
carries `<!-- REGEN: ... -->` markers in seven files, and **every one of them is stale on
arrival**. [why](bootstrap.evidence.md#regen-cannot-be-skipped)

Commit the refreshed blocks and the baseline together:

```
git add -A
git commit -m "regen: REGEN blocks populated on the first run of the gate"
```

`regen:` is the operational verb for exactly this — generated content refreshed, no
understanding changed. Keep it out of the genesis commit: genesis is testimony about what the
corpus knows, and a regenerated index table is not testimony.

**If `graph-check` or `lint` reports anything, fix it now.** Fix and amend the commit it
belongs to, or write a `fix:` commit if the genesis commit has already been pushed.
[why](bootstrap.evidence.md#fix-while-warm)

Do not ask the user to run any of this manually. A bootstrap that hands over a repository
whose gate it has never run has not finished; it has stopped.

### 9. Report

Do not begin this step until the genesis commit, both `consume:` commits, the `vendor:`
commit, and the step 8.5 gate run are all done. If any is unresolved, finish it before
proceeding. Step 9 opens by stating the gate result — a handoff that says the repository is
ready is a claim, and this is the one place it can be checked.

Output a structured handoff with seven sections:

**Gate** — one line: the result of the step 8.5 run. Name the commands, say whether each
passed, and name any finding left open and why. "Green as of `<sha>`" is checkable; "the
repository is ready" is not.

**Ontology** — the class definitions written. One line per class; list the outgoing edges.

**Objects seeded** — the instance nodes created. One line each; note which class each
instantiates.

**Classes seeded and classes empty** — every class in one of two lists, with the instance
count for the seeded ones. If any class is empty, name `.yidam/decisions/seed-scope.yml` and
say in one line what material would close it. A scaffold waiting for material and a corpus
that is finished look identical in the four sections around this one.

**Implied edges, connectors, and calculators** — edges wired, crate stubs and skill stubs scaffolded. One line each.

**Conventions not yet scaffolded** — one line each, so the user knows these exist without
finding an empty directory and guessing:

- `agents/` — domain agent definitions. Create it when you write the first agent.
- `packages/` — non-Rust toolkit code (Python/TypeScript connectors, ML pipelines). Create
  it when a capability genuinely belongs outside `crates/`.
- `docs/` — documentation about the repository, as distinct from the corpus's knowledge.
  Create it when there is something to say that is not a corpus node.
- `.yidam/decisions/kuten.yml` — the practice this corpus declared. Name the kuten adopted in
  step 2 and the revision recorded with it, or say that none was adopted: holding no kuten is
  a supported state, `yidam kuten check` reports it as one and exits zero, and one can be
  adopted later by a `decide:` commit carrying the record.
- `.yidam/sangha/` — collective resolution. State the governance mode chosen in step 2. In
  single-elector mode, say that phases run on `phase/<name>` branches and that the sangha
  can be adopted later if a second elector appears.
- `prelude/domains/` — shared pure-function libraries. Name the domains vendored in step 8,
  or say that none were and that the layer exists: the fifteen are listed in the yidam
  repository, and one can be vendored later by adding its name to `prelude_domains` in
  `.yidam/decisions/proposals.yml` and re-running `mise run yidam-vendor-update`. A reader
  who never hears of the layer will write the calculator by hand.

**Next steps** — three concrete, ordered actions:

1. **First catalog entry** — identify the most authoritative data source for this domain
   and add it to `.yidam/catalog/` as the first provenance anchor. Name it specifically.
2. **First corpus expansion** — name the instance node most ready to grow and suggest the
   first sub-node or property to deepen it. This becomes the first `establish:` commit
   after genesis.
3. **First agent** — describe the simplest agent immediately useful in this domain. One
   sentence on what it does and which corpus nodes it draws from. Creating `agents/` with
   that one definition is the action; do not create the directory to hold nothing.

Then ask:

> **Continue?** I can enter a seed/scaffold loop — reading the corpus, identifying gaps,
> and proposing new objects, implied edges, connectors, or calculators until the corpus
> reaches a stable initial state. Reply **yes** to continue, **no** to stop here, or
> describe a specific area to focus on.
