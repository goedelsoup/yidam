### `due` reports a sixth clock, and questions to another corpus have a home

**`yidam due` and `yidam cycle` now report six clocks (#1467).** The new one is `asks`. It stays
`undeclared` until you set `[due] asks_after`, so no exit code changes. A consumer that counted
the clocks, or listed their ids, sees one more.

**A question for a corpus you depend on now goes in `.yidam/asks/<package>/`.** If you kept such
questions by hand, as a folder of drafts or a table in a README, move each into its own file there
with `pin:` and `status:` frontmatter. `lint` checks the header, and `due` tells you when the
dependency has moved past an open ask's pin. The format is in `agent-conduct.md`, under "When you
have a question for a corpus you depend on." `open-questions --format json` gains an `asks` array
beside `open_questions`.
