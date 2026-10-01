### A cluster corpus deploys with one `kubectl apply -k`

**The objects around a generated workflow are now a Kustomize base and overlay (#1229).**
The service account, both git secrets and the vault claim come from `yidam/cluster/`.

**What changes for you: `docs/cluster/argo-rbac.yml` is gone.**
Its objects live in the base, which your overlay names by URL at your `cli/v*` tag.
Copy the streamflow overlay, set its prefix to your corpus, and add your key files.
Then run `kubectl apply -k`. See [cluster-runs.md](cluster-runs.md#deploy-a-corpus).
Objects you already created keep working, since the names are unchanged.
