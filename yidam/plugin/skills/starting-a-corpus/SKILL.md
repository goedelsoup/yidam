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

## `yidam init`, not `yidam clone` or `yidam overlay`

`yidam clone <target>` and `yidam overlay .` copy the template out of the checkout the shell
is standing in, so neither does anything useful in the person's own project. Do not reach for
them here.

`yidam init` is the one that runs anywhere. It writes a class file per `--class`, one example
node in each, and a catalog entry — the shape below, with every placeholder saying what to
replace it with — and the tree it leaves passes `graph-check` and `lint` as written. It reads
no template, creates no repository, and does not commit.

**But not yet.** Run it *after* the conversation in the next section, with the classes that
conversation settled, because the class names are what it takes on the command line and
renaming a class after nodes carry it means rewriting every edge into them. The corpus is
still authored rather than scaffolded; what `yidam init` removes is the typing, not the
thinking.

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

## Then run it, and edit what it wrote

```
yidam init --class <first> --class <second>
```

in the person's repository — it needs a git repository and refuses to create one, so
`git init` first if there is none. What appears:

```
.yidam/corpus/<class>.ont.yml      one per class — what it is, what it may say, what it links to
.yidam/corpus/<class>/<slug>.yml   the instances
.yidam/catalog/<slug>.md           the sources claims rest on
```

Now edit. The descriptions are placeholders and say so; the relationship is `relates-to`,
which is a blank rather than a suggestion, and renaming it to the one the conversation
settled is the first edit to make. Replace the example nodes with things the person actually
knows, one claim's worth each.

Three things go wrong on the first attempt, every time — `yidam init` writes each of them
correctly and the comments in the files it wrote say why, which is the cheapest place to
learn them:

- **`direction:` decides which file authors the link.** An edge declared `out` is written in
  the file at the tail. Declaring it on both ends gets you two edges, not one.
- **`type: claim` makes the property's *value* a tag.** `claim_tag: verified` written bare is
  a standing the reports read; undeclared it is an ordinary string and nothing counts it. A
  bracketed `[verified]` in prose is counted either way — what the declaration buys is the
  standing as a field rather than as text a scan happens to find.
- **An edge no class declares is a broken link, not a new kind.** `yidam graph-check` says
  so, and it says it about the file at the other end.

Tag honestly while you write. `[open]` on a claim nobody has settled is the corpus working,
not a gap to fill in; the `tagging-a-claim` skill has the traps.

## Then watch it fail, once

Run `yidam graph-check` and `yidam lint` before the first commit, and expect findings. What
`yidam init` wrote passes both; what the person wrote over it often will not, and that is the
interesting part. A leaf node linking to nothing is an error rather than a warning, and a
`[verified]` claim resting on no source is a warning worth reading rather than silencing.
Both are the gate doing its job over the person's own material, which is the first moment the
point of any of this is legible.

Report what came back. Do not repair a finding by deleting the claim that produced it.

## Then commit, and restart the server

The first commit's subject is a `genesis:` — the root of a corpus, and the one place that verb
is correct. The export and bundle commands read the domain name off it later, so it names the
domain rather than the act.

The MCP server was refused when this session started, and it does not retry. Say so: the
person restarts Claude Code, the launcher finds `.yidam/`, and the rest of these skills
become answerable against a real corpus rather than against your memory of one.

## Growing it from here

What `yidam init` wrote is a corpus, not a finished one. The full route is the bootstrap dialogue — ten
steps whose substance is an ontology argument, run by an agent against a repository derived
from the yidam template. `BOOTSTRAP.md` there is the prompt, and it does not travel with this
plugin: it is written for an empty repository, and loading it into one that already has nodes
is wrong.

**Order matters if they want both.** Derive first and run `yidam init` inside the derived
repository, because `yidam clone <target>` refuses a tree that already holds `.yidam/` — and
so does `yidam init`. If the files are already written, derive beside it and move `.yidam/corpus/`
across.

## Where the reasoning is

[Your first corpus, by hand](https://goedelsoup.github.io/yidam/first-corpus-by-hand/) for
what each of those files means, written out one key at a time —
[the quickstart](https://goedelsoup.github.io/yidam/quickstart/) §4 for the dialogue, and
[bootstrap-flow](https://goedelsoup.github.io/yidam/bootstrap-flow/) for the protocol the
dialogue follows.
