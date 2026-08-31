# examples

*Worked corpora, for reading rather than for deriving from.*

This directory is **not** copied into a derived repository. `yidam clone` excludes it
deliberately: an example is a whole corpus, and a new repository should be born with its own
ontology rather than someone else's eight nodes.

| Example | Domain | Shows |
|---|---|---|
| [streamflow/](streamflow/) | Streamflow on regulated rivers | A three-class ontology, claim tags at all three tiers, a catalog source and what it does not answer, two decision records, and one domain skill |
| [property/](property/) | Title research on a parcel | A chain of title as a typed path, a rejected `owner` class, and a gap the graph reports as a reasoned absence rather than as silence |
| [journalism/](journalism/) | Investigative reporting on a company | Which findings rest on a single source, a rejected `allegation` class, and two vault audiences — a document the corpus may cite and may not host |
| [incidents/](incidents/) | Incident retrospectives | Which contributing factors recur and which remediations nobody verified, a rejected `root-cause` class, and a **history** long enough for `replay` to show a slope |

## The pages are transcripts, and they are re-run

A walkthrough under [`docs/walkthroughs/`](../docs/walkthroughs/) is written as a session: a
`$ yidam …` line and the output it produced against the example it links. Every one of those
is executed against a fresh copy of the corpus and diffed, so a page cannot go on claiming an
answer the corpus stopped giving. One `graph-check` count already had: the commit that added a
twelfth instance to `journalism/` left the page saying eleven, and the whole suite was green
until this gate ran.

Two consequences worth knowing before editing either side:

- **Change a corpus and the page goes red.** That is the gate working. Re-run it with
  `UPDATE_TRANSCRIPTS=1 cargo test --manifest-path yidam/cli/Cargo.toml --test
  walkthrough_transcripts` and read the diff; it rewrites the blocks from a fresh run and
  leaves alone any that differ only in a commit sha.
- **A page with no corpus may carry no `console` block.** The three sketch walkthroughs teach
  from `samudaya/examples/`, which is markdown rather than a corpus, so there is nothing to
  run and a transcript there would be an unchecked claim by construction. Use a `text` fence.

An example that needs its own commit history — because the page asks a question with `--at` or
`replay` — ships a `history.toml`, and the gate builds the repository from it one commit at a
time. `property/` and `incidents/` do; the others get a single genesis commit.

## Why these exist rather than a link to a real repository

Of the derived repositories that actually run, two are private and one deliberately so. The
third is public and is the **divergence canary** — the corpus upstream reports measure
against. A canary is by definition the repository that violates things, which is not what a
newcomer should be shown as the model.

So the teaching examples are purpose-built and live here, where they can be shaped for
teaching and where this repository's own gates cover them. An example that has drifted is
worse than no example, because it is read as authoritative and copied.

## Adding one

Commit a directory under `examples/` containing a `.yidam/`. Nothing else: the gates
**discover** their subjects from `git ls-files` rather than naming them, so a new corpus is
covered the moment it is tracked, and one that is present but unstaged fails
`every_example_on_disk_is_discovered` rather than being quietly skipped.

| Gate | Requires |
|---|---|
| `yidam/cli/tests/example_corpus.rs` | `graph-check` clean, `lint` at **zero findings at every severity**, at least one open question, a catalog entry, two decision records, a skill, and at least two classes — with the class count `graph-check` reports matching the `*.ont.yml` files the corpus ships |
| `yidam/cli/tests/class_schemas.rs` | every instance validates against its own compiled class schema, and each schema requires exactly what its ontology declares required |
| `yidam/cli/tests/walkthrough_transcripts.rs` | every `$ yidam …` block on the walkthrough page that links this example still produces what the page records |

Both files keep a few tests pinned to `streamflow` on purpose, and say so where they are
defined: those are regression tests and compiler-behaviour tests that know one corpus's edge
shape, not checks an example has to pass.

## Reading one

An example is a repository. To run the tooling against it, copy it out and initialise it —
`yidam` locates a repository with `git rev-parse --show-toplevel`, so running the binary
inside this directory finds *yidam*, which is not a corpus:

```sh
cp -R examples/streamflow /tmp/streamflow
cd /tmp/streamflow && git init -q && git add -A && git commit -qm genesis
yidam graph-check
yidam lint
yidam open-questions
```

[quickstart.md](../docs/quickstart.md) walks through this and then breaks it on purpose.
