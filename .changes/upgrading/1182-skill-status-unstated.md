### `lint` reports a skill that does not say whether it is built

**`yidam lint` now reports `skill-status-unstated` at `Info` (#1182).** It names each skill in `.yidam/skills/` whose `status:` is missing.
It also names a skill whose value is neither `built` nor `stub`.

Every skill written before #1063 is reported after this upgrade. Nothing gates on it.
Add `status: built` or `status: stub` to each one, then run `yidam skills-index`.
