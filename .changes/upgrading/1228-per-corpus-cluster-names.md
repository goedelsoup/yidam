### Each corpus on a cluster names its own secrets and service account

**`yidam cluster workflow` now names every object for its corpus, as `yidam-<corpus>-<role>` (#1228).**
Two corpora in one namespace used to share `yidam-git-write`, so either lander could move the other's refs.

**What changes for you: a regenerated workflow refers to new names.**
Your cluster still holds `yidam-run`, `yidam-git-read`, `yidam-git-write` and `yidam-vault`.
Recreate each under its new name, and apply the Kustomize overlay in [cluster-runs.md](cluster-runs.md#deploy-a-corpus).
Or keep the old names by setting them in `[cluster.names]`. See [configuration.md](configuration.md#cluster).
Keep old names only when one corpus runs in the namespace.
