# Editor setup

Two surfaces, and the split is deliberate.

| | |
|---|---|
| **`yidam serve --lsp`** | The language server. Diagnostics, definition, references, hover, rename — everything that is a *judgement*, computed by the same functions `yidam lint` runs. Any LSP-capable editor. |
| **The VS Code extension** | The views, claim decoration, the SCM commit box, task wiring — everything that is VS Code-shaped and has no LSP equivalent. |

Both obey one rule, from [RFC-0016](rfcs/0016-editor-surface.md):

> **TypeScript computes affordances. The CLI computes verdicts.**

An affordance is a navigation or authoring convenience whose failure mode is *not helping* —
go-to-definition on an edge, completion in a commit message. A verdict is a statement about
whether the corpus is sound. Verdicts cross the process boundary as JSON from the binary the
repository pins, and the editor renders them. It never derives them, because a TypeScript
re-implementation of the checks is precisely the drift the RFC set exists to close.

**Install the CLI first.** Neither surface bundles, downloads, or builds a binary; with none
reachable there is nothing to render. See [Installation](installation.md).

---

## Set up the language server

`serve --lsp` is in the **light default build** — the binary from any install channel already
has it. `--features index` adds nothing to the LSP.

### What it serves

- **Diagnostics**, live. The checks read the working tree, which is right for a gate and wrong
for an editor: the file you are typing into is the one whose findings you want, and it is the
one on disk that is stale. An overlay closes that. Every check reads through it without knowing
it exists. The findings are about the buffer, and are still computed by the functions `yidam
lint` runs.
- **Definition, references, hover** on `target:` scalars.
- **Rename** over [`yidam rename`](rfcs/0014-node-rename.md). F2 on a node, every inbound
  `target:` rewritten, the file moved, all in one `WorkspaceEdit` the *client* applies — which
  is what keeps undo working. Refused outright, as an LSP error rather than an empty edit, if
  anything would dangle.

### Severity, and the rule that outranks it

Severity follows RFC-0016's table, and **baseline membership outranks check severity in both
directions**. Inherited debt renders as a Hint however severe the check is. `yidam lint` does
not ask *is the corpus clean?* — it asks *did this change make it less clean?*

### Neovim

```lua
vim.api.nvim_create_autocmd('FileType', {
  pattern = 'yaml',
  callback = function(args)
    local root = vim.fs.root(args.buf, { '.yidam' })
    if not root then return end
    vim.lsp.start({ name = 'yidam', cmd = { 'yidam', 'serve', '--lsp' }, root_dir = root })
  end,
})
```

`root_dir` matters. The server takes the workspace from `rootUri` when the client sends one,
and otherwise falls back to `git rev-parse --show-toplevel` from its own cwd. That is not
necessarily the corpus you meant.

### Helix

```toml
# languages.toml
[language-server.yidam]
command = "yidam"
args = ["serve", "--lsp"]

[[language]]
name = "yaml"
language-servers = ["yaml-language-server", "yidam"]
```

Beside `yaml-language-server` rather than instead of it: that one applies the JSON Schemas
`yidam schema` writes, and the two answer different questions. `yidam schema --settings` prints
the `yaml.schemas` mapping to paste into an editor that wants one.

## Install the VS Code extension

Five views over the corpus, lint and `graph-check` verdicts as diagnostics, claim decoration,
and the inherited mise tasks as editor tasks.

### Installing

