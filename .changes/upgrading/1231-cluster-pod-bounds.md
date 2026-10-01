### Every generated cluster pod is bounded, and every run is cleaned up

**`yidam cluster workflow` now writes resources, a deadline, retries and cleanup (#1231).**
Each pod requests 250m CPU and 512Mi memory, is limited at 2 CPU and 2Gi, and fails after an hour.
`pin`, `survey` and `step` retry twice. `land` never does. Argo deletes a pod that succeeded,
and a workflow a day after it succeeds or a week after it fails.

**What changes for you: regenerate your workflow.** Set other sizes in `[cluster.pod]` and
`[cluster.cleanup]`. A calculator can set its own under `[capability.<name>.cluster]`.
See [cluster-runs.md](cluster-runs.md#resources-deadlines-and-cleanup).

**Update the image first.** The pods read `.yidam/config.toml` and `.yidam/capabilities.toml`.
An older binary refuses either file once it holds one of these tables.
