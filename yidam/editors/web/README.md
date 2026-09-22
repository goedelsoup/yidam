# `@goedelsoup/yidam-edit` — the editor that arrives in a terminal

The third client of one contract, and the first that needs no configured editor to reach.

```
npx @goedelsoup/yidam-edit            # in a checkout with a .yidam/ corpus
npx @goedelsoup/yidam-edit --root /srv/corpus --port 9000 --no-open
```

Specified by [RFC-0030](../../../docs/rfcs/0030-standalone-editor.md), **as amended
2026-09-05**. Phase 1 is the read surface; Phase 3 added the one write the contract has —
`propose`, through the `act` tier, onto a branch; Phase 2 added the overlay and the node form —
a buffer judged as it is typed, by `yidam serve --lsp`, and never saved.

## What it is

An Astro application on `@astrojs/node`, served over loopback from a person's own checkout.
It renders a corpus and the verdicts on it. **It computes no verdict of its own**: every
finding on every page came off an RFC-0001 envelope printed by the `yidam` binary the
repository pins, resolved in the order [`binary.ts`](src/lib/binary.ts) documents.

| | |
|---|---|
| Status | `yidam status` |
| Browse | `yidam graph` — nodes, classes, and edges the CLI already resolved |
| Node | one node: its class's declared properties, its edges out, and its edges **in** |
| Reports | `yidam lint` and `yidam graph-check`, in that order |
| Open questions | `yidam open-questions` |

The node page's two additions are the two nothing else in the system offers. **The class
declaration beside the instance** — #605's second finding is that a corpus node is a typed
record edited as free text, and showing the values without the contract they answer to
reproduces exactly that. **Inbound edges** — `used-by` covers catalog entries only, and
`orphan-in` reports their *absence* without ever naming the ones that are there.

Both are traversal over what the binary already resolved. Inbound edges are matched on the
envelope's `resolved` path and never on the raw `target`, which is relative to the authoring
node's directory: comparing that text would be this process doing the path resolution
`dangling_edge` owns, arriving as a three-line convenience.

The properties table says what omitting each one costs — **fails the gate** or **reported** —
rather than "required" and "optional". That distinction is `missing-property`'s: it gates on a
property declared `required: true` and reports the rest, and a reader looking at a node wants
to know which findings fail CI more than they want the ontology's vocabulary for it.

It did not reach this surface at first. `OntProperty` in
[`cmd/graph.rs`](../../cli/src/cmd/graph.rs) serialised `name`, `type` and `description` and
stopped, so a draft of this page rendered the column anyway and printed "optional" for every
property in the corpus — a fabricated verdict arriving as a table column, which is the same
failure as one arriving as a check and harder to see. The column was removed, the CLI now
carries the field, and the column is back.

**An absent `required` is not read as optional.** A repository pins the binary that governs it
and this client is versioned independently, so a corpus on a binary older than the field is a
normal state — and `format_version` does not rise for an additive field, by
`report.schema.json`'s own rule. The CLI emits `required` on every property or on none, so the
column appears when the answer exists and is replaced by one sentence naming the binary when it
does not. Both paths were checked against two real binaries rather than mocked.

## The boundary, and why it is a test here

RFC-0016's rule is **TypeScript computes affordances; the CLI computes verdicts.** Under
RFC-0030's original shape that rule was unreachable rather than merely forbidden: the surface
was compiled into the binary, so the JavaScript that shipped had nothing to compute a verdict
*from*. The reversal took that property away.

So the rule is [three gates](test/boundary.mjs), and they land in this phase rather than the
last one, because a gate written after the code it governs is a gate written around it:

- nothing under `src/` imports a corpus-evaluating module, or anything outside the package root;
- the binary's output is parsed in exactly the places that read it — a report in
  [`src/lib/cli.ts`](src/lib/cli.ts), an MCP answer in [`src/lib/act.ts`](src/lib/act.ts) —
  and never a request body;
- every route under `src/pages/api/` reaches its payload through one of the two spawns, and the
  routes under `api/act/` through the act one only;
- the binary is spawned in three files, and none of them puts `propose` on an argv.

Mutation-test them before trusting them. `the scan sees a population` is the guard against the
guard — a file-scanning check that looks at nothing passes, which is how a lint reads 40 files
and reports nothing on a tree with 21 hand-written hex colours in it.

## The write, and why it goes through the MCP server

`POST /api/act/propose` and `POST /api/act/cycle` are RFC-0029's two `act` tools, and the page at
`/act` is a button for the first and a rendering of the second. There is no form: `propose` takes
`{ dry_run }` and nothing else, drafts `open:`/`withdraw:`/`close:` commits from the gate's own
findings on the *committed* corpus, and lands them on a `propose/<head>` branch with `HEAD`
unmoved. Preview is the same run with nothing written.

