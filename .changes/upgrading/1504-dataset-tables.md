### A table is a catalog entry that declares its columns

**A `dataset` entry may now declare its table with `columns:` and `key:` (#1504, RFC-0059).**

The entry's one `file` location must name a tracked `.tsv` or `.csv`.
Each column takes a `name` and a `type`: `string`, `number`, `date` or `ref`.
A column may also take `required`, `unit` and `values`, as a list property's `fields:` do.

```yaml
type: dataset
location:
  - kind: file
    value: data/sos-receipts.tsv
columns:
  - { name: report_id, type: string }
  - { name: line, type: number }
  - { name: amount, type: number, unit: USD }
key: [report_id, line]
```

**Three new checks read a declared table.**

`table-declaration-invalid` is an Error on a declaration that cannot be held to.
`table-shape` is an Error on a file that departs from its declaration.
`table-ref-unresolved` is a warning on a `ref` cell that names no node.
An entry outside `type: dataset` that carries `columns:` or `key:` is not read for either.
A `dataset` entry already using `columns:` for something else is reported by the first check.

**`yidam run` checks a declared table before it reads one and after it writes one.**

A failing read stops the step before it starts.
A failing write fails the run, and nothing is committed.
An undeclared table is read as before.

**The kit gains `table(path)` in Python and Rust.**

It reads a declared table into rows typed by its columns.
`yidam run` names the declarations in `$YIDAM_TABLES`, and the receipt records their digest.
An older pin sets no `$YIDAM_TABLES`, so `table()` refuses there; update the pin first.

**A markdown link may cite one row, as `sos-receipts.md#row=OH-2025-88412,3`.**

The fragment holds the key's values in `key:` order, joined by `,`.
`broken-prose-link` reports a key no row holds, or the wrong number of values.
An older pin reads the fragment as a heading anchor, and the link still cites the entry.
This release is a minor bump.
