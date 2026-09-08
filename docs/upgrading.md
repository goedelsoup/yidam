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

## cli/v0.11.0

### `score` says why the criteria are the template's, in four states

`score` read a range's kuten at the range's tip. Then it wrote a sentence about the
*repository*. A range ending before your adoption said you hold no kuten. So did a range whose
declared profile is not vendored. That state is one `kuten check` and `doctor` both warn about.

**Who this affects.** Anyone scoring a range older than their adoption. That is nearly every
range: both early adopters declared in their last two commits.

**What changes.** The report names one of four states. The unreadable-profile state is now a
warning. `--format json` gains `source`, `declared` and `holds_now`.

**One JSON field narrows.** `revision` used to carry what the range's tip declared, even where
the criteria were the template's. A record could read `held: false` beside `revision: 1`. It is
now null whenever `held` is false, and what the tip declared is under `declared`.

### `query`'s rejection codes: three you may be matching on have never fired

`rejected.code` is frozen so a client can branch on it without matching prose. The frozen
list and the codes `serve --mcp` actually emits had never been compared. They disagreed in
both directions for nine contract versions.

**Three frozen names are `yidam lint` check ids, and no query rejection has ever carried
one** — `unknown-property`, `unlicensed-edge`, `edge-target-class`. They are gone from the
list. Five the server does emit were frozen nowhere and are now in it: `undeclared-property`,
`unlicensed-hop`, `unsatisfiable-predicate`, `unordered-property`, `anchor-across`.

**Who this affects.** Anything branching on `rejected.code` — a custom MCP client, or a
script reading `yidam query --format json`.

**Why this is more than a spelling.** `unknown-property` and `undeclared-property` are the
same thing to a person and different strings to a `match`. The `else` arm of such a branch is
usually *unknown rejection*. So a misspelled property name arrived unclassified at the caller
the code set exists to serve. Silently, and in the direction that hides it.

**The repair.** Compare your match arms against the list in
`yidam/prelude/sdks/parity/mcp/tools.json`. Arms for the three removed names are dead code;
add the five. `diagnostics[].code` is enumerated in the same file for the first time. A
client can branch on that too, rather than matching its prose.

### `yidam propose` records a question instead of writing a sentence

A question this tool opened used to be a paragraph spliced into `description:`. It was found
again by searching the prose for the sentence it had written. It is a record under the node's
`yidam:` key now. The key is a digest of the check and the finding's words.

**Who this affects.** Any corpus that has run `yidam propose`.

**What gets better.** A question survives an author rewriting the node's prose — it is closed
by id. A carried question stops being counted among the corpus's own open questions. And a
node whose `description:` is a plain scalar can now be asked about at all. It used to be
refused: a paragraph could not be appended without reformatting a line.

**Nothing strands.** Paragraphs written by an earlier release are still recognised and still
closed when their finding goes away. Lifting them into records is not automatic; the questions
stay askable and closable either way.

**One number moves.** The `[open]` claim counts drop by however many questions this tool had
carried. Those were never the corpus's own. `yidam open-questions` still lists the nodes, so
what shrinks is the claim tally and not the worklist.

### Prose is what the ontology declares, and `description` is no longer required by name

`node-too-long` and `missing-description` read `description` and nothing else, while the claim
counter reads the whole file. A corpus keeping prose in a `summary` or a `findings` was
measured on a fraction of what it wrote. One reads 21 lines a node against a true 34. And a
node with a `summary` and no `description` was reported as having nothing said about it.

A class now declares which of its top-level keys hold prose. The corpus may declare it once
for every class:

```yaml
# <class>.ont.yml
prose: [findings]

# universal.yml
prose: [summary, findings]
```

The effective set is `{description} ∪ universal ∪ class`. `description` never has to be
listed and naming other keys does not unname it.

**Who this affects.** Any corpus writing prose in a top-level key other than `description`.
Nothing changes for a corpus that does not: absent both declarations the set is
`[description]`, exactly as before.

