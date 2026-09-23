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

### `yidam serve --lsp` lints an unsaved buffer as a node

**A buffer whose file does not exist yet is now checked (#607).** Before, no check saw it. Its
findings were none, which is what a clean node's are. Now it is a node, if its path is one the
corpus walker would accept.

**Your editor may show new findings on a new file.** They are the findings `yidam lint` would
report once you save. Nothing changes for a buffer whose file is on disk.

**The server declares it.** The `initialize` result carries
`experimental.yidam.unsavedInstances: true`. A client that needs this can check for it.

### A seat can say what it holds, and `yidam lint` reads it across the seat's own history

**Four new findings about `ma/*` branches (#294).** A seat may now keep
`.yidam/sangha/commitments/<elector>.md` on its own branch. It carries two sections, `## What
this seat holds` and `## What this seat has withdrawn`. Each item links the position that
argued it. The file is never transported onto the baseline.

**One new Info finding fires whether or not you adopt it.**
`elector-commitments-absent` reports a seat that has filed a position and carries no
commitments file. A repository with three such seats gains three Info findings and nothing
else. Your exit code does not change. The other three are silent until a commitments file
exists.

**Two of the other three gate, and only over a file you wrote.**
`elector-commitments-malformed` is an Error when the file does not carry both headings. A file
with no headings parses to two empty sections, which would turn the next check off.
`elector-commitment-vanished` is an Error when a ground disappears. A position named under
`holds` at one commit, and in neither section at a later one, is the finding. Deleting the file
counts, because it takes the grounds it held with it. `elector-position-unindexed` is Info and
reports one of the seat's own positions that neither section names.

**The item's identity is the position it links, never its prose.** Reword an item freely. What
must not disappear is the link. Moving it to `## What this seat has withdrawn` is always the
answer. Withdrawing a ground is the act the loop exists to produce, and is never reported.

**The commitments file is read from the branch, so a thin clone reports less rather than
differently.** A checkout without the `ma/*` commits sees no seats at all, like
`resolution-independence-mismatch`. Fetch the elector branches to get the real answer in CI.

### `yidam lint` now derives a resolution's `independence:`

**New Info finding, `resolution-independence-mismatch` (#823).** It reports a record whose
stated `independence:` is not the one its seats derive. The derived value rides in the finding,
so the finding is also the repair. Info, not a gate: your exit code does not change.

**It is silent unless your registry binds `Kind`, `Model`, `Version` and `Config`.** Without
those columns every record derives `unrecorded`. A record that states nothing over an
`unrecorded` derivation is passed over. Filling the columns in is what arms it.

**The registry is read at each seat's own tip, not at HEAD.** A checkout without the `ma/*`
commits reads `unrecorded` rather than falling back. `--depth 1` and `--single-branch` are both
that checkout. So a record that *does* state a value will report there. Fetch the elector
branches to get the real answer in CI. `resolution-scope-unverifiable` already warns about the
same thin clone.

**`yidam sangha --json` gains `resolutions[].independence`**, a string, empty where the record
carries none. Not required by the schema, for the reason `synthesized_by` is not.

## cli/v0.13.0

### Several corpora can share one vector index, and a query can ask across them

**MCP contract 0.23.0 → 0.24.0.** `retrieve` takes an optional `corpora` argument, every
response carries `scope`, and every result carries `corpus` (#835). A server built against
0.23.0 answers every call unchanged. A client built against it never asks to span, so nothing
it does today behaves differently.

**Nothing on disk or in a bucket moves.** Vector keys have carried the corpus since the first
push. So has `corpus`, as a filterable metadata key. An index several corpora already push to
can be asked a spanning question, with no re-push by any of them.

**Only a remote index can answer one.** A local `.yidam/index/` is one corpus by construction.
A call that asks to span it is not an error. It is not silently honoured either: it answers
locally and reports `scope: "local"`. Compare `scope` against what you asked to know whether
the span happened.

**A result from another corpus cannot be fetched.** Its `id` is the reference grammar's
absolute form: `yidam://<corpus>/node/<class>/<name>`. `get_node` cannot resolve it, because
that corpus is not installed here. Its `text` is all you get. `corpus` is null for
this server's own rows, so a client can tell the two apart without parsing anything.

**`corpus` is not `origin`.** `origin` names an installed dependency under `.yidam/tonpa/`,
whose nodes this server read and whose ids it resolves. A corpus that merely shares an index is
neither. One word for both would say a row is followable when it is not.

### `yidam retrieve`, and one vector space per index

**A new command, and a new refusal from `index-push`.** Neither changes a corpus. `yidam
retrieve <words>` is the retrieval `serve --mcp` performs, dispatched through the same tool
call, with `--k`, `--class` and `--corpora`. It had no terminal route before.

**`index-push` now refuses an index built in another vector space.** An index carries one
embedding contract and every push overwrites it. Two spaces in one index would let a spanning
query rank one corpus's rows against the other's. The scores would look ordinary. If your push
starts refusing, that index already holds a contract your `embed.config.json` disagrees with.
Push to a different index, or rebuild with the settings it declares.

**Where the contract cannot be read, the push proceeds and says so.** A write-only credential
is a legitimate shape for a pusher. So a 403 on `GetVectors` is a printed note, not a
refusal. A push also now reports the other corpora it found in the index, with a row count
each. `--dry-run` is the read-only way to ask.

### `yidam migrate findings` lifts the questions an earlier `propose` wrote

A finding is a record under the node's `yidam:` key. It used to be an English sentence appended
to `description:`. It was found again by grepping the prose for its opening words. Paragraphs an
earlier release wrote are still recognised and still closed. **Nothing was stranded, and nothing
here is urgent.** This is the one-time lift (#728).

```
yidam migrate --dry-run findings   # what it would do
yidam migrate findings             # do it
```

**Two numbers move, and only for a corpus that runs it.** The `[open]` claim counts **drop**. A
carried question stops being counted among the claims the corpus makes. That is the whole point
of the record. `open-questions` does **not** change: the record is still an open question on the
node.

A corpus publishing a claim tally in a generated block will see that tally fall. Roughly by the
number of questions `propose` has open. Re-run `yidam corpus-index` after the lift.

**A paragraph you reworded stays prose.** The lift matches the marker, the check in brackets and
the fixed framing sentence exactly. A sentence you rewrote is yours, and this will not guess where
the tool's words ended. Those are counted in the report and left alone, `[open]` tag included. So
a reworded paragraph keeps counting as a claim until you decide otherwise.

**A catalog entry with no frontmatter is refused rather than lifted.** Removing the paragraph
there would leave nowhere findable to put the record. The run reports the file and writes
nothing at all.

The migration is idempotent and writes a record under `.yidam/migrations/`.

### A `retrieve` result says whether its text is all of it

**MCP contract 0.22.0 → 0.23.0.** Every entry of `results` carries `truncated`, a boolean
(#853). It is `false` on keyword search and on a local index. Neither has a per-row ceiling to
cut against. Only a corpus served out of a remote vector index can answer `true`. A server built
against 0.22.0 still answers every call. A client built against it reads a cut row as a whole
one, which is the state this ends.

**A remote index stores each row's text under a 40 KB ceiling.** `yidam index-push` already cut
the rows that exceeded it, and already said so in its report. What was missing was the other
end. The flag travelled back with the row and `retrieve` dropped it. So half a document rendered
exactly like all of one. Nothing about what is stored changes here, and no index needs
rebuilding.

**Expect almost no `true`.** Across sixteen measured corpora, one row in 3,246 is cut. It is a
catalog source rather than a node. A node's text is the fields somebody wrote; a source's is a
whole markdown document that no format caps. A remote index over catalog sources is where this
bites. It is where you find out which of them you have been reading half of.

### An edge may no longer assert a standing its endpoints contradict

`edge-standing-unheld` is a new `warn` (#858). It reports an edge asserting a standing **stronger**
than one its own endpoints declare. `claim_tag: verified` between two nodes graded `[open]` claims
more about the relation than the corpus claims about either end.

**It reports in every corpus, with no declaration**, like `edge-verified-unsourced`. Writing the
tag is the opt-in. A relationship listed under `structural:` is not exempt. Bookkeeping that
asserts more than the nodes it files has stopped being bookkeeping.

**A node's standing is a property its class declared `type: claim`.** Nothing else. Not the
weakest marker in its prose. A synthesis node carries all three tags by design, so reading its
weakest would grade your best-made nodes `[open]`. A corpus whose classes declare no claim-typed
field therefore sees nothing here, however its edges are tagged.

**It is one-directional.** An `open` edge between two `verified` nodes is never reported. Such a
corpus knows both things and does not say they are related. That is the vocabulary working.

**This check's population is not empty by construction, and it is the only one here.** That is why
it is `warn`, and why it stays `warn`. Both sides of the comparison are opted into separately. So a
corpus can adopt edge tags on nodes graded years earlier, and inherit findings it did not write.
The repair is a demotion on the edge, or a promotion on the node with a reason. Nothing proposes
the promotion for you. `yidam lint` reports more findings and `mise run ci` passes.

### An edge's standing is now counted and listed, and where it used to be counted it was the node's

`claim_tag` on a link was graded by two lint checks and read by nothing else. `open-questions`,
`status`, `corpus-index` and the MCP `claims` and `open_questions` tools now read it too (#857).
A corpus that tags no edge sees none of this. Every figure and every table below appears only
where there is something to report.

**A bracketed edge tag stops being counted as a node's claim.** A link writing `claim_tag:
"[open]"` was visible to the byte scan over the node's file. That is the spelling a corpus
adopts after being told the prose scan needs brackets. So the edge's standing was already being
reported — as an `[open]` in the node's own prose. It now belongs to the edge.

If your corpus writes the bracketed form on links, expect three things. `status` and
`corpus-index` will show **fewer** node claims. A new edge figure will carry the difference.
And the node will leave `open-questions`, with the edge arriving in its own right. The bare
form — `claim_tag: open` — was never counted anywhere and only gains.

**`status` and `corpus-index` grow a second figure rather than a bigger one.** `claims` still
counts a node's prose and its claim-typed properties; `edges` is the same three over its links.
A corpus-wide total is the two added, and nothing adds them for you. An edge is in no node's
text, and it belongs to two nodes at once. One merged number would answer about a denominator
nobody chose. The `edges` segment and the `Edges` column render only where some edge is tagged.
That keeps `yidam regen --check` green in a repository that has not made the distinction.

**An open edge is its own entry in `open-questions`, not a promotion of the node.** It is listed
under the node's label, with the triple beside it. The link still points at the node, because
that is the file it is written in. The JSON gains `scope` — `node` or `edge` — on every entry,
and `relationship` and `target` on the edge ones.

**MCP contract 0.20.0 → 0.21.0.** `claims` serves edge claims with `scope: edge`;
`open_questions` gains the edge arm and `scope` on every entry. A server built against 0.20.0
still answers every call, because the additions are new fields and new entries. But it
under-reports a corpus that tags edges, in exactly the way the CLI did before this.

### An edge may say what it rests on, and a `verified` one is asked for a source

A link could always be written with extra keys and nothing read them. `claim_tag:` and `source:`
are now part of the link shape, and two checks read them (#587).

**`edge-verified-unsourced` reports in every corpus, with no declaration.** A link writing
`claim_tag: verified` and no `source:` is a new warning where there was none. Tagging edges is the practice this came from, and the published schema used to
underline it as invalid. If your corpus does it, expect one finding per verified link with
nothing behind it. The repair is a `source:` or a demotion to `inference`. Which of the two is
yours to decide; nothing proposes a promotion.

**`edge-untagged` reports nothing until you ask for it.** Its population is every edge, so it
runs only where `.yidam/corpus/universal.yml` says so:

```yaml
edge_claims:
  required: true
  structural:
    - instance-of
    - concerns
    - subject-of
```

`structural:` names the relationships that are bookkeeping rather than empirical. Both keys are
new and both are optional; a corpus that writes neither is checked exactly as it was. Writing
`structural:` alone records the exemption and switches nothing on.

Both checks are `warn`, so neither reaches the baseline and neither turns the gate red.
`yidam lint` reports more findings and `mise run ci` passes.

**The editor stops underlining the two keys.** The link item in the published node schema listed
`target`, `relationship` and `note`, and closed the object. So a corpus writing edge provenance
got a red squiggle on every tagged link. Rerun `yidam schema` to pick up the new shape, and
`edge_claims:` in the `universal.yml` schema with it.

### The MCP contract has a write tier, and a server's corpus snapshot is no longer fixed at startup

The contract goes to **0.22.0** and gains one tier, `act`, holding two tools: `propose` and
`cycle`. Both are **absent from every server that has not been told to write**. That is every
server today, so nothing about an existing deployment changes until somebody writes the key.

Two things to know if you consume this surface.

**The freshness sentence narrowed, and today nothing you can read moves.** Contract 0.9.1 said
that every read came from the corpus built on disk at startup. Freshness was a restart.
That is now *startup, **or the server's own last write***.

A change made **outside** the server is still invisible until a restart, exactly as before. An
edit, a `tonpa install`, a commit from another process: all unchanged.

The second half is the part worth having before you change any client code. `propose` builds its
commits against a temporary index and creates the ref with `update-ref`. The working tree is
untouched and HEAD does not move. Every read tool answers byte-identically across the write.

So the reload is a correct no-op for the only write this tier has, and **no response observably
differs**. The rule is in the contract for two reasons. It costs one corpus re-walk on a call that
already wrote to git. And the first act tool that touches the working tree should not also have to
be a contract change.

If you cached a handshake's `corpus.commit`, it is still correct today. It is no longer guaranteed
to be.

**`act` is the one capability in the block that is permission rather than ability.** Every other
key is filled from what the server can back. This one is false unless the corpus says otherwise,
even on a server that could write perfectly well. A client must never infer it.

To turn it on, in the corpus's own `.yidam/config.toml`:

```toml
[serve]
act = true
```

And three clauses must hold, or **the server refuses to start** rather than quietly serving the
read tools:

- a git author identity resolves in the corpus being served — `user.name` and `user.email`, the
  values the commit would take. A checkout with none cannot declare `act` on any transport,
  stdio included;
- the declaration is this key and nothing else. A flag would make *may a tool write into this
  corpus* a property of whoever launched the server;
- `serve --mcp --http` binds loopback. A server reachable from another machine may not declare
  it, because no author exists for a remote caller yet.

The refusal is the point rather than an inconvenience. An operator wrote the key and got a
running server. They would read that as the answer to the question they asked.

**What `propose` may write is unchanged from the CLI.** Three verbs go onto a `propose/<head>`
branch: `open`, `withdraw`, `close`. Never onto the baseline, and nothing merges itself.

There is no `--force` on this surface. Replacing a proposal branch discards commits nobody has
reviewed, and that repair is a person's.

`yidam cycle` also ships as a CLI command, available in every build and needing none of this.

### `used-by: []` is now a claim the drift check reads

`catalog-used-by-drift` compares a catalog entry's declared `used-by` against the nodes that
cite it. It skipped that comparison whenever the list was empty. An empty list and an absent key arrived at
the check as one value. So `used-by: []` was a silent opt-out.

**An entry sitting at `used-by: []` while nodes cite it now draws a warning it did not draw
before.** Nothing in a repository changes to bring this about. It is the state a hand-authored
entry starts in. The entries most likely to be affected are the newest ones.

`used-by:` with no key at all is unchanged and still exempt. Absence declares nothing. A command that
filled it would decide an entry should make a claim its author never made.
The two states have always been different; only one of them was being read.

The gate does not go red. The check is `warn` severity, and warnings never reach the
baseline — `yidam lint` reports one more finding and `mise run ci` passes. The repair is:

```
yidam catalog-reconcile
```

which substitutes the citations for the empty list and commits as `reconcile:`. It still
leaves an entry with no `used-by:` key alone.

One thing to know if you read the report rather than the terminal. In `catalog-audit --format
json`, the `drift` field distinguishes the two states and `used_by` does not. Both render as
`[]`. `drift` is `null` for an absent key, and an object for a declared-empty one. A
client switching on `used_by.length === 0` was never telling them apart and still is not.

### `degraded_reason` has a fourth value, and it is not a property of the deployment

The MCP contract goes to **0.20.0**. `retrieve` and an anchored `query` may now report
`remote_unavailable`, alongside `no_index`, `no_vector_support` and `stale_contract`.

**A client that validates the field against a closed set of three will reject a valid
response.** Nothing in a repository changes to bring this about. The new value appears when a
corpus declares `[index.remote]` and the service behind it does not answer.

If you wrote that validation, widen it. The set is frozen in
`yidam/prelude/sdks/parity/mcp/tools.json`, which is the file to read it from.

There is a second thing to know, and it is the part a cache gets wrong. The three older values
are properties of a **deployment**: true when the server starts, true for its lifetime. The new
one describes a **call**. A server may report it on one request and nothing on the next. A
client that reads `degraded_reason` once at handshake and remembers it will be wrong about its
second question.

Nothing else moves. A corpus that declares no `[index.remote]` — which is every corpus today —
answers exactly as it did.

### `yidam regen` now populates two more blocks, so `--check` can newly fail

`vault-status` and `decisions-log` write REGEN blocks. `yidam regen` did not run either of
them, and `yidam regen --check` did not report them stale.

**Both now run, and `mise run ci` can go red on a repository nobody changed.** The README's
Artifacts block is the one to expect. Most corpora still hold the placeholder the template
ships there — the italic line telling you to run the generator. It was never checked, so it
never had to be true.

The repair is the ordinary one:

```
mise run regen
git commit -am 'regen: populate the vault-status block'
```

Read the diff before committing it. The block reports the arrangement your `.yidam/config.toml`
declares — which stores exist, who may read each, what routes where. If it says something you
did not intend, the block is right and the configuration is the thing to fix.

Two other corrections travel with it. `yidam decisions-log` no longer describes itself as
read-only; it writes a block when `.yidam/decisions/README.md` carries one. And `yidam policy`
no longer carries the `*` write marker in `--help`, because it writes nothing.

### A property can be retrievable without being prose

`prose: true`, added in cli/v0.11.0, is right for what a node *says*. A gage's `parameter` and
its `units` are not that. Neither string was in any vector. A query for `cubic feet per second`
could not reach the node that writes the phrase. Identifiers, codes and units are what a reader
types verbatim, and they were the part excluded.

Flagging them `prose: true` would reach the embedding. It would also make `node-too-long` count
the code. And `missing-description` would accept a node that says nothing but the code. So
there is a second flag:

```yaml
# <class>.ont.yml
properties:
  - name: parameter
    type: string
    retrievable: true
    description: The measured quantity, by its publisher's parameter code.
```

**Who this affects.** Any corpus keeping identifiers, codes or units in a property. Nothing
changes for a corpus that flags none. Absent means false, exactly as `required:` and `prose:`
are.

**What changes when you flag one.** No report. `yidam embed` composes the property, and nothing
else reads the flag. So **re-run `yidam index-build`**, and expect no movement in `lint`. A
property already flagged `prose: true` needs nothing. Prose reaches the embedding already, and
a property flagged both is composed once.

**What it does not reach.** A property holding anything but a string. A prose key holding a
list is not prose either. It is a real state, and guessing a rendering would put words in the
corpus's mouth.

## cli/v0.12.0

### The README status block no longer counts phases, and refuses a shallow clone

Two changes to `yidam status` and one to `yidam regen`. Both follow one rule: **a committed,
gated block may only report what every checkout of that commit agrees on.**

**Phase counts leave the block.** `yidam status` no longer renders `N active phase(s)` into the
README. The counts read branch refs, and which refs a checkout holds is a property of how it was
fetched. So for one commit there was no value that passed `yidam regen --check` everywhere. A default CI
checkout and a machine that had run `git fetch` wanted different numbers. The gate was unwinnable
rather than wrong. The count also moved with no commit at all. Pushing a phase branch reddened
the gate on every open pull request.

*What to do.* Run `yidam regen` once and commit the result as a `regen:` commit. Your block loses
one cell. Your CI may have added an all-branches `git fetch` to make the two agree. It is no longer needed
for this. Check whether anything else you run wants it before removing it.

*Where the counts went.* `yidam phases` prints them, and being current is the point there.
`yidam status --format json` still carries `active_phases`, `settled_phases`,
`rewritten_phases` and `positions`. Nothing reading the report loses a field.

**`yidam regen` now refuses a shallow clone.** The block reports the corpus genesis, which is the
repository's first commit. A `--depth 1` checkout does not have it, and git does not say so: it
names the boundary commit a root. The block was therefore written from a date the clone invented,
and `--check` asked you to commit it. Measured on one corpus: `genesis 2026-08-28` from a full
clone, `2026-09-07` from a shallow clone of the same commit.

*What to do.* Check out with `fetch-depth: 0`, or run `git fetch --unshallow`. The scaffolded
`.github/workflows/ci.yml` already does the former. If yours does not, this step now fails with a
refusal naming the remedy rather than reporting a stale block.

**`genesis` reads `unknown` in a shallow clone**, in every command, not just this block. That is
the same change from the other side. A corpus whose history is absent has no genesis to name.
`yidam bundle` writes `genesis_hash: null` there, which consumers already handle.

### A qualified evidence tag is now a claim, so two numbers move

`[verified — as proposed]` used to match none of the three tokens exactly. It counted as **no
claim at all**. `claim-tag-malformed` filed a finding on it, and that finding's own advice was
that there was nothing to do. Such a tag now reads as one claim at its standing. It carries the
qualifier as free text, and the finding is gone.

**Two numbers change in your corpus, and neither is a defect.**

- **Claim counts rise.** `yidam status`, `corpus` and the `claims` MCP tool all count the tags
  they were silently dropping. Run `yidam regen` so the README blocks that publish those
  counts agree again. A stale REGEN block is a failing build in a derived repo.
- **`claim-tag-malformed` findings fall**, by exactly the number of claims gained. Your
  `.yidam/lint-baseline.yml` will report them as *expired*. Run `yidam lint --bless` once.

Measured over the six corpora that write these details, 38 findings became 38 claims. Three
corpora move by nothing; the others:

| corpus | findings | claims |
|---|---|---|
| ohio-education-funding | 535 → 506 | 1362 → 1391 |
| allen-recorder | 13 → 6 | 420 → 427 |
| demi-moore | 10 → 8 | 1678 → 1680 |

**What is unchanged.** A bracket that folds in a *citation* — `[verified — Pearl 2009]` — is
still not a tag, and still reported; move it into `references:`. A bracket holding *two
standings* — `[verified for the arithmetic; inference for the reading]` — is still not a tag
either. Its advice now says so: write two claims. Nothing on disk needs editing for this.

The MCP contract goes to **0.19.0**, because it is the contract that states what counts as a
claim.

### A bundle now says which corpus it is

`manifest.yml` gains `genesis_hash`: the full SHA of the corpus's first commit. `genesis` was
already there and is a *date*, so it can order two bundles and cannot tell them apart. Two
corpora created on one day share a date; no two share a root commit.

`bundle_version` stays `"1"`. Adding a field is not a breaking change. Nothing that could
read a bundle before can fail to read one now. **Nothing on disk needs editing.** Rebuild with
`yidam export --format bundle` for a bundle that carries the field. Bundles you have already
published stay valid without it.

**If you read `manifest.yml` yourself,** absence means the corpus is unidentified. It does not
mean zero, and no placeholder should be substituted. One constant would name every bundle
built before this alike — the confusion the field exists to prevent.

`yidam doctor`'s `corpora` check uses it for one new finding. A path dependency and an unpacked
bundle can claim one name, and the checkout is what gets read. That is normal when the two are
one corpus in two forms. It is a **failure** when their genesis hashes differ, because then
`tonpa.lock` pins a corpus nobody is reading. The check is silent when either hash is unknown.

### `export`, `bundle` and `schema` refuse a directory that is not a corpus

They used to succeed. In a directory with no `.yidam/`, every export format wrote an
artefact and exited 0. An empty bundle, an RDF graph of nothing, a `llms.txt` with no nodes.
`--format bundle`, `--format web` and `schema` also **created `.yidam/` on the way**, which is
the marker every check tests for. One run in the wrong directory left a tree that `graph-check`,
`lint` and `doctor` would all accept.

**If a script of yours runs one of these outside a corpus, it now exits 1** and names the
directory. That is the change to look for. Nothing changes inside a repository, and an empty
corpus still exports. An hour-old repository with no nodes is a corpus, not a missing one.

`schema --settings` is unaffected. It prints a compiled-in editor mapping and reads no corpus,
so there is nothing for it to refuse over.

### A corpus nested in another repository has no identity, and RDF export says so

`genesis_hash` — the corpus's first commit — was read by running git at the corpus root. Git
resolves **upward**, so a corpus that is a directory inside a repository was described by
whichever repository encloses it.

Every corpus under this template's `examples/` was in that position. All four minted RDF
subjects under `urn:yidam:094509a128f4`, which is the template's own first commit. Their four
`owl:Ontology` resources were one resource asserting four different labels.

**`export --format rdf` now refuses such a corpus** rather than naming it with its host's
identity. `manifest.yml`'s `genesis_hash` is `null` for it, and `genesis` reads `unknown`
instead of the host's date. Every other export format is unaffected: only RDF names subjects.

**If this affects you, give the corpus its own repository.** Copy it out and `git init` there,
or `yidam clone` a fresh one. A corpus that is already its own repository changes by nothing,
and **no subject that was correct before moves.**

That is why this refuses rather than falling back to the corpus's own earliest commit. Such a
fallback would have kept the four exports working, and changed each identity the day its corpus
was extracted.

### A phase merged with GitHub's rebase or squash button is no longer counted as active

`yidam status` and `yidam phases` decided a phase had settled by asking whether its ref is an
ancestor of the baseline. That is true only of the merge `PHASES.md` prescribes — `--no-ff`,
keeping the synthesis event.

GitHub's *Rebase and merge* and *Squash and merge* both write a single-parent commit onto the
baseline. Neither moves the branch tip. So the ancestry test was false, permanently, and the
phase read `active` forever. One repository reached **22 phases in flight**, every one of
them complete. The number could not fall from inside the repository, because it is computed over
refs.

**Your active-phase count may drop, and that is the fix rather than a loss.** Those phases now
read `rewritten` — settled by a merge that rewrote their commits. `yidam status` names them
separately from `settled`, because the repair is different. The ref can never become an
ancestor, so deleting it is the only way the count falls.

**If you consume the JSON,** `status --format json` gains `rewritten_phases`, and `phases` rows
can carry `state: "rewritten"`. It is a new field rather than a fold into `settled_phases`. A
consumer watching `active_phases` fall needs somewhere to see where the difference went.

One case is knowingly not covered: a squash-merged branch whose files the baseline later edits
again still reads `active`. Detecting it needs the branch's combined patch searched through the
baseline's history, which is exact and unbounded in cost. The error is in the safe direction.

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
