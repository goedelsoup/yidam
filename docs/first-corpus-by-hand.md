# Your first corpus, by hand

*Six files, an editor, and the binary. No agent session and no template checkout. Under
twenty minutes.*

[quickstart.md](quickstart.md) §4 makes a corpus the way the product means it to be made.
The ontology is argued out in a dialogue with an agent. That is the better corpus, and it is not the
shortest way to see what one is. It needs a checkout of the yidam template and an agent
session, and a reader without either stops at §3.

This page is the other route. You write the files. Nothing here is a lesser corpus. The tree you
finish with passes `graph-check` and `lint`, answers `open-questions`, and serves over MCP. It is the same shape the bootstrap dialogue produces, reached by typing instead of
talking.

**What is not here.** No `.yidam.toml`, no REGEN blocks, no lint baseline, no decision
records. Each is optional, and each is easier to add once you have something to add it to.

---

## What a corpus is, as files

Everything lives under `.yidam/corpus/`, and there are two kinds of file in it.

| File | What it is |
|---|---|
| `<class>.ont.yml` | A **class** — the kinds of thing in your domain, and what may link to what |
| `<class>/<slug>.yml` | An **instance** — one node of that class |

A third directory, `.yidam/catalog/`, registers the sources nodes draw on. One entry is
enough, and step 4 says why you want it.

---

## 1. Make the repository — one minute

```sh
mkdir tailwater && cd tailwater
git init
mkdir -p .yidam/corpus/concept .yidam/corpus/gauge .yidam/catalog
```

The `git init` is not ceremony. A corpus is a git history with a gate over it, and `lint`
dates a finding against that history.

---

## 2. Declare two classes — five minutes

This is the part the dialogue exists for, and the part to spend your time on. Two classes and
one relationship are enough to start. A concept is something you are trying to understand; a
gauge is an instrument that measures one.

**`.yidam/corpus/concept.ont.yml`**

```yaml
class: concept
label: Concept
description: A unit of understanding in this corpus.
properties:
  - name: claim_tag
    type: claim
    description: The evidence standing of this node.
edges:
  - relationship: relates-to
    target: concept
    direction: out
    description: A bare association between two concepts.
  - relationship: measured-by
    target: gauge
    direction: in
    description: A gauge measures this concept — authored on the gauge.
```

**`.yidam/corpus/gauge.ont.yml`**

```yaml
class: gauge
label: Gauge
description: An instrument reading a concept in the field.
properties:
  - name: station
    type: string
    description: The operator's identifier for this station.
edges:
  - relationship: measured-by
    target: concept
    direction: out
    description: The concept this gauge reads.
```

Three things are being decided here, and only the first looks like a decision.

**`direction` says who authors the edge.** `measured-by` is declared `out` on the gauge and
`in` on the concept. So the link is written in the gauge's file, and the concept gets it
without being edited. Declaring it `out` on both would let two files disagree.

**A property typed `claim` is an evidence tag**, not a string that happens to hold one. It is
what makes a claim countable. Untyped, the tag is prose about the standing rather than the
standing.

**An edge you do not declare is a broken link.** That is the whole of what a class is for.
`relates-to` between two concepts is licensed, and anything else fails the gate.

---

## 3. Write three nodes — five minutes

**`.yidam/corpus/concept/low-flow.yml`**

```yaml
class: concept
label: Low flow
description: >-
  A discharge regime — the flow a channel falls to between storms. Read off
  [the stage–discharge relation](../../catalog/stage-discharge.md). [verified]
properties:
  claim_tag: verified
links:
  - target: ../concept/mixing-zone.yml
    relationship: relates-to
```

**`.yidam/corpus/concept/mixing-zone.yml`**

```yaml
class: concept
label: Mixing zone
description: >-
  Where the discharge plume stops being distinguishable from the receiving water.
  Nobody here has measured it. [open]
properties:
  claim_tag: open
links:
  - target: ../concept/low-flow.yml
    relationship: relates-to
```

**`.yidam/corpus/gauge/riffle-station.yml`**

```yaml
class: gauge
label: Riffle station
description: A stage gauge at the riffle, reporting every fifteen minutes.
properties:
  station: "03274000"
links:
  - target: ../concept/low-flow.yml
    relationship: measured-by
```

One node is `[verified]` and one is `[open]`, and that asymmetry is the point. A corpus where
every claim is settled is a corpus whose tags are decoration.

`links:` targets are paths relative to the file they are written in, which is why each one
starts `../`. The gauge authors the `measured-by` edge because step 2 declared it `out`
there.

