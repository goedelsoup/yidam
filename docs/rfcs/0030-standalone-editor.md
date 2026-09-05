# RFC-0030 — The surface is in the binary, not beside it (`yidam edit`)

- **Status:** Draft
- **Track:** I25
- **Relates to:**
  - RFC-0016 (the editor surface this is the third client of, and whose boundary it inherits unchanged)
  - RFC-0001 (the JSON report contract it renders)
  - [RFC-0029](0029-write-tier.md) (the write tier this defers its authoring path to rather than inventing a second one, and whose identity gate this surface is the first hard case for)
  - RFC-0005 (the `handle` seam whose framing argument this repeats at a second transport)
  - RFC-0025 (the design system this consumes, and the adherence gate that does not yet see the file types it would be written in)
- **Versioning layers touched:** none. The claim is argued in
  [§ Not a fifth artifact](#not-a-fifth-artifact) —
  [Layer 4](../../VERSIONING.md#layer-4--tooling)'s table is unchanged, deliberately.
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

yidam has two editor surfaces and both require a configured editor to reach. `serve --lsp`
needs an LSP-capable editor and a config block a person writes by hand; the VS Code extension
needs VS Code, and — until #314 — a `.vsix` sideloaded from a GitHub release, because Open VSX
does not serve VS Code proper ([VERSIONING.md § Layer 4](../../VERSIONING.md#layer-4--tooling)).
Neither reaches somebody who has a corpus and a terminal.

This RFC proposes `yidam edit`: a local authoring surface **served by the binary that already
computes the verdicts**, built with Astro against the repository's own design system, whose
node forms are generated from the ontology rather than typed as YAML. The load-bearing claim is
not about the UI. It is that this surface **is not a fifth artifact** — no npm package, no
second runtime, no new release channel, no new row in Layer 4's table. It is the CLI in a
different wrapper, and [VERSIONING.md has already made that argument once](../../VERSIONING.md#layer-4--tooling)
about the `.mcpb` bundle.

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
that currently does not.

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

### The repository already ships a web UI out of the binary

This is not a new category. `export --format web` embeds a static site in the CLI and writes it
out — `include_str!` over [`assets/web/`](../../yidam/cli/assets/web/), with the design system
concatenated into the binary by `build.rs` from `_ds_manifest.json`
([`export_web.rs:12-21`](../../yidam/cli/src/cmd/export_web.rs#L12-L21)). What is proposed here
is the same category with the two properties that one lacks: it is **served** rather than
exported, so a process is alive to compute verdicts, and it is **authoring** rather than
reading.

### A web surface was already killed, and the reasons do not transfer

#236 closed by decision: `packages/web/` was a browser shell over an exported `.yiz` bundle,
with no deployment and no public corpus to render, and the owner's settlement recorded that a
rendered shell is a derived repository's *own object* — kuten territory (#573/#582) — not a
template surface.

That reasoning is about a **deployed reader over an exported artifact**. Three of its four
premises are absent here:

| #236's premise | Here |
|---|---|
| Needs a deployment and a URL | Loopback, started by the person who owns the corpus |
| Needs an exported bundle, so needs a corpus to export | Reads the working tree it was started in |
| Renders; computes nothing | Authors; and the thing that computes is in the same process |
| A rendered shell is a derived repo's own object | An editor is tooling a *person* runs — Layer 4's own definition |

The fourth is the one to watch, and it is why this RFC exists rather than a pull request: if
`yidam edit` were ever pointed at a bundle and deployed, it would be #236 wearing a new name.
[§ What this deliberately is not](#what-this-deliberately-is-not) closes that door explicitly.

### RFC-0016 rejected a web app, and named the reason

> **A web app instead of an editor.** Rejected as a substitute; `export --format web` already
> exists and serves reading and retrieval. It cannot put a squiggle under the link you just
> mistyped, which is where the friction actually is.

That rejection is correct and this RFC does not overturn it. It rejected a *reader* offered as a
substitute for an *authoring* surface. What is proposed here can put the squiggle there —
[§ The overlay is the diagnostics path](#the-overlay-is-the-diagnostics-path) is how — and it is
offered as a third client of the same contract, not as a replacement for either existing one.

## Proposal

### The boundary, unchanged and easier to keep

> **TypeScript computes affordances. The CLI computes verdicts.**

RFC-0016's rule applies verbatim. What changes is how hard it is to break: there is **no Node
process anywhere in this design**. The browser renders and the binary decides, so the failure
mode the rule guards against — a TypeScript re-derivation of a check, standing beside the real
one and disagreeing with it — is not merely forbidden, it is unreachable. The JavaScript that
ships cannot compute a verdict because it has nothing to compute one from.

### Not a fifth artifact

VERSIONING.md, on the `.mcpb` bundle:

> **The `.mcpb` bundle is not a layer, and does not earn a version.** #421 asked which, and the
> answer is neither: a bundle is the CLI binary in a different wrapper, cut on the same `cli/v*`
> tag by the same workflow step that cuts the tarballs, and its manifest carries the version of
> the binary inside it. Giving it a version of its own would put the CLI's version in a second
> place.

Every clause transfers. `yidam edit` is the CLI binary with a UI compiled into it, cut on the
same `cli/v*` tag, carrying the version of the binary it is inside. So:

- **No row in Layer 4's table.** The table names artifacts that are versioned and delivered
  independently; this is neither.
- **No new release channel**, and therefore no new check in `install-channels.yml` — which
  matters, because a channel check has nothing to look at on the day it lands and goes red for
  a release that predates it.
- **No `npm publish`, no `package.json` version to keep in step with a Cargo one.**
- **`format_version` is not involved.** The handshake exists because a client is versioned
  independently of the binary it talks to. This client is not.

This is the entire reason to prefer this shape over an npm-distributed Astro server. It is
argued at length in [§ Alternatives considered](#alternatives-considered).

### The command surface

```
yidam edit [--root DIR] [--bind ADDR] [--port N] [--no-open]
```

Behind the `serve-http` feature, which is in the default set
([`Cargo.toml:238`](../../yidam/cli/Cargo.toml#L238)) — so the binary from every install channel
already carries the transport, and this adds a route table rather than a dependency.

- `--root` resolves exactly as `serve --root` does, and refuses a directory with no `.yidam/`
  by name (#549). The same flag, the same resolution, one answer.
- `--bind` defaults to loopback for `--http`'s stated reason: this server authenticates nobody.
- `--no-open` suppresses the browser launch, for a remote shell or a container.
- **No `--allow-origin`.** `serve --mcp --http` needs it because the client is another site;
  here the client is the page this server just served, so any `Origin` that is not this server's
  own is refused rather than configured. A narrower rule than `--http`'s, because the situation
  is narrower.

### The routes

Per [`http.rs`](../../yidam/cli/src/cmd/serve/http.rs)'s own note about what sits on top of
hyper — *"a match on method and path, not an HTTP implementation"*:

| Route | Serves |
|---|---|
| `GET /`, `GET /assets/*` | The embedded Astro build |
| `GET /api/handshake` | Version, commit, feature list — the fields [`report::YidamBlock::current()`](../../yidam/cli/src/report.rs#L48) already assembles |
| `GET /api/corpus` | Nodes, classes and resolved edges from [`model::corpus_nodes()`](../../yidam/cli/src/model.rs#L400) — the function `serve`, `graphml` and `rdf` already share |
| `GET /api/reports` | `lint` and `graph-check` as the RFC-0001 envelope, byte-identical to `--format json` |
| `POST /api/overlay` | Buffer contents in, findings out — see below |
| `POST /api/act/*` | Deferred to RFC-0029. Absent until it settles. |

The MCP endpoint is a single path by deliberate choice
([`http.rs:46`](../../yidam/cli/src/cmd/serve/http.rs#L46)); this is a static file server plus a
handful of reads, which is a different shape and does not pretend otherwise. It is **not** a
second MCP contract, and nothing here is reachable by an agent — an agent has `serve --mcp`.

### The overlay is the diagnostics path

This is what makes the surface an editor rather than a viewer, and it costs nothing new.
`Overlay` is already a `pub struct` in the lint module
([`lint/mod.rs:101`](../../yidam/cli/src/cmd/lint/mod.rs#L101)), and
[`run_checks_with(&root, &opts, &overlay)`](../../yidam/cli/src/cmd/lint/mod.rs#L153) is the
entry point the language server already calls on every change
([`lsp.rs:217`](../../yidam/cli/src/cmd/lsp.rs#L217)).

`POST /api/overlay` sets the buffer text and makes that same call. The squiggle RFC-0016's
rejection said a web app could not draw is drawn by the function that draws it in Neovim, in the
same process, with no LSP framing between them and no second implementation to drift. **This is
the single strongest technical argument for serving the editor from the binary** rather than
from a Node process that would have to bridge stdio LSP to a WebSocket to get the same answer.

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

### Where the sources live, and where the build output goes

**Sources at `yidam/editors/web/`**, a sibling of `vscode/` under
[`yidam/editors/`](../../yidam/editors/README.md), which already exists as the index of editor
surfaces. Grouped by surface rather than by technology: the extension's binary resolution and
handshake modules are the closest prior art, and RFC-0016's split is a split of surfaces.

**Build output committed under the crate root**, reached the way this repository has already
settled twice:

> `cargo package` copies only what lives under the crate root. An `include_str!` whose path
> escapes it … resolves in the working tree and in every CI job, because the tree is right
> there, and is simply **absent from the tarball**. … This has now happened twice. … The fix in
> both cases was the same and is the one this repository has settled on: a **symlink at the
> crate root** (git mode `120000`).
>
> — [`packaging.rs:1-19`](../../yidam/cli/tests/packaging.rs#L1-L19)

Committing build output is a genuine cost — diff noise, unreviewable hashed filenames — and the
alternative is worse: build the assets in CI and `cargo install yidam` from crates.io produces a
binary whose `edit` command serves nothing, for everybody, while every job here stays green.
That is precisely the failure `packaging.rs` was written about, one level up.

The cost is contained the way this repository contains every other generated artifact: a
**staleness gate in the REGEN idiom**. `mise run editor-build` regenerates; a CI check rebuilds
and fails if the committed output differs. `packaging.rs` gains an assertion that the tarball
carries the assets.

### Phases

**Phase 0 — this RFC.** The decisions above are the deliverable. Nothing below is worth starting
before the artifact question and the write question have answers on the record.

**Phase 1 — the read surface, no new CLI.** `yidam edit` serving the embedded build; handshake,
corpus, reports routes. Astro MPA for browse / node / reports / open-questions / status, using
the design system's components. No writes, no overlay. Useful on its own, and it is the phase
that proves the asset pipeline before anything depends on it.

**Phase 2 — the overlay, and the forms.** `POST /api/overlay`; class-driven node forms with the
relationship rule above; claim tags as controls. Still no writes to disk — the form's output is
a buffer the overlay judges, and the person saves through their own editor or through Phase 3.
An honest and slightly odd state, and worth shipping: it is a linting sandbox for a node before
it exists.

**Phase 3 — writes.** Gated on [RFC-0029](0029-write-tier.md). If `act` lands, the editor's writes
are its operations behind a second framing, for the reason
[`http.rs:9`](../../yidam/cli/src/cmd/serve/http.rs#L9) gives about the first: *"Not a second
contract. `super::handle` is the seam RFC-0005 left for exactly this."* If RFC-0029 settles the
other way, this phase needs its own argument and does not get to assume one.

**And there is a composition question RFC-0029's gate does not answer**, addressed below.

**Phase 4 — the gates.** `ci-editor` mise task and a CI job mirroring `vscode`; the asset
staleness check; a Dependabot npm group (there are exactly two today); a row in
[`editor-setup.md`](../editor-setup.md) and in [`yidam/editors/README.md`](../../yidam/editors/README.md);
a sidebar entry, without which `astro build` fails.

### The identity gate, and the case it did not have

RFC-0029 §2.2 gates the write tier on authorship rather than on sequencing:

> The `act` capability is declarable only where a git author identity exists. Over stdio that
> identity exists today: the server is a subprocess of a person's shell inside their checkout …
> Over HTTP no author exists until #427 lands in a shape that yields a **stable subject claim**
> mapped onto a committer identity — so an HTTP server MUST NOT declare `act` until then.

`yidam edit` is the first case where that rule's criterion and its proxy come apart. The
criterion is *does a git author identity exist*; the proxy is *stdio, not HTTP*. This surface is
HTTP — and it is also a process a person started from their own shell, inside their own checkout,
bound to loopback, serving the page to the browser on the same machine. Every clause of the
stdio justification holds; only the socket differs.

Two honest readings, and this RFC does not pick one:

- **The sentence binds as written.** An HTTP server may not declare `act`, so Phase 3 waits on
  #427 — a question about remote authorisation that a loopback editor does not raise. RFC-0029
  names this cost itself, about the neighbouring case: re-blocking on #427 *"parks stdio-local
  writes on a question they do not have."*
- **The criterion binds, and the transport was its proxy.** A loopback server started by the
  corpus's owner is in the stdio position, and the gate should say so in terms of the author
  rather than the socket.

The second reading is the one this RFC would argue for, and it is **not this RFC's to settle**:
RFC-0029 is where that sentence lives, and sharpening it is an amendment to RFC-0029 with a dated
block, not a paragraph here. What belongs here is the case, stated precisely enough to be
decided — and the note that if the first reading stands, Phase 3 blocks on #427 and the epic
should say so rather than discovering it at implementation time.

### How it is tested

- The extension's fixture is reused, not rebuilt: `mise run ext-fixture` stages the reports
  golden corpus as a real repository, and `edit` is driven against that. What CI checks and what
  a person sees stay one repository, which is why the task exists.
- Route handlers take plain values and return them, following `http.rs`'s stated discipline —
  *"Everything that decides whether a request is served … takes plain values and returns an
  `Outcome`. None of it needs a socket"* — so the policy is unit-testable without binding a port.
- The design gate covers this surface **only if the file extensions match**. See
  [§ Open questions](#open-questions).

### What this deliberately is not

- **Not deployable, and not deployed.** No hosted instance, no `--bind 0.0.0.0` in any
  documentation, no path over an exported bundle. Any of those makes it #236, and #236 is
  closed by decision.
- **Not a second MCP surface.** Agents have `serve --mcp`. Nothing here is reachable by one.
- **Not a re-implementation of any check.** The boundary is RFC-0016's, unchanged.
- **Not a replacement for the extension or the LSP.** Three clients, one contract. A person with
  a configured editor should keep using it; this is for the person who has not got one.
- **Not a bootstrap wizard**, for RFC-0016's reason: genesis is an ontology dialogue with an
  agent, and a form that skips the dialogue produces the unconsidered ontology the dialogue
  exists to prevent.
- **Not a sangha surface.** Positions and resolutions are governed by the constitution; RFC-0016
  put anything that writes one out of scope pending the governance RFCs, and that holds here.

## Migration & compatibility

Nothing in a derived repository changes. `edit` is a new subcommand in the default feature set;
every existing command, format and exit code is untouched. No layer bumps, no tag is introduced,
no channel is added, and `.yidam.toml` gains nothing.

The binary grows by the size of the built assets. That is a real cost against a light build whose
whole point is that it carries no ML runtime and no system C library, and it should be measured in
Phase 1 rather than asserted here. If it turns out to be large enough to matter, the fallback is a
feature gate — which reopens the crates.io question in [§ Open questions](#open-questions), and is
why the measurement belongs in Phase 1 rather than Phase 4.

## Alternatives considered

- **Astro on `@astrojs/node`, published to npm.** The richer development loop, and rejected. It
  adds a Node runtime to a project whose distribution story is *one binary, five channels*; it
  adds a fifth artifact to Layer 4 with its own version, tag, registry and channel check; and it
  puts a Node process between the browser and the verdicts — which does not violate RFC-0016's
  boundary, but is the only shape here in which violating it is *possible*. The dev loop is
  recoverable anyway: `astro dev` against a running `yidam edit` is Phase 1's inner loop.
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

- **Committed build output, or a feature gate?** The RFC recommends committing, on
  `packaging.rs`'s reasoning. The cost is real and the alternative — gating `edit` off by default
  so crates.io never needs the assets — breaks *one artifact reached five ways*, which is a
  stronger promise than a clean diff. Genuinely a decision, and the binary-size measurement from
  Phase 1 should inform it.
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
  appears on any quality page — [`astro.config.mjs:231-234`](../../yidam/web/docs/astro.config.mjs#L231-L234):
  *"this is a build-time renderer: React produces HTML and none of it is shipped to a reader."*
  This surface would be the first consumer to ship them to a browser. Whether they survive
  client bundling is unknown and is a Phase 1 spike, not an assumption.
- **Does RFC-0029's identity gate reach a loopback editor?** The case is argued in
  [§ The identity gate, and the case it did not have](#the-identity-gate-and-the-case-it-did-not-have)
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
- **The port, and coexistence.** `--http` defaults to 8787. Whether `edit` takes a neighbouring
  default, and whether both may run against one corpus at once, is unsettled and cheap to settle.
