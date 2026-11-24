### tonpa pins a dependency's export, and `derive check` reads it at that pin

**A pinned path dependency may declare `export = "<file>"` (#1508, RFC-0063).**

`tonpa add <path>@<rev> --export <file>` reads the file at the pin.
It unpacks it to `.yidam/tonpa/<name>/export/` and hashes it in the lock.
It is refused when the corpus moved after the commit that last wrote the file.
With an export, `tonpa update` moves to the commit on HEAD's history that last wrote it.
`tonpa verify` fails an export the lock does not hash, and `tonpa install` hashes it.

**`query --dependency <name>` answers from a pinned path dependency at its locked commit.**

It reads exactly as `--root <path> --at <rev>` does, both taken from tonpa.

**`[[derive.records]]` holds a JSON file's span records as citations.**

Each record's node must resolve, and its span must be in the node's file.
Its governing quote must sit in one paragraph that carries a standing tag.
A pointer matching nothing is `derive-record-unmatched`, an error.
See [Configuration](configuration.md#derive-records) for the mapping's keys.

**`[derive]` now refuses unknown keys.**

A misspelt key under `[derive]` fails every command that reads the config.
Fix the key, or remove it.

**`derive check --format json` gains `records`, the number of records matched.**

Hold it above zero in CI, so an older CLI that reads no records fails.
It is `0` with no mappings, and no other output changes.
