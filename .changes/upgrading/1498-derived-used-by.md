### A catalog entry no longer lists what cites it

**Run `yidam migrate --dry-run used-by`, then `yidam migrate used-by` (#1498, RFC-0053).**

It deletes every `used-by:` block from the catalog. `catalog-audit` still reports the citers as `cited_by`.
Read the names it says the list held and nothing cites. Each one is a stale name or a missing citation.
The migration writes nothing to git. Commit the change under the `migrate:` subject it prints.

`catalog-used-by-drift` is gone. `catalog-used-by-declared` warns on an entry that still declares a list.

`catalog-reconcile` is retired and leaves the built-in cluster steps. It writes nothing and exits 0.
Its report keeps `reconciled`, always empty. The next release removes the command.

`propose withdraw` no longer edits a catalog entry. In `catalog-audit`, `drift` is always null.
