# Changelog

The Marketplace renders this as a tab beside the README. It records what changed for
someone who has the extension installed, which is not the same list as the commit log.

The extension and the `yidam` CLI version independently (VERSIONING.md, Layer 4). What
they negotiate on is neither version but the report contract, `format_version` — so an
entry that changes which contract this build understands says so explicitly.

## 0.3.0

Report contract: still **1**. Every field this build reads that 0.2.0 did not is optional,
and a `yidam` that omits it renders as 0.2.0 did. Where a row needs a binary newer than the
one a repository pins, the entry says which.

- **An expired baseline entry is visible, and the status bar stops ticking over it.** The
  Lint row in Health reads `N new · N inherited · N expired · N stale`, and each expired
  entry is a child row — `<check> — out of time after N commit(s)` — that opens the node. A
  gate failing only on an expired entry used to show a red row reading `0 new · 2 inherited`
  with no children and nothing anywhere saying why. Those rows offer no Bless, and the Lint
  row withholds its own while any entry is expired: `--bless` carries `since` forward rather
  than restamping, so blessing cannot clear one. The repair is the node, or a longer
  `expire_after` in `.yidam/lint-baseline.yml`. Needs `yidam` 0.3.0 or later.

- **A finding that gates renders as an error.** `missing-property` on a property the class
  declares `required: true` escalates to `error` in the CLI; the Problems panel was reading
  the check's declared severity and filing it as a warning. It now reads the finding's own,
  and falls back to the check's where a binary omits it.

- **`yidam: New node` marks the properties the class requires.** A required property is
  scaffolded `prop: ""   # <type>, required`; an optional one as before. Needs `yidam`
  0.10.0 or later for the flag; against an older binary the scaffold is unchanged, because
  an absent flag is not a statement that the property is optional.

- **Refresh refreshes a report that is still running.** `yidam: Refresh` pressed mid-run
  handed back the run it had just been told to drop, and cached that answer until the next
  save or checkout. The `yidam.lint.showBaselined` toggle was swallowed the same way. Both
  now take effect on the next result.

- **An open edge is its own row in Open questions.** A link tagged `claim_tag: open` is
  listed under the node that wrote it, with `relationship → target` beside the label where
  a node's own question shows its path; the row opens the node, because that is the file
  the edge is written in. Needs `yidam` 0.13.0 or later. Against that binary a corpus that
  wrote the bracketed form on links — `claim_tag: "[open]"` — sees a node's claim counts
  drop: the edge's tag stops being counted as the node's prose claim, and belongs to the
  edge.

- **`used-by: []` reads as drift, not silence.** A catalog entry declaring an empty
  `used-by` while nodes cite it now shows a drift object naming the citers it omits, where
  it showed `null` — the state an absent key still shows. Needs `yidam` 0.13.0 or later.

- **A new icon**, the project's own mark rather than the second one the extension had been
  shipping. Nothing else in the listing changes.

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