**On VSCodium, Cursor, Windsurf, Gitpod or code-server** — anything reading
[Open VSX](https://open-vsx.org/extension/goedelsoup/yidam-vscode) — search for *yidam* in the
extensions panel, or:

```sh
codium --install-extension goedelsoup.yidam-vscode
```

**On VS Code itself**, download the `.vsix` from the
[latest `editor/v*` release](https://github.com/goedelsoup/yidam/releases) and install it by
hand:

```sh
code --install-extension yidam-vscode-<version>.vsix
```

VS Code reads the Microsoft Marketplace and nothing else, and this project does not publish
there — the publisher needs an Azure DevOps organisation that does not exist yet. That is stated
rather than papered over: a documented install line that cannot succeed is exactly what
`install-channels.yml` was written to catch.

### It activates only in a corpus

`workspaceContains:.yidam.toml` or `.yidam/**`, and nowhere else. Opening a repository that is
not a derived corpus activates nothing, which is intended — including the yidam template
repository itself, which has neither.

### How it finds a binary

In this order, and it never downloads or builds one:

1. The repository's own build (`.yidam/bin`)
2. `yidam.path`
3. `PATH`
4. The workspace's mise shims

The repository's own build outranks `PATH` deliberately. `mise run yidam-build` installs to
`.yidam/bin/`, beside the pin it was built from. A location like `~/.cargo/bin/yidam` is one
per *machine*, while the pin is one per *repository*. On a machine with two yidam repositories,
it is whichever built last. Preferring `PATH` would let one repository's pinned binary answer
for another's corpus. An explicit `yidam.path` still wins — that is somebody's decision, and
the rest is a default.

If the binary it finds speaks a report contract this build does not understand, verdict
features are disabled and the status bar says so. The extension does not guess at an envelope
it cannot read. `yidam: Show binary and contract status` reports which binary answered.

### What a node rests on

A reader looking at a node in the Corpus view could not ask what it is sourced from without
leaving the view. Sources now hang under the node that cites them, and clicking one opens the
catalog entry. Only nodes that cite something get children.

The edge comes from the CLI: `catalog-audit` gained `cited_by`, naming the corpus instances
that link to each entry — resolved by the same function `catalog-uncited` gates on — plus the
entry's declared `used-by` and a `drift` field computed by the same function
`catalog-used-by-drift` reports. The editor inverts source → nodes into node → sources, which
is a re-index of what the report said rather than a second opinion about what a citation is.

An unretrieved source says `not obtained` and is not reddened, and drift is a tooltip: both
are already verdicts elsewhere, in the lint diagnostics and the Health view.

Health also opens with a **Setup** row from `yidam doctor` — the right binary, a recorded
provenance, a prelude that is not too stale. It is a precondition rather than a gate, so only
`fail` renders red; a light `reports` install with no vector index warns and is normal. Each
remedy is stated in a tooltip and never offered as a click.

### Narrowing a view

The views were built against a four-node fixture. A real derived corpus is 90 nodes across 13
classes. That is thirteen collapsed groups, and a long scroll inside whichever one you open.
The Open questions view is a flat list of sixty-four.

VS Code already narrows a tree — focus one and start typing. It matches the rendered label and
nothing else, so it cannot ask *which class is this in* or *which of these carry an open
claim*. `yidam: Filter the Corpus and Open questions views`, on the funnel in either view's
title bar, asks both by reading `corpus-index` and `open-questions` rather than the screen.
Free text matches a label or a node path, `class:<name>` restricts by class, and `is:open`
keeps the nodes the report names.

The filter lives in memory and is gone with the window — deliberately not a setting, because a
committed one would hide part of a corpus in a window whose reader did not narrow anything. A
narrowed view says so in its message and a narrowed class reads `3 of 12`; the badges keep
counting the repository rather than the view.

Phases, Health and Sangha are not filtered. Health is five rows by construction, Phases and
Sangha are bounded by how many branches a repository has, and nothing measured says they scroll.

### Settings

Five of them, with defaults, in
[Configuration](configuration.md#editor-settings).

## Build the extension from a checkout

```sh
mise run ext-package -- dist/yidam-vscode.vsix   # packages, and checks what is in the package
code --install-extension yidam/editors/vscode/dist/yidam-vscode.vsix
```

`mise run ext-dev` instead opens an Extension Development Host against a staged fixture, which
is the loop for working *on* it — see [Contributing](contributing.md#the-editor-extension).
