### A push can submit a cluster run, and every run holds a per-corpus mutex

**`yidam cluster workflow --on-push github` writes an Argo Events `EventSource` and `Sensor` (#1235).**
A push to the branch submits the same run the cron does, and the run asks `admit` first.
The lander's own push submits a run too, and that run finds nothing owed.
See [cluster-runs.md](cluster-runs.md#run-on-push).

**Every generated workflow now holds a mutex, `yidam-<corpus>`.** Two runs of one corpus queue instead of racing.
The field is `synchronization.mutexes`, which needs Argo Workflows 3.6 or later.

**What changes for you: regenerate your workflow, on Argo Workflows 3.6 or later.** A manifest written before this upgrade still runs, without the mutex.
Running on push is opt-in. It needs Argo Events and the `streamflow-on-push` overlay's two extra objects.
