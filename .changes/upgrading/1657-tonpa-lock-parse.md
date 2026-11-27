### A tonpa.lock that does not parse is an error

**A `.yidam/tonpa/tonpa.lock` with a syntax error no longer reads as a lock that pins nothing (#1657).**

`yidam doctor` fails its `corpora` check and names the file, line and column.
It used to warn that every fetched dependency was declared but never pinned.
`yidam gather` refuses the file too.
The `tonpa` subcommands already refused it, and now name the line on one line.
`due`'s asks clock still reads it as no lock, and `doctor` is what reports it.
A repository with no `tonpa.lock` still has nothing locked, and nothing changes for it.
