# Working in this repository

<!-- TEMPLATE
One sentence naming the domain, so an agent knows what it is working on immediately.
-->

This is a yidam-derived repository: a living knowledge artifact whose git history is the
knowledge graph. It has already been bootstrapped — there is no bootstrap mode here, and
`.yidam/.vendor/prelude/skills/bootstrap.md` is history, not an instruction.

Read [AGENTS.md](../AGENTS.md) before taking substantive action. It names the prelude files
that govern conduct here and the gate that CI runs.

## The short version

- **Nodes** are files in [`.yidam/corpus/`](../.yidam/corpus/). One concept per file. Every
  node needs at least one outgoing link.
- **Sources** are files in [`.yidam/catalog/`](../.yidam/catalog/). Corpus nodes cite them by
  link rather than embedding citations inline.
- **Claims** carry `[verified]`, `[inference]`, or `[open]`. Untagged inference is a defect.
- **Commits** are knowledge events. Epistemic commits (understanding changed) and operational
  commits (a pipeline ran) are written in visibly different styles and never mixed.
- **`.yidam/.vendor/` is read-only.** Fix prelude defects by re-vendoring
  (`mise run yidam-vendor-update`), never by editing in place.

## Before committing

```
mise run regen           # refresh REGEN blocks, then commit the result as `regen:`
mise run ci              # graph-check, graph-lint, regen --check — the whole gate
```

`mise run ci` is what CI runs, and the two are held together by a test upstream. Run it
rather than its parts: `graph-check` reads the graph and says nothing about a stale REGEN
block, which is a failing build all the same.
