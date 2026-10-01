### Regenerate a cluster workflow

**A workflow from `yidam cluster workflow` stopped after its first landing (#1237).**
Its expressions named hyphenated tasks with a dot, which Argo could not resolve. Nothing failed, and the run waited.
They now name each task in brackets, as `tasks['land-x']`.

**A run not admitted now ends Succeeded, not in Error.** Each task waits for the one before to succeed.
A skipped `pin` used to let the first step start and read an output nothing wrote.

**A `CronWorkflow` now lists its time under `spec.schedules`.** Argo Workflows 4 refuses the old `spec.schedule`.
Argo 3.6, the oldest version the run supports, reads either form.

**What changes for you: regenerate every committed workflow.** Run `yidam cluster workflow` again with the flags you used.
