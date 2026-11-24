### `doctor` dates its offers, and a decline names its record

**`doctor` separates an offer from a defect (#1512, RFC-0066).** An offer is something a
release added since you adopted yidam. An undecided offer warns and names that release.
A declined one does not warn, so `--strict` passes it.

**`[doctor.declined]` declines an offer by its id, naming a decision record.** The ids are
offer ids, not check ids. A declined offer reads `declined`, and the report counts it.

```toml
[doctor.declined]
routes = "agents-by-hand"
```

A key naming no offer is listed under `unknown_declines`, and `--strict` fails on it.

**`[schema] owner = "repository"` says this repository writes `.yidam/schemas/`.** `yidam schema`
then writes nothing, and `--force` refuses. The `cli/v0.17.0` note (#1057) was wrong for such a repository.
It said to run `--force`, which replaces your schemas with the generic ones. A guard task
around `mise run schema` is no longer needed.

**`expire_after` in `.yidam/lint-baseline.yml` now wants a record.** Write one in
`.yidam/decisions/` stating the same `expire_after` in its frontmatter. Name it as
`expire_record`, and nothing else is owed. Without one, `lint` prints a warning. A record
stating another number fails the ratchet.

**`--bless` keeps the baseline's argument block, and each entry's `note:`.** You no longer restore
them by hand after a bless.