The route does not spawn `yidam propose`. It spawns `yidam serve --mcp`, one stdio connection
per request, and speaks the protocol to it for one call — because the `[serve] act = true`
declaration, the no-author refusal at startup and the frozen `capability-not-supported` refusal
all live behind the MCP dispatch, and a route that went around them would be the second route
into the tier RFC-0029 §2.2 forbids. So a corpus that has not opted in gets a `409` carrying the
binary's own refusal, verbatim, and the page says which of three states it is in: undeclared
(add the key), a binary that predates the tier (re-pin), or a refusal from a declared server
(the text says why). Nothing in this package decides whether a corpus may be written to.

`POST` only, and `Origin` required — a browser sends it on every `POST`, so a request without
one did not come from a page. Astro's own cross-site check sits in front of that and agrees;
building this found it had been refusing every same-origin `POST`, because it compares against
a `url.origin` it makes up as `http://localhost` unless the host is listed, and
[`astro.config.mjs`](astro.config.mjs) now lists the three loopback names. `force` cannot be
sent from here by any spelling: the frame has no field for it.

## Two flags that are missing on purpose

There is no `--bind`. This server authenticates nobody, and the flag that turns a loopback
editor into a deployed reader is the flag that turns it into #236, which is closed by decision.
A container reaches this by publishing a port; that is the container's decision to make.

There is no `--allow-origin`. `serve --mcp --http` needs one because its client is another
site. Here the only legitimate client is the page this server served, so any other origin is
refused rather than configured.

## Two files are copies, and a test says so

[`src/lib/binary.ts`](src/lib/binary.ts) and [`src/lib/handshake.ts`](src/lib/handshake.ts) are
**byte-identical copies** of the VS Code extension's, which are `vscode`-free by deliberate
design. They are copies rather than imports because `npm publish` packs only what lives under
the package root — the identical property [`packaging.rs`](../../cli/tests/packaging.rs)
records for `cargo package`, and a lesson two near-miss releases already paid for.

[`test/parity.mjs`](test/parity.mjs) holds them byte-identical rather than
same-shape-and-signature, because the *order* in `binary.ts` is its whole content. The cost is
that two of `handshake.ts`'s strings name the extension; [`src/lib/messages.ts`](src/lib/messages.ts)
owns this surface's wording instead, keyed off the failure kind.

## The design system is imported, not copied

[`src/styles/app.css`](src/styles/app.css) opens with `@import "../../../../design/tokens.css"` —
the same string, and the same pattern, as
[`yidam/web/docs/src/styles/custom.css`](../../web/docs/src/styles/custom.css). Vite resolves it
at build time and inlines it into `dist/`, so the system travels inside the published build and
costs no request.

An earlier draft concatenated it into a committed copy under this package root, to satisfy the
packing rule above. That was wrong twice. `design_tokens.rs` walks the repository and reads any
`.css` outside `yidam/design/` as a *consumer*, so the copy failed the raw-colour gate for
holding the palette it was copying — correctly, because a committed copy of the palette is
exactly what that gate exists to stop. And the packing worry did not apply: `files` publishes
`bin` and `dist`, and Vite has inlined the CSS before either exists.

So the escape rule is two rules, and [`test/boundary.mjs`](test/boundary.mjs) states both:
`bin/` ships unbundled and may not reach outside the package root at all; `src/` may reach
`yidam/design/` for its stylesheets and its `index.js`, and nowhere else — never
`components/<group>/**`, which the design system's own adherence lint has forbidden since
before it had a consumer.

Every colour in `app.css` and in the island is a `var(--…)` and none is a literal. That keeps
this surface inside `design_tokens.rs`'s scan: its extension list is `css`, `astro`, `jsx`, and
`tsx` is not on it, which is why the island is `.jsx`. #611 is still open — that choice covers
this surface and not the next one.

## The hydration spike, and its answer

RFC-0030 made Phase 1 answer a question before Phase 2 could depend on it: **the design
system's React components had never been hydrated anywhere.** `yidam/web/docs` renders them at
build time with no `client:*` directive on any page, so React produced HTML and none of it was
ever shipped to a browser. Phase 2's forms are written against those components.

**They survive client bundling.** [`src/islands/NodeTable.jsx`](src/islands/NodeTable.jsx) is
the island — the browse table, filtered in the browser through the design system's `Input`,
chosen because it is from the forms group Phase 2 needs and because it is *stateful*, so a page
where hydration silently failed would look identical until you focused the field. Verified by
running it: `mise run edit-dev`, driven with headless Chrome over the DevTools protocol. The
island hydrated, `onChange` reached React and narrowed the table, the `useState` focus ring
resolved `--border-focus` and `--shadow-focus-gold` in the client bundle, and the console
was clean.

