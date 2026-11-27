### due's asks clock names a tonpa file it could not read

**`yidam due` now says when its asks clock could not read `tonpa.toml` or `tonpa.lock` (#1665).**

The clock still answers, and `due` still exits zero.
Its line used to give a count as if both files had been read.
With the lock unreadable, each pin is the declared `rev`, so an ask can read as moved.
With `tonpa.toml` unreadable, no dependency is declared, so no ask is compared.
The line now adds `could not read <file>` and which of those two happened.
With `[due] asks_after` set, a count that would read `ok` reads `unmeasurable` instead.
`yidam doctor` still fails its `corpora` check over the file and names the line.
A repository with no lock, or no open asks, sees no change.
