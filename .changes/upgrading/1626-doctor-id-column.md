### `yidam doctor` lines up its check ids

**`yidam doctor` pads every check id to the longest id, so each detail starts in one column (#1626).**

The id column was 12 wide and `run-output-stamps-input-commit` is 30.
It is now 30 wide on every line, and a remedy is indented to match.
The JSON report and the ids themselves are unchanged.
A script that reads the text output by column position should read `--format json` instead.