[`test/hydration.mjs`](test/hydration.mjs) holds the half a browserless CI job can hold: that
the island is still *shipped*, and that `Input`'s own code is in the chunk the browser
downloads. Hydration working is a fact about React and Vite that does not silently change; an
island ceasing to exist is one word deleted from `browse.astro`.

Filtering is an affordance, not a verdict. Every row was resolved by the binary before the
component saw it, and substring-matching a list decides what is on a screen rather than
anything about the corpus.

## Working on it

```
npm install
npm run build      # produces dist/server/entry.mjs, which bin/yidam-edit.mjs starts
npm test           # boundary gates, hydration, graph traversal, origin, parity, flags, root
npm run dev        # astro dev
```

Or `mise run ci-editor-web` from the repository root, which is what CI runs. It builds before
it tests, and that order is now load-bearing twice over: the build is what proves `app.css`'s
relative import still resolves, and `test/hydration.mjs` reads `dist/client/` to check the
island still ships. Running `npm test` on its own against no build fails loudly and says so,
rather than skipping.

`npm test` needs no binary and no corpus: everything it asserts is a property of this package.

To drive the app against a real corpus:

```
mise run edit-dev                        # stages the fixture, builds, serves it
YIDAM_EDIT_PORT=4399 mise run edit-dev   # if 8788 is taken
```

That stages the reports golden corpus — the same one the goldens and the extension's tests
assert against, through the same `stage.toml` — as a real git repository at
`.local/ext-fixture`, then serves it through `bin/yidam-edit.mjs` rather than through `astro
dev`, so what runs is the entry point `npx @goedelsoup/yidam-edit` runs. What CI checks and what a person
sees stay one repository.

## The overlay, and why there is a child process

`/draft` is a linting sandbox for a node before it exists. The form is generated from the
ontology (`src/lib/form.ts`: declared properties by type, declared out-edges first in the
relationship picker, relationships already in use beside them with the reason each is offered,
and a free-text escape), and everything it produces is one YAML buffer. The buffer goes to
`POST /api/overlay/change?client=&doc=` and its verdict comes back over `GET /api/overlay`,
server-sent events.

The verdict is computed by `yidam serve --lsp` — the same function that draws the squiggle in
Neovim — because the overlay is reachable through nothing else. `src/lib/lsp.ts` is the client
(Content-Length framing, id correlation, a polite shutdown with a kill behind it) and
`src/lib/overlay.ts` is the supervisor: one child per process, started by the first page to
subscribe, stopped by the last to leave, restarted on the next change after it dies — with every
held buffer replayed — and abandoned after three starts in a minute. A child that dies is
announced on the stream, and the page says *verdicts stopped* rather than showing a clean node
it has no judge for.

A buffer for a file that is not on disk is linted at all only by a CLI that declares
`experimental.yidam.unsavedInstances` (after 0.13.0). On an older binary no check sees it — a
clean verdict is silence — and the page says so rather than showing the silence as clean.

## What is not here yet

A form's save is not `propose` and is not here: it would be a new `act` tool with an input
schema, which is a contract event (RFC-0005) that needs its own argument — RFC-0030's open
question *does Phase 2 write at all?* records where that stands.

## How it ships

Phase 4 (#609). This package is Layer 4's third artifact: a row in
[VERSIONING.md](../../../VERSIONING.md)'s Layer 4 table, a publish path in
[`edit.yml`](../../../.github/workflows/edit.yml) that fires on `edit/v*` and on nothing else,
and a channel check in [`install-channels.yml`](../../../.github/workflows/install-channels.yml)
that asks npm which version it serves. `the_registries_layer_4_names_are_delivered_and_checked`
refuses any two of the three without the other, because a registry named in a versioning
document is read as a promise and #232 is what an unkeepable one costs.

**npm is the only channel, and there is no GitHub release.** Every other Layer 4 artifact has a
file to attach; here the package *is* the artifact. That has one consequence worth knowing
before reading `install-channels.yml`: this is the one layer whose version cannot be resolved
from the releases API, so `edit-released` resolves it from the tag.

[`scripts/check-package.mjs`](scripts/check-package.mjs) is what stands between a green build
and a published tarball that does not run. It packs what `npm publish` would upload, installs
its *production* dependencies in a directory outside this repository, and starts the server
there — which catches both of the failures a file listing cannot: a module imported from
outside the package root, and a runtime import that is only a `devDependency`. It runs in
`ci-editor-web` on every pull request rather than first at the tag, because #871 is the
precedent: `ci (vscode)` never packaged, and two `vsce` refusals stayed green for weeks before
arriving during a release.
