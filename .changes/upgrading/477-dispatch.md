### `yidam dispatch` runs an agent seat as a run

**`yidam dispatch <question> --seat <name>` runs the elector a seat declares in `.yidam/sangha/dispatch/<name>.toml` (#477).**
The position lands on a `propose/*` branch with a receipt of what ran. The seat's registry row must already record that model, version and config hash.

**What changes for you: nothing, unless you dispatch a seat.** `.yidam/sangha/dispatch/` is new and absent by default.
`yidam lint` gains `elector-receipt-disagrees`, a warning that is silent until a dispatch receipt exists.
See [cli-reference.md](cli-reference.md) for the declaration's shape.
