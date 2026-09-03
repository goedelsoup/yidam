# Upgrade notes

*What changes when you move to a new release, where a version number cannot say it.*

Most releases need nothing here. Semver carries compatibility, `--generate-notes` carries the
list of what changed, and the four layers in [versioning](versioning.md) carry which of them
moved. What none of those can carry is the sentence a person needs before upgrading: **your
working setup will behave differently, and here is the repair.**

A change belongs here when a configuration that works today stops working, or starts
working differently, without the person having changed it. A new feature does not; a bug fix
usually does not; a fix that makes a previously-quiet misconfiguration loud does.

## How this reaches a release

Notes are written under `## Unreleased` while the next version number is still unknown, and
filed under the exact tag when it is chosen — `## cli/v0.9.0`. At release time
`.github/workflows/release.yml` prepends this file's section for the tag it is publishing to
the generated notes, so the note appears in the release itself rather than only here.

`release.sh` **refuses a tag while `## Unreleased` still has content.** A note staged under
that heading is one somebody wrote for a release and did not file, and the two ways it can go
wrong are both silent: it is dropped from the release being cut, or it is repeated into the
next one. The repair is to rename the heading to the tag.

<!-- Keep the `## Unreleased` heading even when nothing is under it: release.sh reads
     it, and a missing heading reads as "no note" for every release from here on. -->

## Unreleased

## cli/v0.9.0

### `serve` refuses a directory that is not a corpus

`yidam serve --mcp` and `serve --mcp --http` now fail at the command when started somewhere
with no `.yidam/` directory, instead of starting and serving an empty corpus.

**Who this affects.** Anyone whose MCP client launches the server from a directory other than
the corpus. That configuration was always wrong and never said so, which is why this is worth
reading rather than merely noting: **the symptom changes from a server that answers every
question with nothing into a server that does not come up.**

The old behaviour was worse than "serves nothing". The handshake for a directory that was not
a corpus was identical in shape to one for a repository bootstrapped an hour ago — `nodes: 0`,
`skills: 0`, `decisions: 0` in both — and the only field that differed, `domain`, was derived
from the directory's own name. An agent had no way to tell "this corpus is empty" from "this
is not a corpus", and the warning that would have said so went to stderr, which an HTTP client
cannot read.

**The repair.** Point the client at the corpus. For a client that starts its servers somewhere
else, pin it:

```json
{
  "mcpServers": {
    "yidam": {
      "command": "sh",
      "args": ["-c", "cd /abs/path/to/my-corpus && exec yidam serve --mcp"]
    }
  }
}
```

The [Claude Code plugin](mcp-server.md#claude-code-as-a-plugin) does this for you and checks
before it spawns.

**What is not affected.** A repository that has been bootstrapped and has nothing written into
it yet is still served. The test is `.yidam/`, not corpus content — an empty corpus is a
legitimate corpus, and saying otherwise to its author is the one thing this change must not
do.

See [#549](https://github.com/goedelsoup/yidam/issues/549).
