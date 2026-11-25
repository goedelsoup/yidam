### `vocabulary --check` exits 1 on a violation

**`yidam vocabulary --check <subject>` now exits 1 when the subject breaks a rule (#1518).**
It printed the verdict and exited 0 before, so a script reading the exit code refused nothing.
Listing mode, `yidam vocabulary` with no subject, still exits 0.

**A subject git wraps is judged by what the wrapper holds.**
`Revert "…"` and `Reapply "…"` are checked by the reverted subject.
`fixup!` and `squash!` are checked by the target's subject after the marker.
`amend!` is checked by the replacement subject on the first line of its body.
So `Revert "wip"` is refused, and `Revert "establish: a node"` passes.
`lint --commits` judges a revert the same way, and still reports an autosquash marker in history as unsquashed.

**Two subjects are not judged at all.**
A generated merge subject is exempt while a merge is in progress, read from `MERGE_HEAD`.
A commit whose staged paths are all `[object] paths` is exempt, as `lint --commits` exempts it.

**The MCP tool `check_subject` reads the repository the same way.**
Its contract is unchanged, and its `violations` now agree with `--check` on every subject above.
`recognized` still says only whether `verb` is in the vocabulary.
So a wrapped or exempt subject can read `recognized: false` with no violations; `violations` is the verdict.

See [the CLI reference](cli-reference.md).
