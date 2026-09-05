# RFC-0030 — The surface is beside the binary, and earns a version (`yidam-edit`)

- **Status:** Draft
- **Track:** I25
- **Relates to:**
  - RFC-0016 (the editor surface this is the third client of, and whose boundary it inherits — no longer unbreakable, and therefore guarded)
  - RFC-0001 (the JSON report contract it renders, and now the contract it negotiates on)
  - [RFC-0029](0029-write-tier.md) (the write tier this defers its authoring path to rather than inventing a second one, and whose identity gate this surface is the first hard case for)
  - RFC-0005 (the `handle` seam whose framing argument this repeats at a second transport)
  - RFC-0025 (the design system this consumes, and the adherence gate that does not yet see the file types it would be written in)
- **Versioning layers touched:** **Layer 4.** This surface is a third artifact in the tooling
  layer, versioned and released on its own tag. The row lands with its publish path and its
  channel check, never before — see [§ The fifth artifact](#the-fifth-artifact).
- **Downstream reference case:** Project BOSC (watermark-directory)

> **Amended 2026-09-05.** This RFC was written on the claim that the surface **is not a fifth
> artifact**: served by the binary that computes the verdicts, cut on the same `cli/v*` tag, with
> no npm package, no second runtime, and no new row in Layer 4's table. The owner has reversed
> that decision. The proposal is now the shape this document originally filed first under
> [§ Alternatives considered](#alternatives-considered) — **an Astro application on
> `@astrojs/node`, published to npm, versioned and released on a tag of its own.**
>
> Two arguments carried the original and both are withdrawn. The overlay is no longer computed in
> the process that serves the page, so it costs a bridge; and a Node process is no longer
> impossible, so RFC-0016's boundary stops being unreachable-by-construction and becomes a rule
> that has to be *guarded*. This document does not soften either cost — it states the price in
> [§ The overlay crosses a process boundary](#the-overlay-crosses-a-process-boundary) and
> [§ The boundary, now breakable](#the-boundary-now-breakable), and quotes its own original
> objection where that objection was right.
>
> What the reversal buys is in [§ The fifth artifact](#the-fifth-artifact): a development loop
> that does not rebuild a Rust binary to move a button, a distribution route that does not put
> megabytes of hashed assets in a crate, and two modules the extension already carries that this
> shape can use and the embedded shape had no use for.

## Summary

yidam has two editor surfaces and both require a configured editor to reach. `serve --lsp`
needs an LSP-capable editor and a config block a person writes by hand; the VS Code extension
needs VS Code, and — until #314 — a `.vsix` sideloaded from a GitHub release, because Open VSX
does not serve VS Code proper ([VERSIONING.md § Layer 4](../../VERSIONING.md#layer-4--tooling)).
Neither reaches somebody who has a corpus and a terminal.

This RFC proposes `yidam-edit`: a local authoring surface, an Astro application on
`@astrojs/node`, published to npm and run against a checkout — `npx @yidam/edit` — whose node
forms are generated from the ontology rather than typed as YAML, and which **computes no verdict
of its own**. Every judgement it renders comes from the `yidam` binary the repository pins,
resolved the way the extension resolves it and spoken to over the contract RFC-0001 froze.

The load-bearing claim is no longer about the UI, and it is no longer that this costs nothing.
It is that a third client of one contract is worth a third row in Layer 4, and that the row
carries the obligations every other row carries.

## Problem

### Both editor surfaces are reached by people who already have the tools

[`docs/editor-setup.md`](../editor-setup.md) is, correctly, two configuration recipes and an
extension install. Everything it asks for is reasonable and none of it is free: a Neovim user
writes an autocmd, a Helix user edits `languages.toml`, a VS Code user downloads a `.vsix`
because the registry route does not exist yet. The population that has installed `yidam` and
has *no* configured editor for it is served by nothing, and it is the population a bootstrapped
repository produces — genesis is an agent dialogue, and the person on the other side of it did
not necessarily arrive with an editor already pointed at a corpus.

This is #420's sentence — *every surface arrives in a terminal* — applied to the one surface
that currently does not. `npx` is a terminal, and it is one the target population already has:
this surface's whole premise is a person who has not configured an editor, and the shortest
route to a page for that person is one command that installs and runs in the same breath.

### A structured document is being edited as free text

A corpus node is not prose with links in it. It is a typed record:

```yaml
class: gage
label: Canyon Outlet gage
description: |
  … prose carrying [verified] / [inference] / [open] tags …
properties:
  parameter: "00060"
  units: cubic feet per second
  claim_tag: inference
links:
  - target: ../gage.ont.yml
    relationship: instance-of
```

([`examples/streamflow/.yidam/corpus/gage/canyon-outlet.yml`](../../examples/streamflow/.yidam/corpus/gage/canyon-outlet.yml))

The class declares its `properties` and the edges it licenses; the claim tags are a closed set
of three; the commit that carries the change is drawn from a closed vocabulary. Every one of
those is a control, and in both existing surfaces every one of them is typed by hand into a
text buffer and checked afterwards. The gap is not that the checks are missing — they are
excellent — it is that the *authoring* affordance stops at completion in a line of YAML.

### The repository already ships a web UI, and this is not it

`export --format web` embeds a static site in the CLI and writes it out — `include_str!` over
[`assets/web/`](../../yidam/cli/assets/web/), with the design system concatenated into the
binary by `build.rs` from `_ds_manifest.json`
([`export_web.rs:12-21`](../../yidam/cli/src/cmd/export_web.rs#L12-L21)). That page is opened
over `file://` with no process behind it, which is exactly why the design system had to travel
inside it:

> An exported page is opened over `file://` and cannot fetch anything, so the system travels
> with it — which is the constraint that produced a hand-copied subset before #465, and ten
> drifted values with it.
>
> — [`export_web.rs:12-21`](../../yidam/cli/src/cmd/export_web.rs#L12-L21)

What is proposed here differs on both counts. There is a process, so nothing has to travel; and
that process can ask the binary a question, so the page can carry a verdict rather than a
snapshot of one. The earlier draft of this RFC read the export as prior art for embedding a UI
in the binary. It is better read as the measurement of what a UI with no process behind it costs.

### A web surface was already killed, and one of its premises has come back

#236 closed by decision: `packages/web/` was a browser shell over an exported `.yiz` bundle,
with no deployment and no public corpus to render, and the owner's settlement recorded that a
rendered shell is a derived repository's *own object* — kuten territory (#573/#582) — not a
template surface.

That reasoning is about a **deployed reader over an exported artifact**. Under the original
shape three of its four premises were absent. Under this one, two and a half are:

| #236's premise | Here |
|---|---|
| Needs a deployment and a URL | Loopback, started by the person who owns the corpus. Unchanged. |
| Needs an exported bundle, so needs a corpus to export | Reads the working tree it was started in. Unchanged. |
| Renders; computes nothing | Authors — but the thing that computes is now **a subprocess, not this process**. |
| A rendered shell is a derived repo's own object | An editor is tooling a *person* runs — Layer 4's own definition. Unchanged. |

The third row is the one the reversal moved, and moving it is the whole cost of this document.
A Node server that renders a corpus and shells out for its verdicts is one refactor away from a
Node server that renders a corpus and computes them, and that server is #236 with a package.json.
[§ The boundary, now breakable](#the-boundary-now-breakable) is what stands between the two, and
it has to be a gate rather than a paragraph.

The fourth premise remains the one to watch: if `yidam-edit` were ever pointed at a bundle and
deployed, it would be #236 wearing a new name.
[§ What this deliberately is not](#what-this-deliberately-is-not) closes that door explicitly.

### RFC-0016 rejected a web app, and named the reason

> **A web app instead of an editor.** Rejected as a substitute; `export --format web` already
> exists and serves reading and retrieval. It cannot put a squiggle under the link you just
> mistyped, which is where the friction actually is.

That rejection is correct and this RFC does not overturn it. It rejected a *reader* offered as a
substitute for an *authoring* surface. What is proposed here can put the squiggle there —
[§ The overlay crosses a process boundary](#the-overlay-crosses-a-process-boundary) is how, and
what it costs — and it is offered as a third client of the same contract, not as a replacement
for either existing one.

## Proposal

### The boundary, now breakable

> **TypeScript computes affordances. The CLI computes verdicts.**

RFC-0016's rule applies verbatim, and the original draft of this RFC observed that under the
embedded shape it was not merely forbidden but *unreachable*: with no Node process anywhere, the
JavaScript that shipped could not compute a verdict because it had nothing to compute one from.

That property is gone. There is now a Node process holding a parsed corpus, and every check in
this repository is a pure function over exactly that. The failure mode the rule guards against —
a TypeScript re-derivation of a check, standing beside the real one and disagreeing with it — is
reachable in this design by a contributor with good intentions and an afternoon.

So the rule stops being a sentence in an RFC and becomes a gate. Three, specifically, and Phase 1
carries them rather than Phase 4:

- **No check names in the server's dependency graph.** A test asserts that nothing under the
  app's server directory imports a corpus-evaluating module, and that the only source of a
  `violations` array is a process spawn.
- **Every verdict the page renders is traceable to an envelope.** The page renders findings from
  a parsed RFC-0001 envelope and from nowhere else; a finding with no `format_version` behind it
  fails a render test.
- **The `@yidam/core` boundary is drawn explicitly.** The TypeScript SDK is a parity surface with
  real corpus logic in it, which makes it the single most likely accidental route to a
  re-derivation. It is available for types and for affordances; a server route that calls its
  evaluating functions fails the first gate above.

This is the price of the reversal, paid where it falls due. It is not a smaller amount of work
than the embedded shape's asset pipeline; it is a different kind of work, and it protects
something more valuable.

### The fifth artifact

VERSIONING.md, on the `.mcpb` bundle, argues the case this RFC originally borrowed:

> **The `.mcpb` bundle is not a layer, and does not earn a version.** #421 asked which, and the
> answer is neither: a bundle is the CLI binary in a different wrapper, cut on the same `cli/v*`
> tag by the same workflow step that cuts the tarballs, and its manifest carries the version of
> the binary inside it. Giving it a version of its own would put the CLI's version in a second
> place.

The clauses do not transfer, and the reversal is the recognition that they never quite did. A
`.mcpb` bundle contains the binary; `yidam-edit` contains no binary and resolves one at run time.
Its version is not the CLI's version in a second place, because it is not the CLI's version at
all — it is a statement about a client, released on a cadence a button change should be allowed
to have and a corpus-lint fix should not be forced into.

That is Layer 4's existing argument about the extension, applied a third time:

> A Marketplace release needs a public, semver-shaped version and its own cadence; a CLI patch
> should not imply an extension release, and an extension patch should not imply a CLI one.
> Neither version is what the two negotiate on.

So:

- **A third row in Layer 4's table**, artifact `@yidam/edit`, manifest
  `yidam/editors/web/package.json`, tag `edit/v{major}.{minor}.{patch}`, registry npm.
- **A release channel**, and therefore a check in `install-channels.yml` that asks npm what it
  serves — for the reason #231 established: a channel with a publisher and no check is an
  artifact that builds while nobody has asked whether it can be obtained.
- **`format_version` is now involved**, and this is the sharpest consequence of the reversal.
  The handshake exists because a client is versioned independently of the binary it talks to,
  and this client is exactly that. It reads the envelope first and degrades loudly on an unknown
  major, as RFC-0016 requires and as the extension already does.

**The row does not land yet, and withholding it is deliberate.** VERSIONING.md's own rule:

> This table names registries this project *delivers to*, never ones it intends to.

Nothing publishes to npm today, and `the_registries_layer_4_names_are_delivered_and_checked`
refuses a row whose publish path and channel check are not there with it — the same test that
holds the Marketplace row out. The row, the `npm publish` step, and the channel check land
together in Phase 4, as one change. Until then Layer 4 is unchanged and this RFC is the record
of what will change it. #232 is what the other order costs: `cargo binstall yidam` stood in the
README for a release cycle while `yidam` did not exist on crates.io.

### The command surface

```
npx @yidam/edit [--root DIR] [--port N] [--no-open]
```

The `yidam` binary gains nothing. There is no `yidam edit` subcommand, no route table in
`http.rs`, and no new Rust feature — a consequence of the reversal worth stating plainly, since
the original design put all three in the CLI.

- **`--root` sets a working directory, and the binary resolves the corpus from there.** The
  original text said it *"resolves exactly as `serve --root` does"*, which is right about
  `serve` and wrong about this surface: **`--root` exists on `serve` and `export`, and on
  neither of the five report commands this design spawns** — `lint`, `graph`, `graph-check`,
  `status`, `open-questions`. Those resolve by `git rev-parse --show-toplevel` from the
  working directory ([`paths.rs:4-8`](../../yidam/cli/src/paths.rs#L4-L8)), which has a
  consequence stated below rather than inherited silently — see
  [§ A corpus inside a corpus](#a-corpus-inside-a-corpus).
- **Loopback only, and not configurable.** The original offered `--bind` for a container; this
  one does not, because a server that authenticates nobody and now has a Node process in it
  should not carry the flag that turns it into #236. A container reaches it by publishing a port,
  which is the container's decision to make rather than this server's.
- `--no-open` suppresses the browser launch, for a remote shell.
- **Origin is checked, not configured.** The only legitimate client is the page this server
  served, so any other `Origin` is refused rather than allowed by a flag.

### A corpus inside a corpus

Found by running Phase 1's scaffold rather than by reading, and it belongs in this document
because it is a property of the seam this design chose, not of the code that hit it.

`--root` exists on `serve` and on `export`, and `export`'s own declaration says why:

> The same flag `serve` takes, and for the reason #236 and #428 both give: the corpus worth
> exporting is often not the repository you are standing in. `examples/streamflow` is a corpus
> inside this one, and `git rev-parse --show-toplevel` from it answers with yidam, which has
> no `.yidam/` — so the only way to export it was to copy it elsewhere and `git init` the copy.
>
> — [`main.rs:404-408`](../../yidam/cli/src/main.rs#L404-L408)

The five commands this surface spawns do not have that flag. So `yidam-edit --root DIR` sets
the child's working directory and inherits exactly the behaviour the quotation describes:
pointed at `examples/streamflow` inside this checkout, every report answers about the outer
repository, and the browser shows a corpus with no nodes in it. **An empty corpus and a wrong
corpus render identically**, which is what makes this worth a paragraph rather than a bug.

Phase 1 does the honest minimum: every envelope carries `root`, so the surface compares what
it asked for against what answered and says so in the header when they differ. That is
degrade-loudly applied to a question the handshake does not cover.

The real fix is a `--root` flag on the report commands, and it is **the CLI's to make, not this
surface's** — restating the resolution rule in TypeScript would be the second copy the parity
apparatus exists to prevent. Until it exists, a corpus nested in another git repository is
opened by copying it out, which is the workaround `export`'s comment already records.

### How it reaches a verdict

The server spawns the pinned binary per request and parses the envelope. Nothing else.

| Route | Serves |
|---|---|
| `GET /` and the app's pages | Astro, server-rendered on `@astrojs/node` |
| `GET /api/handshake` | `format_version` plus the CLI's version, commit and feature list — the fields [`report::YidamBlock::current()`](../../yidam/cli/src/report.rs#L48) assembles |
| `GET /api/corpus` | `yidam graph --format json`, whose nodes and resolved edges come from [`model::corpus_nodes()`](../../yidam/cli/src/model.rs#L400) — the function `serve`, `graphml` and `rdf` already share |
| `GET /api/reports` | `lint` and `graph-check` as the RFC-0001 envelope, byte-identical to `--format json` |
| `GET /api/overlay` (SSE) | Diagnostics from a supervised `yidam serve --lsp` — see below |
| `POST /api/act/*` | Deferred to RFC-0029. Absent until it settles. |

**Binary resolution is the extension's, and this is the one place the reversal is free.**
[`binary.ts`](../../yidam/editors/vscode/src/binary.ts) and
[`handshake.ts`](../../yidam/editors/vscode/src/handshake.ts) are already `vscode`-free by
deliberate design — *"everything here is a pure function over inputs the extension host
supplies, so it is testable with `node --test` and needs no Electron"* — and they encode two
rules this surface must obey and would otherwise re-derive:

> **The extension never bundles, downloads, or builds a binary.** A derived repository's
> `.yidam.toml` records which yidam commit governs its corpus; installing some other one
> behind the user's back would make the editor's verdicts disagree with CI's, which is the
> single failure this whole surface exists to avoid.
>
> — [`binary.ts:8-11`](../../yidam/editors/vscode/src/binary.ts#L8-L11)

The embedded shape had no use for either module — a binary serving its own UI knows its own
version and its own path. This shape needs both, and they exist.

### The overlay crosses a process boundary

This is what makes the surface an editor rather than a viewer, and under the reversal it is the
most expensive thing in the document. The original said so, about this design, and was right:

> **This is the single strongest technical argument for serving the editor from the binary**
> rather than from a Node process that would have to bridge stdio LSP to a WebSocket to get the
> same answer.

That bridge is now the plan. `Overlay` is a `pub struct` in the lint module
([`lint/mod.rs:101`](../../yidam/cli/src/cmd/lint/mod.rs#L101)), and
[`run_checks_with(&root, &opts, &overlay)`](../../yidam/cli/src/cmd/lint/mod.rs#L153) is the
entry point the language server calls on every change
([`lsp.rs:217`](../../yidam/cli/src/cmd/lsp.rs#L217)) — but it is reachable only through
`serve --lsp`. `yidam lint` has no overlay flag, and the extension is no prior art here: it
carries no LSP client and no dependencies at all, running `lint --format json` against the tree
and building diagnostics itself.

So the server supervises **one `yidam serve --lsp` child per corpus**, sends buffer text as
`textDocument/didChange`, and relays `publishDiagnostics` to the browser over server-sent
events. The squiggle is drawn by the function that draws it in Neovim, with no second
implementation to drift — the boundary holds — and there is now a process, a protocol and a
lifecycle between the browser and that function. Process supervision, LSP framing over stdio,
and a child that dies mid-session are three failure modes the embedded shape did not have. They
belong to Phase 2 and are the reason Phase 2 is no longer the cheap phase.

### Forms from the ontology — and the trap already found

A class declares its `properties` and its `out` edges, so a node form can be generated rather
than typed. The obvious next step — restrict the relationship picker to declared edges — is
**wrong**, and the extension already measured why:

> Measured against a live derived repository at 90 nodes and 299 edges: **17 of the (class,
> relationship) pairs actually in use are not declared as `out` edges on their class**, and one
> of them — `instance-of`, the edge every node carries to its own `.ont.yml` — is used by every
> class and declared by none. Nothing lints relationships against the ontology, so a completion
> list restricted to declared edges would be stricter than any rule in the system and would omit
> the corpus's single most-used relationship.
>
> — [`graph.ts:19-24`](../../yidam/editors/vscode/src/graph.ts#L19-L24)

The form adopts that finding rather than rediscovering it: **declared edges first, relationships
already in use beside them, and the reason each is offered shown next to it.** A form is a
stronger nudge than a completion list, so getting this wrong here is worse than getting it wrong
there — a picker with no free-text escape would make `instance-of` unwritable.

The same distinction governs the rest of the form. The commit vocabulary *is* closed and *is*
gated, so the commit affordance may constrain to it. Claim tags are three and closed. The
ontology is a guide. Three sources, three different strengths of constraint, and the form has to
carry the difference.

Note what the reversal does *not* change here. Reading the ontology to build a form is an
affordance, not a verdict, and it was on the TypeScript side of RFC-0016's line under both
designs.

### Where the sources live, and what ships

**Sources at `yidam/editors/web/`**, a sibling of `vscode/` under
[`yidam/editors/`](../../yidam/editors/README.md), which already exists as the index of editor
surfaces. Grouped by surface rather than by technology: the extension's binary resolution and
handshake modules are the closest prior art, and RFC-0016's split is a split of surfaces.

**Nothing is committed, and nothing is embedded.** The whole asset question the original spent a
section on is dissolved by the reversal: `npm publish` ships a build produced in CI, `cargo
package` never sees it, and the crate root is untouched. The staleness gate, the crate-root
symlink and the `packaging.rs` assertion are all withdrawn — that section existed to answer a
question this shape does not ask.

One clause of it does transfer, and it is worth carrying over rather than rediscovering:

> `cargo package` copies only what lives under the crate root. An `include_str!` whose path
> escapes it … resolves in the working tree and in every CI job, because the tree is right
> there, and is simply **absent from the tarball**.
>
> — [`packaging.rs:1-19`](../../yidam/cli/tests/packaging.rs#L1-L19)

`npm publish` has the identical property with an identical failure signature: a module imported
from outside the package root resolves in the working tree and in every CI job, and is absent
from the tarball. So the app carries every module it ships under `yidam/editors/web/`, and a
packing test asserts the published tarball runs — the same check, one ecosystem over, learned
from two near-misses this repository has already paid for.

### Phases

**Phase 0 — this RFC and this amendment.** The decisions above are the deliverable.

**Phase 1 — the read surface.** `npx @yidam/edit` serving an Astro MPA for browse / node /
reports / open-questions / status, against the design system's components. Binary resolution and
the handshake, both from the extension's modules. **The three boundary gates from
[§ The boundary, now breakable](#the-boundary-now-breakable) land in this phase**, before there
is anything for them to fail on — a gate written after the code it governs is a gate written
around it. No writes, no overlay.

**Phase 2 — the overlay, and the forms.** The `serve --lsp` bridge, with supervision and a
lifecycle; class-driven node forms with the relationship rule above; claim tags as controls.
Still no writes to disk. This phase absorbed the reversal's cost and should be planned as the
largest of the four, not the second-cheapest.

**Phase 3 — writes.** Gated on [RFC-0029](0029-write-tier.md). If `act` lands, the editor's
writes are its operations behind a second framing, for the reason
[`http.rs:9`](../../yidam/cli/src/cmd/serve/http.rs#L9) gives about the first: *"Not a second
contract. `super::handle` is the seam RFC-0005 left for exactly this."* If RFC-0029 settles the
other way, this phase needs its own argument and does not get to assume one.

**Phase 4 — the artifact, and the gates.** The Layer 4 row, the `npm publish` path and the
`install-channels.yml` check, landing as one change. A `ci-editor-web` mise task and a CI job
mirroring `vscode`; a Dependabot npm group; a row in [`editor-setup.md`](../editor-setup.md) and
in [`yidam/editors/README.md`](../../yidam/editors/README.md); a sidebar entry, without which
`astro build` fails.

### The identity gate, and the case that got weaker

RFC-0029 §2.2 gates the write tier on authorship rather than on sequencing:

> The `act` capability is declarable only where a git author identity exists. Over stdio that
> identity exists today: the server is a subprocess of a person's shell inside their checkout …
> Over HTTP no author exists until #427 lands in a shape that yields a **stable subject claim**
> mapped onto a committer identity — so an HTTP server MUST NOT declare `act` until then.

`yidam-edit` is the first case where that rule's criterion and its proxy come apart. The
criterion is *does a git author identity exist*; the proxy is *stdio, not HTTP*. This surface is
HTTP — and it is also a process a person started from their own shell, inside their own checkout,
bound to loopback, serving the page to the browser on the same machine.

Two honest readings, and this RFC does not pick one:

- **The sentence binds as written.** An HTTP server may not declare `act`, so Phase 3 waits on
  #427 — a question about remote authorisation that a loopback editor does not raise. RFC-0029
  names this cost itself, about the neighbouring case: re-blocking on #427 *"parks stdio-local
  writes on a question they do not have."*
- **The criterion binds, and the transport was its proxy.** A loopback server started by the
  corpus's owner is in the stdio position, and the gate should say so in terms of the author
  rather than the socket.

**The reversal weakens the second reading, and the amendment says so rather than leaving the
case as it stood.** Under the embedded shape the argument was that only the socket differed from
the stdio justification. That is no longer true: a Node process now sits between the person and
the binary, and the git author the write would be attributed to is resolved by that process
rather than inherited by it. The reading is still arguable — the process is still a child of the
person's shell — but it is no longer the near-identity it was, and RFC-0029 should be asked with
that difference on the table.

It remains **not this RFC's to settle**: RFC-0029 is where that sentence lives, and sharpening it
is an amendment to RFC-0029 with a dated block, not a paragraph here.

### How it is tested

- The extension's fixture is reused, not rebuilt: `mise run ext-fixture` stages the reports
  golden corpus as a real repository, and the app is driven against that. What CI checks and what
  a person sees stay one repository, which is why the task exists.
- Route handlers take plain values and return them, following `http.rs`'s stated discipline —
  *"Everything that decides whether a request is served … takes plain values and returns an
  `Outcome`. None of it needs a socket"* — so the policy is unit-testable without binding a port.
  The discipline is worth copying across the language boundary precisely because this side is the
  one that can now cheat.
- **The boundary gates are tests, not review notes.** See
  [§ The boundary, now breakable](#the-boundary-now-breakable).
- The design gate covers this surface **only if the file extensions match**. See
  [§ Open questions](#open-questions).

### What this deliberately is not

- **Not deployable, and not deployed.** No hosted instance, no bind address in any documentation,
  no path over an exported bundle. Any of those makes it #236, and #236 is closed by decision.
  This clause matters more under the reversal than it did under the original, because the
  original had no server that *could* be deployed.
- **Not a second MCP surface.** Agents have `serve --mcp`. Nothing here is reachable by one.
- **Not a re-implementation of any check.** The boundary is RFC-0016's, unchanged in what it
  requires and newly guarded in how it is held.
- **Not a replacement for the extension or the LSP.** Three clients, one contract. A person with
  a configured editor should keep using it; this is for the person who has not got one.
- **Not a bootstrap wizard**, for RFC-0016's reason: genesis is an ontology dialogue with an
  agent, and a form that skips the dialogue produces the unconsidered ontology the dialogue
  exists to prevent.
- **Not a sangha surface.** Positions and resolutions are governed by the constitution; RFC-0016
  put anything that writes one out of scope pending the governance RFCs, and that holds here.

## Migration & compatibility

Nothing in a derived repository changes, and `.yidam.toml` gains nothing. The CLI is untouched:
no new subcommand, no new feature, no route added to `http.rs`, and no growth in the binary —
the size question the original deferred to Phase 1 does not arise.

What changes is the release surface. Layer 4 goes from two artifacts to three, a fifth channel
becomes a sixth, and `edit/v*` joins `cli/v*` and `editor/v*` as a tag pattern a workflow must
trigger on and no other layer may fire. VERSIONING.md's standing rule applies to all of it:
never bump a layer as a side effect of another layer's release.

`format_version` becomes load-bearing for a third client. It is `"1"` today and this surface
does not propose to move it.

## Alternatives considered

- **Served from the binary, with the assets embedded** — the original proposal of this RFC, and
  now the declined alternative. It is the cheaper shape on two axes that matter: the overlay is
  an in-process function call rather than a supervised LSP child, and RFC-0016's boundary is
  unreachable rather than guarded. It was declined for the development loop it forces — a Rust
  rebuild to move a button — for the megabytes of hashed build output it puts under the crate
  root and into every `cargo install`, and for the versioning claim it required, that a
  UI released on the CLI's cadence is the same artifact as the CLI. The full original argument
  is preserved in this document's git history at `f3b203e`.
- **Static assets over `serve --mcp --http`, read-only.** This is #236's shell with a different
  data source. It cannot author, and the MCP contract is read-only by construction
  ([`tools.json`](../../yidam/prelude/sdks/parity/mcp/tools.json) — thirteen tools, none of them
  a write).
- **Extend `export --format web` into an editor.** Rejected. An export is opened over `file://`
  with no process behind it; that is the constraint that forced the design system into the
  binary in the first place. Nothing there can compute a verdict, which is the whole difference.
- **A plain Vite/React SPA rather than Astro.** Defensible, and Astro is chosen for two concrete
  reasons rather than taste: the toolchain, the Dependabot group and the CI pattern already exist
  for [`yidam/web/docs`](../../yidam/web/docs/), and `@astrojs/react` is already how this
  repository's design-system components are consumed. Most of this surface — browse, node,
  reports, status — is content-shaped and wants an MPA; only the forms are islands.
- **A desktop application (Tauri, Electron).** A new toolchain, a new signing story, a new
  channel, three new platform builds. Everything it buys over a loopback page is unrelated to the
  problem.
- **Wait for RFC-0029 before starting.** Rejected. Phases 1 and 2 are read-only and are the
  majority of the work; sequencing them behind a governance decision they do not depend on delays
  the useful part for the contested one.

## Open questions

- **Which npm name, and who owns it.** The lean is `@yidam/edit`, for consistency with
  [`@yidam/core`](../../yidam/prelude/sdks/typescript/package.json). Neither is published and the
  `@yidam` scope is unclaimed, so the scope has to be registered before the Layer 4 row can land
  — a precondition, not a detail. `yidam-edit` unscoped and `@goedelsoup/yidam-edit` are the
  fallbacks, and the second matches the VS Code publisher this project already owns.
- **Should the report commands take `--root`?** `serve` and `export` do; `lint`, `graph`,
  `graph-check`, `status` and `open-questions` do not, and
  [§ A corpus inside a corpus](#a-corpus-inside-a-corpus) is what that costs this surface.
  Phase 1 detects the mismatch and reports it, which is a workaround rather than an answer.
  Adding the flag is a CLI change with consumers beyond this one — the extension resolves a
  workspace the same way — so it is filed here and decided there.
- **Do `binary.ts` and `handshake.ts` get extracted, or copied?** Both are `vscode`-free and both
  are needed here, but importing them across package roots is the `npm publish` failure named in
  [§ Where the sources live](#where-the-sources-live-and-what-ships). A shared module would be a
  fourth Layer 4 artifact unless it stays private, and a copy needs a parity test. Phase 1 carries
  a copy plus that test; the extraction is a decision this RFC does not force.
- **`tsx` is not in the design gate's scope.** The consumer scan discovers surfaces by file
  extension, and says so — [`design_tokens.rs:134-140`](../../yidam/cli/tests/design_tokens.rs#L134-L140):
  *"A consumer is any file of a [`CONSUMER_EXTENSIONS`] type outside `yidam/design/` that
  references a token the system declares. The next UI kit is covered the moment it uses the
  palette, which is the point: a roster here would stop covering whatever came next."*
  `CONSUMER_EXTENSIONS` is `css`, `astro`, `jsx`. So this surface is covered for free if its
  islands are `.jsx`, and invisible to the raw-colour check if they are `.tsx` — #591's webview
  blind spot arriving a third time, and it should be closed by choosing one before the first
  island is written, not after. Lean: extend the list, since the next surface will have the
  same question.
- **The design system's React components have never been hydrated.** No `client:*` directive
  appears on any quality page — [`astro.config.mjs:243-246`](../../yidam/web/docs/astro.config.mjs#L243-L246):
  *"this is a build-time renderer: React produces HTML and none of it is shipped to a reader."*
  This surface would be the first consumer to ship them to a browser. Whether they survive
  client bundling is unknown and is a Phase 1 spike, not an assumption.
- **Does RFC-0029's identity gate reach a loopback editor with a Node process in it?** The case
  is argued in [§ The identity gate, and the case that got weaker](#the-identity-gate-and-the-case-that-got-weaker)
  and belongs to RFC-0029, as an amendment with a dated block. Until it is answered, Phase 3's
  dependency is *either* RFC-0029's build *or* RFC-0029's build plus #427, and those are very
  different schedules.
- **Does Phase 2 write at all?** A form whose output cannot be saved is a strange object. The
  alternative is a small, editor-only write path that does not wait for RFC-0029 — which is a
  second answer to "how does something outside the corpus write into it", and is the thing this
  RFC is trying not to create.
- **Does a commit affordance belong here?** An editor that cannot commit cannot finish the work,
  and the closed vocabulary makes the affordance cheap and safe. But the extension's SCM box is
  VS Code-shaped; a browser page driving `git commit` on the host is a larger claim than
  rendering one, and it deserves its own argument.
- **The port, and coexistence.** `--http` defaults to 8787. Whether this surface takes a
  neighbouring default, and whether both may run against one corpus at once, is unsettled and
  cheap to settle. The supervised `serve --lsp` child makes the second half of the question
  sharper than it was: two editors, two children, one corpus.
