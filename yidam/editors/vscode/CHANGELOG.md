# Changelog

The Marketplace renders this as a tab beside the README. It records what changed for
someone who has the extension installed, which is not the same list as the commit log.

The extension and the `yidam` CLI version independently (VERSIONING.md, Layer 4). What
they negotiate on is neither version but the report contract, `format_version` — so an
entry that changes which contract this build understands says so explicitly.

## 0.2.0

- **A node's sources, under the node.** The Corpus view had no surface for the provenance
  layer, so asking what a node rests on meant leaving the view. `yidam catalog-audit` gained
  a `cited_by` field naming the instances that cite each entry — plus the entry's declared
  `used-by` and a `drift` field for how the two disagree, computed by the same function
  `catalog-used-by-drift` gates on. Report contract: still **1**; adding a field is not a
  break.

  Which is also why this section is **empty rather than broken** against a `yidam` older
  than 0.6.0. Adding a field is not a break, so such a binary still reports contract `1` and
  the handshake has nothing to refuse — and the extension updates itself while a repository
  builds its binary from the commit pinned in `.yidam.toml`, so the two drift apart in
  ordinary use. A node simply lists no sources until the binary can name them.

- **A Setup row in Health**, from `yidam doctor`: the right binary, a recorded provenance, a
  prelude that is not too stale. First in the view because it is a precondition rather than a
  gate — only `fail` renders red, and each remedy is stated in a tooltip and never run.

- **The Corpus and Open questions views can be narrowed.** `yidam: Filter the Corpus and
  Open questions views`, on the funnel in either view's title bar. Free text matches a
  label or a node path; `class:<name>` restricts by class and `is:open` keeps the nodes
  `yidam open-questions` names — the two questions VS Code's own type-to-filter cannot ask,
  because it matches the rendered label and nothing else.

  The filter is held in memory and gone with the window rather than written to settings, a
  narrowed view says so in its message, and the badges keep counting the repository rather
  than the view.

## 0.1.0

First published release. Report contract: **1**.

The extension has existed and been testable since RFC-0016; what is new is that it can be
obtained. Previously it reached a person only as a `.vsix` built by hand on the machine
that wanted it.

- Five views: Corpus, Open questions, Phases, Health, and Sangha (collective repositories
  only).
- Lint and `graph-check` verdicts as diagnostics, re-run on save, with baselined
  violations shown as faded hints.
- Claim decoration for `[verified]` / `[inference]` / `[open]`, off in high-contrast
  themes.
- Neighbourhood view, node creation, phase-branch checkout, and the inherited mise tasks
  as VS Code tasks.
- `.yidam/.vendor/**` marked read-only, so an edit that would be discarded at the next
  re-vendor cannot be made by accident.

**It does not bundle a `yidam` binary and never will.** `.yidam.toml` records which yidam
governs a corpus, and only it gets to say. The extension resolves one from `yidam.path`,
then `PATH`, then the workspace's mise shims — and if what it finds speaks a contract this
build does not understand, it disables verdict features and says so rather than guessing.
