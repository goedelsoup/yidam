### `due` reports a seventh clock, for what a corpus lacks

**`yidam due` and `yidam cycle` now report seven clocks (#1491).** The new one is `coverage`. It
stays `undeclared` until you declare a `[[due.coverage]]` universe, so no exit code changes. A
consumer that counted the clocks, or listed their ids, sees one more.

**If your corpus tracks a published list, declare it.** A session's bills and a body's meetings
are lists. Name the catalog entry that holds the list, how to read it, and the class that
should cover it. `select` takes a JSON path, and `column` takes a CSV header:

```toml
[[due.coverage]]
source = "lis-136-legislation-list"
select = "$.items[*].number"
class = "bill"
property = "number"
```

`due` then counts the members no instance carries, and `--format json` names each one. A list
past its TTL, or an empty one, cannot vouch for a zero gap. The clock reads `unmeasurable` there,
not `ok`.
