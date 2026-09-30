### `export --format llms` prints instead of writing `llms.txt`

**Stdout by default (#919).** Without `--out`, `yidam export --format llms` wrote `llms.txt` at
the repository root. A reader who only meant to look at the corpus got an untracked file.

It now prints the pack to stdout. The summary line goes to stderr. With `--out`, nothing
changes. A script that reads `llms.txt` after the export needs `--out llms.txt`.
