### A re-vendor takes the newest release, not the origin's HEAD

**`yidam-vendor-update` now pins the newest `cli/v*` release by default (#1308).**
A release is a pin `yidam-build` can download, so no compiler runs.
An untagged pin compiled the CLI from a clone, in every repository that took it.

**What changes for you: your next re-vendor may land on a release, not on main.**
To keep pinning unreleased main, run `YIDAM_REF=HEAD mise run yidam-vendor-update`.
A repository pinned ahead of the newest release is refused rather than rolled back.
The refusal prints both commands: stay on main, or take the release on purpose.

`yidam-vendor-status` and the CI staleness report now measure against the newest release too.
