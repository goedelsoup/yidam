### A run reads what a step wrote, and a kit writes it

**`yidam run` now refuses a step whose output would not read (#1501).**

Every `.yml`, `.yaml` and `.json` output must parse. The refusal quotes the failing line.
A file directly under `.yidam/computed/` must pass the rules `yidam doctor` applies to signal tables.
A computed `.yaml` is refused, because only `.yml` is read there.
Nothing is committed when a step is refused.

A step that already lands a parseable table is unaffected.
A report artifact, such as a TSV or a nested file, keeps every form it has.

**The prelude now ships a calculator kit under `.vendor/prelude/kit/`.**

It reads `$YIDAM_GRAPH` and writes a signal table, in Python or in Rust.
Add `.yidam/.vendor/prelude/kit/**` to a step's `reads` to import it.
See [the CLI reference](cli-reference.md).
