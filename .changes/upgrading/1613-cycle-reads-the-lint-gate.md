### `cycle` blocks on a baseline that `lint` fails

**`cycle` and the editor read `.yidam/lint-baseline.yml` through the gate `lint` uses (#1613).**

`cycle` reported nothing blocked on a baseline whose `expire_record` named no decision record.
It now lists a `lint` block for it, and `cycle --strict` fails.
The same holds when the record states another `expire_after`.
A baseline that does not parse was read as empty, and is now a `lint` block too.
The editor shows that parse error once and keeps the findings it last published.
Run `yidam lint` to see the failure, then correct `expire_record` or the file.
