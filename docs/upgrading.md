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

## cli/v0.10.0

### A corpus can declare what its practice is aimed at

`yidam kuten` is new. A kuten declares what this corpus's work is **for**, as the ontology
declares what it is *about*.

It ships as vendored prelude and carries a revision. A decision record adopts it. Holding
none is a supported state, and it reports as one.

**Who this affects.** Every repository, on its next re-vendor. Nothing changes until you
adopt one.

**The repair.** Nothing is broken. To adopt, re-vendor and then declare:

```sh
YIDAM_REF=v0.4.0 mise run yidam-vendor-update
yidam kuten adopt inquiry
```

`adopt` copies the revision out of the vendored profile rather than asking for it. It also
adds the `AGENTS.md` section and fills it, because the scaffold carrying that section is
consumed at genesis.

Then `yidam kuten check` reads your history against what you declared. It writes nothing and
exits zero however far a corpus has drifted.

### `yidam score <range>` reads a session's work

The genesis rubric scores a repository's birth, once. Nothing said whether a *session's* work
was any good.

`score` reports one row per declared criterion, each with the evidence it came from. There is
no overall number: a single score over a range names somebody's session.

Criteria come from the kuten's `rubric` slot, or from the template where no kuten is held. The
report says which, every time.

### `node-too-long` counts the description, and your baseline is now loose

The check counted the whole file — frontmatter, properties and links included — while its own
rationale argues about prose. The two came apart once corpora began recording where their
edges come from.

One derived corpus reported 212 findings, and 207 came from a single class. Its fixed
structure runs 18 lines before a word of prose.

**Who this affects.** Any corpus whose nodes record provenance on their links. Findings fall,
often sharply.

**The repair.** Re-bless after upgrading:

```sh
yidam lint --bless
```

A baseline holding the old counts still passes, so nothing goes red. It also leaves room for
regressions that nothing will report.

### `lint --commits` can be scoped to the corpus register

A repository with an artifact beside its corpus has two commit registers, and yidam modelled
one. A `feat:` on `web/` was reported as a corpus-vocabulary violation.

Name the artifact's paths and it no longer is:

```toml
[object]
paths = ["web/**", "crates/**"]
```

**Who this affects.** Nobody who leaves the key unset. The default is unchanged.

**One asymmetry, deliberate.** `yidam vocabulary --check` runs in the commit-msg hook, before
the commit exists. It cannot ask git for paths, so it still warns where `lint` now stays
quiet. Tracked as #652.

### A citation naming a line is held to what the line says

`lint` now checks `#L42` fragments on prose links, reading the quoted passage against the
lines cited.

**Who this affects.** Repositories whose prose cites files by line number. Most do not.

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