---

## 4. Register the source — two minutes

A `[verified]` claim resting on nothing is a finding, and `lint` will say so. A catalog entry
is what it rests on.

**`.yidam/catalog/stage-discharge.md`**

```markdown
---
name: Stage–discharge relation
description: The rating curve the low-flow figures are read off.
type: paper
obtained: true
---

Where the numbers came from.
```

The link in `low-flow.yml` is an ordinary markdown link into `.yidam/catalog/`. That is the
whole citation mechanism. Nothing else has to be configured.

---

## 5. Let the editor fill in the rest — one minute

```sh
yidam schema
```

It writes JSON Schema for every shape above into `.yidam/schemas/`. That includes one file
per class, derived from what you declared in step 2. `yidam schema --settings` prints the block to paste
into `.vscode/settings.json`. After that an editor completes property names and rejects a
class you have not declared.

This is optional and it is the step that changes how the next hour feels.

---

## 6. Run the gates — two minutes

Commit first. The genesis subject is where `export` and `bundle` read the domain name from.

```sh
git add -A && git commit -m 'genesis: tailwater'
```

```sh
yidam graph-check
# Checked 3 instances across 2 classes — all clean.

yidam lint
# lint: 0 finding(s), no errors

yidam open-questions
# - [Mixing zone](.yidam/corpus/concept/mixing-zone.yml)

yidam status
# **3 nodes** · 1 open · 1 sources · claims 2v / 0i / 2o
```

Now break it, because a gate you have not watched fail is a gate you do not believe. Delete
the `links:` block from `low-flow.yml`:

```text
ERROR [orphan-out] Node connected to nothing — 1 finding(s)
  .yidam/corpus/concept/low-flow.yml: no outgoing links
```

Exit code 1. Put it back, and read [quality-rubric.md](quality-rubric.md) for what else is
checked.

`yidam doctor` will warn that there is no vector index. That is accurate and it is not a
problem. `retrieve` falls back to keyword search, and says `degraded` when it does.

---

## 7. Point an agent at it

```sh
claude mcp add yidam -- yidam serve --mcp
```

The server answers on this tree with no further setup. [mcp-server.md](mcp-server.md) has the
tools and the ways to read their answers.

The Claude Code plugin registers the same server and adds the skills that call it. One of
them, `starting-a-corpus`, is this page driven by an agent. Reach for it on the next corpus;
this one is already written.

---

## The worked minimum, kept honest

The corpus above is the shape of
[`yidam/prelude/sdks/parity/fixtures/reports/basic/repo/.yidam/`](../yidam/prelude/sdks/parity/fixtures/reports/basic/repo/.yidam/),
which every report golden in this repository is generated from. It is two `.ont.yml` files,
four instances and a catalog, and it is exercised on every commit.

`yidam/cli/tests/first_corpus.rs` builds this page's files out of the blocks above and runs
the gates over them. A command whose output drifts from what is printed here fails there.

---

## Then grow it with the dialogue

What you have is a corpus, not an ontology you have argued about. Two classes were chosen in
five minutes, and the second week is where that shows.

The bootstrap dialogue is the repair, and it reads better as a second step than a first. You
arrive at it with files to disagree with rather than a blank page.

**Derive the repository before you write the corpus, if you can.** Both `yidam clone` and
`yidam overlay` refuse a tree that already holds `.yidam/`. Run this on the empty repository
from step 1, then write steps 2 to 4 inside it:

```sh
git clone https://github.com/goedelsoup/yidam
cd yidam && yidam overlay /path/to/tailwater
```

`overlay` adds the prelude, the task layer and the agent routing to a repository that already
has content. An agent opening it afterwards finds no commits, reads `BOOTSTRAP.md` and enters
the dialogue.

Already wrote the corpus first? `yidam clone ~/tailwater-full` makes a fresh derived
repository, and `.yidam/corpus/` moves across as ordinary files.

[quickstart.md](quickstart.md) §4 is the dialogue itself, and
[bootstrap-flow.md](bootstrap-flow.md) is its specification.

## Where to go next

| If you want | Read |
|---|---|
| The rest of the loop | [quickstart.md](quickstart.md) |
| What a class should span | [`ontology/what-an-ontology-is.md`](ontology/what-an-ontology-is.md) |
| How to write a node | [information-architecture.md](information-architecture.md) |
| What the gates check | [quality-rubric.md](quality-rubric.md) |
| What the terms mean | [vocabulary.md](vocabulary.md) |
