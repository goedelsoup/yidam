### `yidam gather` asks pinned peers a question

**`yidam gather <name>` runs the question in `.yidam/gathers/<name>.toml` against each `tonpa` peer (#476).**
Each peer's answers land as `cites:` on one `?` question node, on a `propose/*` branch.

**What changes for you: nothing, unless you write a gather.** `.yidam/gathers/` is new and absent by default.
No existing command changed. See [cli-reference.md](cli-reference.md) for the file's shape.
