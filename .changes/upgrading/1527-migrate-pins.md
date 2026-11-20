### Dated reads can become commit pins

**A new migration, `yidam migrate pins`, writes `commit:` on a `file` location from the read its entry's prose dates (#1527).**

It takes the last "read at <commit>" or "read again at <commit>" in the body.
The commit is written in full, with `repo:` when only one `path` dependency has it.
The prose is not changed. A read it cannot place is listed with its reason.

It is opt-in. Run `yidam migrate --dry-run pins` first.
