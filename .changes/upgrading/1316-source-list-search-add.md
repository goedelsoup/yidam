### `yidam source list`, `search` and `add`

**`yidam source add doi:…` writes a draft catalog entry from a pack (#1316).**
It follows the scheme's `then` chain and records each identifier as a location.
The entry is `obtained: false`, and its body is the pack's eight prompts, unanswered.
Each entry is its own `catalog:` commit. `--fetch` then runs `catalog-fetch` on it.

**`yidam source search <pack> <query>` prints the identifiers a pack's search answers with.**
A pack declares that endpoint in a new `[search]` table. `source check` holds it to the format.
See [Source packs](cli-reference.md#source-packs).

**Set `YIDAM_CONTACT` before asking a pack that requires a contact.**
It goes in the user agent. `source list` shows what each pack needs and whether it is set.
`--offline` answers from a pack's recorded fixtures and asks nothing.

**What changes for you: nothing, until you enable a pack.**
