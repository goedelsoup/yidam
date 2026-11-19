### Prose that names a sibling's commit is reported

**A new check, `prose-commit-unpinned` (warning), reads prose under `.yidam/` for "at <hex>" (#1523).**

It reports a commit that this repository does not have.
The finding names the `path` dependency in `.yidam/tonpa.toml` that has the commit.
A commit in this repository is left alone, and `docs/` is not read.

The fix is to move the read into a `file` location with `commit` and `repo`.
A warning does not fail `lint`, so existing prose costs nothing until you next touch it.
