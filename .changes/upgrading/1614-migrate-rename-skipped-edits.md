### `migrate` and `rename` refuse an edit they cannot make, and keep CRLF

**`yidam migrate` and `yidam rename` now refuse a planned edit they cannot write, and write nothing (#1614).**

Before, such an edit was dropped, and the summary and commit subject still counted it.
`rename` hit this on a markdown link with a title, such as `[x](entry.md "title")`.
That link was left pointing at the old name.
A file saved with CRLF line endings now keeps them, and only the edited lines change.
A blocked `migrate` no longer prints `Error: migrate: blocked` under its refusal.
It still exits 1, as a blocked `rename` does.
