/**
 * RFC-0030's Phase 1 spike, kept answered.
 *
 * # The question, and the answer
 *
 * #606: *"the design system's React components have never been hydrated —
 * `yidam/web/docs/astro.config.mjs:243-246` records that no `client:*` directive appears on
 * any quality page, so this is a build-time renderer: React produces HTML and none of it is
 * shipped to a reader. Whether they survive client bundling is unknown. Find out here, before
 * Phase 2's forms depend on it."*
 *
 * **They survive.** Measured against `mise run edit-dev` — the app on the reports golden
 * corpus — driven with headless Chrome over the DevTools protocol:
 *
 *   - the island hydrated: Astro's client runtime removed the `ssr` marker from it;
 *   - the design system's `Input` is a *controlled* component and its `onChange` reached
 *     React — typing `gauge` took the table from 4 rows to 1, and the helper text from
 *     "4 nodes" to "1 of 4";
 *   - its `useState` focus ring works under a real dispatched click: the border went from
 *     `#d5cfc4` (`--border-ui`) to `#d4a400` (`--border-focus` → `--gold-400`) and the box
 *     shadow to `--shadow-focus-gold`, so the tokens resolve in the client bundle too;
 *   - the console was clean.
 *
 * # What this file can hold, and what it cannot
 *
 * Not that. The `editor-web` job has no browser and should not grow one for this: a gate that
 * needs a browser to run is a gate that gets skipped on the day it matters, which is the
 * property `ci.yml` states as deliberate for this job — *"it needs no yidam binary"* — and
 * would be giving up.
 *
 * What survives without one is the half that actually rots: whether the island is still
 * *shipped*. Hydration working is a fact about React and Vite and does not silently change;
 * an island quietly ceasing to exist is a one-character edit. Delete `client:load` from
 * `browse.astro` and every page still renders, every other test here still passes, and the
 * answer above becomes false with nothing to say so. That is what this asserts.
 *
 * It reads `dist/`, so it needs a build. `ci-editor-web` builds before it tests for exactly
 * this reason, and the failure below says so rather than making a reader work it out.
 */

import { strict as assert } from 'node:assert'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const pkg = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const CLIENT = path.join(pkg, 'dist', 'client', '_astro')

const built = existsSync(CLIENT)
const chunks = built
  ? readdirSync(CLIENT)
      .filter((f) => f.endsWith('.js'))
      .map((f) => ({ name: f, text: readFileSync(path.join(CLIENT, f), 'utf8') }))
  : []

test('the build is there to read', () => {
  // Loud rather than skipped. `ci-editor-web` runs `npm run build` and then `npm run test`,
  // and a suite that quietly passes because there was nothing to look at is the exact shape
  // `no_gate_passes_on_an_empty_scan` exists to refuse one file over.
  assert.ok(built, `no client build at ${CLIENT}. Run \`npm run build\` first — ci-editor-web does.`)
  assert.ok(chunks.length >= 2, `found only ${chunks.length} client chunks`)
})

test('the page still asks for an island', () => {
  // Source-level, because this is the edit that would silently un-answer the spike: a
  // `client:*` directive is one word, and removing it leaves a page that renders perfectly.
  const browse = readFileSync(path.join(pkg, 'src', 'pages', 'browse.astro'), 'utf8')
  assert.match(
    browse,
    /<NodeTable\s+client:load/,
    'browse.astro no longer hydrates NodeTable. If that is deliberate, this package has ' +
      'stopped answering RFC-0030 Phase 1’s spike and #606 needs to say so.',
  )
})

test('the island is bundled for the browser', () => {
  const island = chunks.find((c) => c.name.startsWith('NodeTable'))
  assert.ok(
    island,
    `no NodeTable chunk in ${CLIENT} — found ${chunks.map((c) => c.name).join(', ')}`,
  )
})

test('the design system travelled into the client bundle with it', () => {
  // The load-bearing assertion, and the one an island of our own would not make. A wrapper
  // component proves Astro and React work together; it proves nothing about
  // `yidam/design/components/`, which is what had never been hydrated and what Phase 2's
  // forms are written against.
  //
  // These two strings are `Input.jsx`'s and only `Input.jsx`'s — the focus ring it draws from
  // `useState`. Finding them in a chunk the browser downloads means the component was
  // bundled rather than externalised, tree-shaken to a shell, or resolved to the `.d.ts`.
  // If the design system renames either token this test goes red, which is correct: the
  // contents of the client bundle would have changed and somebody should look.
  const island = chunks.find((c) => c.name.startsWith('NodeTable'))
  assert.ok(island, 'no island chunk to read')
  for (const marker of ['shadow-focus-gold', 'border-focus']) {
    assert.ok(
      island.text.includes(marker),
      `the island chunk does not contain \`${marker}\`, so the design system's Input is not ` +
        'in it. The spike this file records was run against a bundle that had it.',
    )
  }
})

test('the React client runtime ships beside it', () => {
  // Without this the island is inert markup: the component chunk is downloaded and nothing
  // ever calls `hydrateRoot`. `client.*.js` is the renderer entry `@astrojs/react` emits.
  assert.ok(
    chunks.some((c) => c.name.startsWith('client')),
    `no React client entry in ${CLIENT} — found ${chunks.map((c) => c.name).join(', ')}`,
  )
})
