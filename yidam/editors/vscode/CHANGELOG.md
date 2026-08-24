# Changelog

The Marketplace renders this as a tab beside the README. It records what changed for
someone who has the extension installed, which is not the same list as the commit log.

The extension and the `yidam` CLI version independently (VERSIONING.md, Layer 4). What
they negotiate on is neither version but the report contract, `format_version` — so an
entry that changes which contract this build understands says so explicitly.

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
