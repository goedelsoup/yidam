### The cluster base creates the executor's token secret

**`yidam/cluster/base` now creates `yidam-<corpus>-run.service-account-token` (#1237).**
Without it, every pod of a generated workflow waited in `Init` on a mount that never arrived.

**What changes for you: move your base reference to this release's tag.** An overlay that copies the base needs the secret too.
See [cluster-runs.md](cluster-runs.md#apply-it).
