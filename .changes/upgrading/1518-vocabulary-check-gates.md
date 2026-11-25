### `vocabulary --check` exits 1 on a violation

**`yidam vocabulary --check <subject>` now exits 1 when the subject breaks a rule (#1518).**
It printed the verdict and exited 0 before, so a script reading the exit code refused nothing.
Listing mode, `yidam vocabulary` with no subject, still exits 0.
The MCP tool `check_subject` is unchanged and still returns its verdict in the payload.

**Subjects git writes are exempt from `--check`.**
These are `Revert "…"`, `Reapply "…"`, and the autosquash markers `fixup!`, `squash!` and `amend!`.
A generated merge subject is exempt while a merge is in progress, read from `MERGE_HEAD`.

**`lint --commits` exempts reverts too, and still reports an autosquash marker.**
A marker in history landed unsquashed, and the finding now says so.

See [the CLI reference](cli-reference.md).
