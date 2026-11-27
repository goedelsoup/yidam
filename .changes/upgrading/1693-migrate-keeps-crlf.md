### `migrate pins`, `routes`, `scaffold` and `references` keep a CRLF file's line endings

**These four `yidam migrate` subcommands now write a CRLF file back as CRLF (#1693).**

Before, one edit rewrote every line of the file to LF, so its diff showed the whole file changed.
Now only the lines the command changes differ.
A line the command adds takes the file's own ending, which is the ending of its first line.
A file with mixed endings keeps each untouched line's own ending.
A file with LF endings is written exactly as before.
`migrate pins` also refuses a pin planned against line 0, where it used to fail on an underflow.
