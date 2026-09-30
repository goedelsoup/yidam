### `yidam cluster` runs the manifest on Argo Workflows

**`yidam cluster workflow`, `admit`, `pin`, `step`, `land` (#475, RFC-0026 §7).** The same
run as `yidam run`, one pod per act. `workflow` generates the Argo manifest from
`.yidam/capabilities.toml` and `[cluster]` in `.yidam/config.toml`. The step pod holds no
credential that can move a ref; only the lander does. [cluster-runs.md](cluster-runs.md) is the
deployment guide, for a `file://` vault that needs no credentials and for `s3://`.

**What changes for you: nothing, unless you deploy it.** `[cluster]` is a new config table and
is absent by default. No existing command changed its behaviour.

**Every workflow also runs the catalog.** `catalog-fetch`, `catalog-extract` and `catalog-reconcile`
run ahead of your manifest's steps. A manifest that declares one of those names has no workflow.
Rename the capability.

**Receipts are now `format_version: 2`.** A receipt can record `version`, and `image_digest` on a
cluster. The digest is recorded only for an image pinned by `@sha256:`. Version 0.17.0 reads these
receipts unchanged.
