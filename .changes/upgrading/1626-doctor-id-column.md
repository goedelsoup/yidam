### `yidam doctor` lines up its check ids

**The text output of `yidam doctor` pads the check id to the longest id, so the detail column starts in one place (#1626).**

The id column was 12 wide and `run-output-stamps-input-commit` is 30.
It is now 30 wide on every line, and a remedy is indented to match.
The JSON report and the ids themselves are unchanged.
A script that reads the text output by column position should read `--format json` instead.
