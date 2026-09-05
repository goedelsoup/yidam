# `@yidam/edit` — the editor that arrives in a terminal

The third client of one contract, and the first that needs no configured editor to reach.

```
npx @yidam/edit            # in a checkout with a .yidam/ corpus
npx @yidam/edit --root /srv/corpus --port 9000 --no-open
```

Specified by [RFC-0030](../../../docs/rfcs/0030-standalone-editor.md), **as amended
2026-09-05**. This is Phase 1 — the read surface. No writes, no overlay.

## What it is

An Astro application on `@astrojs/node`, served over loopback from a person's own checkout.
It renders a corpus and the verdicts on it. **It computes no verdict of its own**: every
finding on every page came off an RFC-0001 envelope printed by the `yidam` binary the
repository pins, resolved in the order [`binary.ts`](src/lib/binary.ts) documents.

| | |
|---|---|
| Status | `yidam status` |
| Browse | `yidam graph` — nodes, classes, and edges the CLI already resolved |
| Reports | `yidam lint` and `yidam graph-check`, in that order |
| Open questions | `yidam open-questions` |

## The boundary, and why it is a test here

RFC-0016's rule is **TypeScript computes affordances; the CLI computes verdicts.** Under
RFC-0030's original shape that rule was unreachable rather than merely forbidden: the surface
was compiled into the binary, so the JavaScript that shipped had nothing to compute a verdict
*from*. The reversal took that property away.

So the rule is [three gates](test/boundary.mjs), and they land in this phase rather than the
last one, because a gate written after the code it governs is a gate written around it:

- nothing under `src/` imports a corpus-evaluating module, or anything outside the package root;
- a report envelope is parsed in exactly one place, [`src/lib/cli.ts`](src/lib/cli.ts);
- every route under `src/pages/api/` reaches its payload through the spawn.

Mutation-test them before trusting them. `the scan sees a population` is the guard against the
guard — a file-scanning check that looks at nothing passes, which is how a lint reads 40 files
and reports nothing on a tree with 21 hand-written hex colours in it.

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
`yidam/design/` for stylesheets and nowhere else.

Every colour in `app.css` is a `var(--…)` and none is a literal. That keeps this surface inside
`design_tokens.rs`'s scan whichever way #611 is decided: the scan's extension list is `css`,
`astro`, `jsx`, and `tsx` is not on it.

## Working on it

```
npm install
npm run build      # produces dist/server/entry.mjs, which bin/yidam-edit.mjs starts
npm test           # boundary gates, origin rule, parity, flags, root mismatch
npm run dev        # astro dev
```

Or `mise run ci-editor-web` from the repository root, which is what CI runs.

`npm test` needs no binary and no corpus: everything it asserts is a property of this package.
Driving the app against a real corpus uses `mise run ext-fixture`, which stages the reports
golden corpus as a real repository — so what CI checks and what a person sees stay one
repository.

## What is not here yet

The overlay and the ontology-driven forms are Phase 2 (#607), and the reversal made that phase
the expensive one: the overlay is reachable only through `yidam serve --lsp`, so it needs a
supervised child and an LSP bridge rather than the in-process call the original design had.
Writes are Phase 3 (#608), gated on RFC-0029's build. The npm name, the Layer 4 row, the publish
path and the channel check are #610 and Phase 4 (#609) — **nothing here is published, and the
`@yidam` scope is not yet registered.**
