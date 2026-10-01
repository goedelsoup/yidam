### `cluster status` reports what a run did, from records

**`yidam cluster status` lists a corpus's recent cluster runs (#1236).** For each run it gives the admission and its reason.
Each step is landed, proposed, refused with the lander's reason, or not reached. Each gather peer has its outcome.
It reads the records Argo keeps on each `Workflow`, through `kubectl`, and never a log.
See [cluster-runs.md](cluster-runs.md#when-something-looks-wrong).

**A refused landing now writes a record to `--out`.** The record holds `refused` and the lander's reason, and the pod still fails.

**What changes for you: nothing, until you upgrade the image.** A lander from an older image writes no refusal record.
`status` shows its refusals as `failed`, pointing at the pod's message.
