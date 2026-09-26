---
name: starting-a-corpus
description: Use in a project that is NOT yet a yidam corpus — no .yidam/ directory, and the yidam MCP server refused to start — when the person wants one to exist. Causes the smallest real corpus to exist from any directory, with the binary and an editor and nothing else; every other skill in this plugin applies the moment it does. Triggers on "make this a yidam corpus", "start a knowledge base", "set up yidam here", "initialise yidam", "the yidam server says this is not a corpus", "yidam refuses to start", "bootstrap a corpus", "I want to track what we know about this".
---

# Starting a corpus

Every other skill here opens by naming a repository that has a `.yidam/` directory. This one
is for the other case, and the other case is the ordinary one: a plugin is installed once and
the person opens projects that were never corpora.

**The server's refusal is correct and is not the thing to work around.** It refused because
there is nothing to serve. What is missing is a corpus, and the useful thing to do is cause
one to exist rather than explain the message.

## Do not derive the repository first

The instinct is to look for a command that scaffolds one. Both of the ones that exist —
`yidam clone <target>` and `yidam overlay .` — copy the template out of the checkout the
shell is standing in, so neither does anything useful in the person's own project. Neither
creates `.yidam/` even where it works: the corpus is authored, not scaffolded.

So the route is the files, and there are six of them. [Your first corpus, by
hand](https://goedelsoup.github.io/yidam/first-corpus-by-hand/) is that route written out,
with the exact output each command answers with. Fetch it if you can. What follows is what to
do when you cannot.

## First, ask what the kinds are

This is the whole of the work and the one part no tool performs. Do not infer an ontology
from the repository you are sitting in — a directory listing is a description of code, not of
what the person knows.

Three questions, and wait for the answers:

- **What are the two or three irreducible kinds here?** Not every noun. The kinds that other
  things are said *about*.
- **What relates them?** One relationship is enough to start. Which of the two kinds authors
  it is a decision, not a detail.
- **What is out of scope?** A kind ruled out now is cheaper than one ruled out after thirty
  nodes carry it.

Two classes and one relationship is a corpus. Eight classes agreed to in one message is a
guess, and every node written under it inherits the guess.

## Then write the files

```
.yidam/corpus/<class>.ont.yml      one per class — what it is, what it may say, what it links to
.yidam/corpus/<class>/<slug>.yml   the instances
.yidam/catalog/<slug>.md           the sources claims rest on
```

Three things go wrong on the first attempt, every time:

- **`direction:` decides which file authors the link.** An edge declared `out` is written in
  the file at the tail. Declaring it on both ends gets you two edges, not one.
- **A property typed `claim` is what makes an evidence tag countable.** Prose in a free-text
  field carrying `[verified]` reads the same to a person and is invisible to every report.
- **An edge no class declares is a broken link, not a new kind.** `yidam graph-check` says
  so, and it says it about the file at the other end.

Tag honestly while you write. `[open]` on a claim nobody has settled is the corpus working,
not a gap to fill in; the `tagging-a-claim` skill has the traps.

## Then watch it fail, once

Run `yidam graph-check` and `yidam lint` before the first commit, and expect findings. A leaf
node linking to nothing is an error rather than a warning, and a `[verified]` claim resting on
no source is a warning worth reading rather than silencing. Both are the gate doing its job
over the person's own material, which is the first moment the point of any of this is
legible.

Report what came back. Do not repair a finding by deleting the claim that produced it.

## Then commit, and restart the server

The first commit's subject is a `genesis:` — the root of a corpus, and the one place that verb
is correct. The export and bundle commands read the domain name off it later, so it names the
domain rather than the act.

The MCP server was refused when this session started, and it does not retry. Say so: the
person restarts Claude Code, the launcher finds `.yidam/`, and the rest of these skills
become answerable against a real corpus rather than against your memory of one.

## Growing it from here

Six files is a corpus, not a finished one. The full route is the bootstrap dialogue — ten
steps whose substance is an ontology argument, run by an agent against a repository derived
from the yidam template. `BOOTSTRAP.md` there is the prompt, and it does not travel with this
plugin: it is written for an empty repository, and loading it into one that already has nodes
is wrong.

**Order matters if they want both.** Derive first and write the corpus into the derived
repository, because `yidam clone <target>` refuses a tree that already holds `.yidam/`. If
the files are already written, derive beside it and move `.yidam/corpus/` across.

## Where the reasoning is

[Your first corpus, by hand](https://goedelsoup.github.io/yidam/first-corpus-by-hand/) for
the worked six files, [the quickstart](https://goedelsoup.github.io/yidam/quickstart/) §4 for
the dialogue, and [bootstrap-flow](https://goedelsoup.github.io/yidam/bootstrap-flow/) for the
protocol the dialogue follows.
