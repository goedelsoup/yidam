### `yidam rename` moves a catalog entry

**`yidam rename catalog/old new` renames a catalog entry (#1159).** Before, the command
took corpus nodes only. Renaming an entry by hand left every edge `source:` naming its old
stem, which `edge-source-unresolved` then reported.

The command now moves `.yidam/catalog/old.md` to `new.md` and rewrites every citation of it.
An edge `source:` is rewritten in the spelling its author used, stem or path. A markdown link
under `.yidam/` that resolved to the entry is re-relativized from the file that holds it. That
covers node prose, other entries, decision records and the catalog README. On the largest
corpus we measured, one rename rewrote 107 citations across 41 files. Lint was identical
before and after.

Anything that names the old file without linking it is reported and not rewritten. The
report's `corpus_dir` is `.yidam` for a catalog entry, and `from` and `to` read
`catalog/<name>.md`. `--dry-run` prints the plan and changes nothing. Nothing changes for a
corpus node rename.
