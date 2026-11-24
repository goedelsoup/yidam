### kuten check reads the corpus register

**`kuten check` and `practice` leave out a commit touching only `[object] paths` (#1507, RFC-0062).**

They read the same population `lint --commits` already reads.
`cohort` still counts every authored commit.
A corpus with no object declared reads exactly as before.
A path under `.yidam/` is corpus whatever the globs say.

**The verdict names what it left out.**

A line reads `register: 1432 object-only commit(s) left out ([object] paths)`.
The same line goes on to the unscoped phase and off-vocabulary shares.
The `--json` measurement gains four `object_only_` counts, each 0 with no object.

**A `PRACTICE.md` REGEN block may go stale.**

Its figures now count the scoped population, so run `yidam regen` once.
See [RFC-0062](rfcs/0062-kuten-reads-the-corpus-register.md) for the measurement behind the change.
