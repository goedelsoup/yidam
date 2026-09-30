### A resolution record must name its rounds and positions

**New check, `resolution-deliberation-unrecorded` (#592).** It names a resolution record missing
`rounds:` or `positions:`. It also names a `rounds:` that is not a count of at least one.
PROTOCOL.md has asked for both fields since 2026-08-20. No record in any derived repository
carries them.

**Old records warn. New ones gate.** Git ancestry decides which is which. A record gates when the
commit that added it descends from the commit that put `rounds:` into your PROTOCOL.md. A
repository bootstrapped after 2026-08-20 has that line from genesis, so every record it adds
gates. A repository whose vendored PROTOCOL.md lacks the line only warns, until it re-vendors.

**The repair is two lines of frontmatter.** Write `rounds: 1` if the loop ran once. List every
position file the loop read under `positions:`. A shallow clone cannot see the ancestry, so there
every finding warns.
