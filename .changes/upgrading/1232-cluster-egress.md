### Cluster pods are labelled by what they may reach, and a new command fences them

**`yidam cluster workflow` labels every pod, and `yidam cluster network-policy` writes the
policies that select them (#1232).** Each pod carries `yidam.dev/corpus` and `yidam.dev/egress`:
`remote`, `vault` or `internet`. A step pod, where a calculator runs, reaches the vault and
nothing else. `catalog-fetch` now runs from its own template, `step-catalog-fetch`.

**What changes for you: regenerate your workflow.** Then set the API server's addresses in
`[cluster.egress] executor` and write the policies into your overlay. An `s3://` vault also
needs `vault`. See [cluster-runs.md](cluster-runs.md#limit-what-each-pod-reaches). Without a
CNI that enforces NetworkPolicy, the policies apply and enforce nothing.

**Update the image first.** An older binary refuses `.yidam/config.toml` once it holds
`[cluster.egress]`.
