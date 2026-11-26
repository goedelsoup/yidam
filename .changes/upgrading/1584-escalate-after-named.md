### `lint` names a set `escalate_after`

If `.yidam/config.toml` sets `[lint] escalate_after`, `yidam lint` now opens its report with one line.
The line names the key, its value and its file.
`--format json` carries the value as `gate.escalate_after`, absent when unset.
A corpus without the key sees no change.
A script that reads the text report's first line should skip this one.
See [configuration.md](configuration.md#a-set-value-is-named-where-the-build-fails).
