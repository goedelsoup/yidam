### Every generated cluster pod meets the `restricted` Pod Security Standard

**`yidam cluster workflow` now writes a security context on every template (#1230).**
Each pod runs as uid 1000, with a read-only root filesystem and no capabilities.
It writes only to two `emptyDir` volumes, `/tmp/yidam` and `/home/yidam`, and a `file://` vault.

**What changes for you: regenerate your workflow.** A manifest written before this upgrade still runs.
It is refused by a namespace that enforces `restricted`. See [cluster-runs.md](cluster-runs.md#a-restricted-namespace).
