### REGEN refreshes ride in the commit that caused them

**`settle --merge`, `yidam run` and a new `yidam regen --fold` stop writing separate `regen:` commits (#1499).**

`settle --merge` writes the refreshed blocks into the merge, and `merged.regen` is always null.
Each `compute:` commit from `yidam run` carries the blocks it made stale.
`yidam regen --fold` amends the blocks into an unpushed HEAD, with a `Regen:` trailer.
The practice block now reads the history before the commit carrying it, so one reading shifts by one commit.

Run `yidam regen --check` after upgrading. Fold any stale block, or commit it with `--commit`.
See [RFC-0054](rfcs/0054-fold-derived-commits.md).
