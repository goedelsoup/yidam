# Editor surfaces

Two of them, and the split is deliberate.

| | |
|---|---|
| **`yidam serve --lsp`** | The language server. Diagnostics, definition, references, hover, and rename — everything that is a *judgement*, computed by the same functions `yidam lint` runs. Any LSP-capable editor. |
| [**`vscode/`**](vscode/) | The VS Code extension. The views, the claim decoration, the SCM commit box, the task wiring, the guards — everything that is VS Code-shaped and has no LSP equivalent. |

The rule both obey is RFC-0016's:

> **TypeScript computes affordances. The CLI computes verdicts.**

## `yidam serve --lsp`

In the **light default feature set**. `serve --mcp` needs fastembed, lancedb and protoc;
an LSP needs none of them, and one that required the ML stack would be one nobody could
install. The two transports are gated separately for that reason.

```
cargo install --path yidam/cli          # light: `serve --lsp` works
cargo install --path yidam/cli --features index   # adds `serve --mcp`
```

### What it serves

- **Diagnostics**, live. The checks read the working tree, which is right for a gate and
  wrong for an editor: the file you are typing into is the one whose findings you want, and
  it is the one on disk that is stale. An overlay closes that — every check reads through it
  without knowing it exists, so the findings are about the buffer and are still computed by
  the functions `yidam lint` runs.
- **Definition, references, hover** on `target:` scalars.
- **Rename** — `textDocument/rename` over [`yidam rename`](../../docs/rfcs/0014-node-rename.md).
  F2 on a node, every inbound `target:` rewritten, the file moved, all in one
  `WorkspaceEdit` the *client* applies — which is what keeps undo working. Refused outright,
  as an LSP error rather than an empty edit, if anything would dangle.

Severity follows RFC-0016's table, and **baseline membership outranks check severity in both
directions**: inherited debt is a Hint however severe the check is, because `yidam lint` does
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

`root_dir` matters: the server takes the workspace from `rootUri` when the client sends one,
and falls back to `git rev-parse --show-toplevel` from its own cwd.

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
`yidam schema` writes, and the two answer different questions.

### VS Code

The extension does not use the LSP today. Its providers already cover navigation, and
migrating working code to gain a process would be a change with no user-visible upside. The
one thing it lacks is rename — see [`vscode/README.md`](vscode/README.md).
