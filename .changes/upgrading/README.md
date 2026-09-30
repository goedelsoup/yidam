# Staged upgrade notes

Each file here is one upgrade note waiting for its release. See
[`docs/upgrading.md`](../../docs/upgrading.md) for what belongs in one.

- Name the file `<issue>-<slug>.md`, such as `1172-pdf-quotation.md`.
- Start it with the note's `### ` heading. Use no `#` or `##` heading inside it.
- Keep every sentence within 20 words. `docs/upgrading.md` is a Tier 1 page.
- Write a relative link as it will read from `docs/upgrading.md`, such as `(cli-reference.md)`.

Do not edit `docs/upgrading.md` to add a note. Every note written there landed on one line, so
two open pull requests always conflicted.

When a release is cut, `mise run file-upgrade-notes <tag>` moves every file here under
`## <tag>`. `release.sh` refuses the tag while a note is still here.
