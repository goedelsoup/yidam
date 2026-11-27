### A tonpa.toml that does not parse is an error

**A `.yidam/tonpa.toml` with a syntax error no longer reads as a file declaring no dependencies (#1616).**

`yidam doctor` fails its `corpora` check and names the file, line and column.
`yidam lint` refuses to run until the file parses.
`yidam gather`, `query --dependency` and `derive check` refuse it too.
`migrate pins` reports it as blocked.
A repository with no `tonpa.toml` still declares nothing, and nothing changes for it.
A file that was quietly ignored before now stops these commands, so run `yidam doctor` after upgrading.
