# What yidam is

> What you commit to shapes you.
>
> A yidam is not found. It is chosen — and in the choosing, it begins to choose back. The form
> does not complete itself. It completes *through* being held.

yidam is a template and a toolchain for **living knowledge artifacts**: git repositories whose
history *is* a knowledge graph, maintained collaboratively by humans and agents.

Most knowledge systems keep the graph in a database and the prose in files, then spend their
lives keeping the two in agreement. yidam removes the database. In a yidam-derived repository
the graph is the repository:

| Git | Graph |
|---|---|
| a file | a node — one concept, relation, artifact, or open question |
| a markdown link `[label](path)` | a directional edge |
| a commit | a knowledge event |
| a branch | a parallel inquiry thread |
| a merge | a synthesis |

Provenance, attribution, review, blame, and time travel come for free, because git already does
all of them. What yidam adds is the model that makes a repository legible as *knowledge* rather
than as code — and the tooling to query, lint, index, and export it.

**This is not a software project. It is a research instrument.** There may be software in it —
most derived repositories grow a [domain computer](domain-computer.md) of connectors and
calculators — but that is not its nature.

## What it is not

- **Not a static document collection.** A wiki accumulates; a corpus is gated. Orphan nodes,
  dangling edges and unlabelled classes fail a build.
- **Not a scratchpad.** Every commit is a permanent node in the graph. There is no draft area
  whose contents do not count.
- **Not a database with a git front-end.** Nothing is projected into or out of a store. The
  files are the data, and `git log` is the audit trail.

## Two kinds of commits, and no others

**Epistemic** — what the corpus knows has changed. A change in understanding. The commit message
is *testimony*, not a changelog.

**Operational** — the pipeline advanced. An extraction ran, an index rebuilt. Legitimate
provenance, but not a knowledge event.

The distinction is carried by a **closed vocabulary of leading verbs** (`establish:`, `revise:`,
`open:`, `close:`, `synthesize:` … versus `extract:`, `refresh:`, `index:`, `regen:` …), and it
is enforced rather than encouraged: `yidam lint --commits` gates on it, and `yidam vocabulary`
prints the list with the reasoning behind closing it.

Closing the vocabulary is what makes the history queryable. `yidam log --epistemic` shows the
testimony and nothing else — which is only possible because "and nothing else" is decidable.

## What a corpus is made of

A derived repository keeps its knowledge under `.yidam/`:

- **`corpus/`** — the graph itself. Class definitions in `<class>.ont.yml`, instances in
  `<class>/<instance>.yml`. One concept per file; nodes are short by rule, and a node with no
  outgoing edge is a finding.
- **`catalog/`** — one node per external source, so a claim can point at where it came from.
- **`decisions/`** — structured records of choices made at bootstrap and after.
- **`sangha/`** — governance: electors, their positions, and settled resolutions.

Claims carry their epistemic status inline: `[verified]` is supported by a committed primary
source, `[inference]` is a reasonable conclusion from verified facts, and `[open]` is a live
question. [Information architecture](information-architecture.md) has the full shapes;
[Vocabulary](vocabulary.md) defines every term.

## Where the ontology comes from

A yidam repository does not ship with an ontology, and it does not infer one. It is
**bootstrapped**: an agent reads the inherited prelude, runs an ontology-discovery dialogue with
you, and only then writes the genesis commit — a faithful rendering of the model you confirmed
together. Scaffolding before that dialogue is the failure the process is shaped against.

See [Bootstrap flow](bootstrap-flow.md) for the sequence, and [Quickstart](quickstart.md) to
watch one happen.

## What you get for holding the discipline

The constraints are the point: because the corpus is gated, it can be *computed over*.

- **Typed traversal.** `yidam query 'reach -measured-by-> gage'` walks the resolved graph, at any
  commit, and optionally across installed dependencies.
- **Context packs.** `yidam pack` fills a token budget with a query's answer and reports what did
  not fit — which is what an agent needs and what flat retrieval cannot say.
- **An agent surface.** `yidam serve --mcp` serves the corpus to any MCP-capable agent, with an
  anchored traversal instead of a similarity guess.
- **Time.** `yidam replay` reconstructs corpus health across the whole history, which is where a
  corpus quietly accumulating uncited nodes becomes visible.

None of that needs a database, and none of it is a projection that can fall out of sync.

## Reading on

| If you want to | Go to |
|---|---|
| Try it | [Quickstart](quickstart.md) — about twenty minutes |
| Install the CLI | [Installation](installation.md) |
| Understand the file shapes | [Information architecture](information-architecture.md) |
| Bootstrap a repository | [Bootstrap flow](bootstrap-flow.md) |
| Point an agent at a corpus | [Connecting an agent](mcp-server.md) |
| Look up a term | [Vocabulary](vocabulary.md) |
