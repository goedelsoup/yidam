# examples

*Worked corpora, for reading rather than for deriving from.*

This directory is **not** copied into a derived repository. `yidam clone` excludes it
deliberately: an example is a whole corpus, and a new repository should be born with its own
ontology rather than someone else's eight nodes.

| Example | Domain | Shows |
|---|---|---|
| [streamflow/](streamflow/) | Streamflow on regulated rivers | A three-class ontology, claim tags at all three tiers, a catalog source and what it does not answer, two decision records, and one domain skill |

## Why these exist rather than a link to a real repository

Of the derived repositories that actually run, two are private and one deliberately so. The
third is public and is the **divergence canary** — the corpus upstream reports measure
against. A canary is by definition the repository that violates things, which is not what a
newcomer should be shown as the model.

So the teaching example is purpose-built and lives here, where it can be shaped for teaching
and where this repository's own gates cover it. `yidam/cli/tests/example_corpus.rs` runs
`graph-check` and `lint` against it on every CI run and requires **zero findings at every
severity** — an example that has drifted is worse than no example, because it is read as
authoritative and copied.

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
