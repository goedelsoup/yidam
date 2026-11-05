### `cycle` now reports stale manifest steps as owed

`yidam cycle` now reads `.yidam/capabilities.toml` the way `yidam run --dry-run` does.
A step whose receipt does not match the tip is listed under `stale_steps`.
It adds one `yidam run` act to `next`.

`cycle --strict` now exits nonzero on a stale step, as it does on a due clock.
A scheduled job using `--strict` may start failing where it passed before.
Run `yidam run`, or drop `--strict` if you only wanted the report.

When the steps cannot be planned, `stale_steps_unread` says why.
A detached HEAD is the common case, so CI checkouts will see it.
