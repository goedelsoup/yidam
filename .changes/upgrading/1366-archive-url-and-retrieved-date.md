### `catalog-fetch --archive` asks for the capture nearest the retrieved date

**The Wayback lookup now sends the `retrieved:` date of the bytes the entry holds (#1366).**

A re-run can add a different, older capture than before. New bytes still ask for today's.
The added location's description names the date asked for beside the capture date.
`--archive=url` also looks up `kind: url` locations. Bare `--archive` is unchanged.
A run that finds nothing to look up now says so on stderr.
See [`catalog-fetch`](cli-reference.md).