**What changes when you declare.** `node-too-long` counts more lines, so findings may rise —
this is the number the ceiling was always about. `missing-description` findings fall.
`yidam embed` composes the full prose, so **re-run `yidam index-build`**. An index built
before this embedded a node's label and its edge names. It did not embed what the node said.

`description` has also been dropped from the node schema's `required` list. The check no
longer asks for that key by name. An editor will stop underlining a node that says what it
knows in a field the corpus declared.

### A property can hold prose too

The declaration above reaches a top-level key. Most prose that is not in `description` is not
there. Measured over sixteen corpora and 2,763 nodes: 68.6% hold a nested block scalar. 83% of
that sits under `properties:`. A `method` or a `verbatim` is the node
saying something. Nothing that read prose could see it.

A property now says so itself:

```yaml
# <class>.ont.yml
properties:
  - name: method
    type: string
    prose: true
    description: How the figure was computed.
```

**Who this affects.** Any corpus keeping prose in a property. Nothing changes for a corpus that
flags none: absent means false, exactly as `required:` is.

**What changes when you flag one.** `node-too-long` counts those lines. On one public corpus,
flagging 14 declarations moved it from 58 findings to 94. That is the length those nodes
always were.
`missing-description` stops reporting a node whose substance is a transcription in a property.
And `yidam embed` composes the property, so **re-run `yidam index-build`**. On that corpus the
flag adds 10.4% more prose and rewrites the text of 380 nodes of 694.

**What it does not reach.** Prose under `links:`, and prose nested under a coined top-level
key. Together those are 17% of the nested prose measured. The first belongs to the edge
vocabulary. The second needs a class contract that describes the key. Neither is in this
release.

A baseline holding the old counts still passes, so nothing goes red. It also leaves room for
regressions that nothing will report.

### The `inquiry` kuten is at revision 2, and two bands are gone

`nodes-per-commit` and `median-node-lines` measured how old a repository is, not how it works.
Nothing read them at matched maturity, and every member of the fit exits both bands by ageing.
One left both within six hours of the fit that used it.

**Who this affects.** Any repository holding `inquiry`. Nothing is broken and nothing gates.

**What changes.** `kuten check` reports three findings where it reported five. The `AGENTS.md`
block loses its **Shape** line. The two numbers are still measured and still in the report; no
band judges them.

**The repair.** Re-vendor, then record a superseding decision:

```sh
YIDAM_REF=v0.5.0 mise run yidam-vendor-update
```

Until you do, `doctor` warns that your record names revision 1 and the vendored profile is at 2.
That is the revision model working, not a fault.

### `kuten check` rows read in one unit, and a divergent row says which side

A row read `declared 0.12–0.27  measured 12%` — two halves of one comparison, in two units.
Worse, `12%` is inside `0.12–0.27` when you convert it, and the row was reporting divergence.

**Who this affects.** Anyone reading `kuten check`, and any consumer parsing its JSON.

**What changes.** Bands render in the value's unit: `declared 12%–27%`. Shares carry a decimal:
`measured 11.9%`. A divergent value names the side it fell on: `11.9% (below 12%)`.

**Why the last one.** The verdict compares a float and the display was rounded. So a divergent
row could show a number that reads as inside its own band. Precision alone cannot close that;
naming the side can.

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

Where there is no `AGENTS.md` at all, it writes none and says so. It prints the section for
you to paste. `yidam doctor` reports it until a file carries the block.

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

### `lint --commits` can be scoped to the corpus register

A repository with an artifact beside its corpus has two commit registers, and yidam modelled
one. A `feat:` on `web/` was reported as a corpus-vocabulary violation.

Name the artifact's paths and it no longer is:

```toml
[object]
paths = ["web/**", "crates/**"]
```

**Who this affects.** Nobody who leaves the key unset. The default is unchanged.

**One asymmetry, deliberate.** `yidam vocabulary --check` reads a subject line before the
commit exists. It cannot ask git for paths, so it still warns where `lint` now stays quiet.
Its callers are a contributor at a terminal, the VS Code commit box, and the MCP tool
`check_subject`. Tracked as #652.

*Corrected 2026-09-07 (#693): this note first named a commit-msg hook, which this project does
not ship.*

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
