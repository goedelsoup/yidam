/**
 * RFC-0016's boundary, as a test rather than a sentence.
 *
 * > **TypeScript computes affordances. The CLI computes verdicts.**
 *
 * Under the shape RFC-0030 originally proposed, that rule was *unreachable* rather than
 * merely forbidden: with no Node process anywhere, the JavaScript that shipped could not
 * compute a verdict because it had nothing to compute one from. The reversal recorded in
 * RFC-0030's 2026-09-05 amendment took that property away. There is now a process holding a
 * parsed corpus, and every check in this repository is a pure function over exactly that — so
 * a re-derivation is reachable by a contributor with good intentions and an afternoon.
 *
 * These three gates are the price of the reversal, and they land in Phase 1 rather than Phase
 * 4 on purpose: a gate written after the code it governs is a gate written around it.
 *
 * **Mutation-test them before trusting them.** A file-scanning check that looks at nothing
 * passes. `no_gate_passes_on_an_empty_scan` below is the guard against the guard — it is the
 * property `design_tokens.rs` was missing when a lint read 40 files and reported nothing on a
 * tree with 21 hand-written hex colours in it.
 */

import { strict as assert } from 'node:assert'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const pkg = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const SRC = path.join(pkg, 'src')
const BIN = path.join(pkg, 'bin')

/**
 * The one directory outside this package that `src/` may reach, and why.
 *
 * `yidam/design/` is the design system, imported for its stylesheet the way
 * `yidam/web/docs/src/styles/custom.css` imports it — relative, resolved by Vite, inlined
 * into `dist/` at build time. It is a build input rather than a runtime dependency, so the
 * packing rule below does not reach it: by the time `npm publish` packs `dist/`, the bytes
 * are already in there.
 *
 * Copying it under this root instead was tried and was worse. `design_tokens.rs` walks the
 * repository and reads any `.css` outside `yidam/design/` as a *consumer*, so a committed
 * copy of the palette fails the raw-colour gate — correctly, because a committed copy of the
 * palette is the exact failure that gate exists to stop.
 */
const BUILD_TIME_ALLOWED = path.resolve(pkg, '..', '..', 'design')

/** Every source file under a directory, discovered rather than listed. */
function sources(dir = SRC, found = []) {
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry)
    if (statSync(full).isDirectory()) {
      sources(full, found)
    } else if (/\.(ts|tsx|js|jsx|astro|mjs|css)$/.test(entry)) {
      // `.css` too, and not as an afterthought: the one import in this package that leaves
      // the package root is `app.css`'s `@import` of the design system, so a scan that read
      // only JavaScript would be a scan that never saw the case the rule is about.
      found.push(full)
    }
  }
  return found
}

const FILES = sources().map((file) => ({
  rel: path.relative(pkg, file),
  text: readFileSync(file, 'utf8'),
}))

/** `bin/` ships as source, unbundled, so its imports are held to a stricter rule. */
const BIN_FILES = sources(BIN, []).map((file) => ({
  rel: path.relative(pkg, file),
  text: readFileSync(file, 'utf8'),
}))

/**
 * Every import specifier in a file, in all four spellings.
 *
 * The side-effect form — `import './styles/app.css'`, with no `from` — is the one that
 * matters most and the one a `from`-only scan silently drops. It is how this app loads its
 * stylesheets, so it is also how a module from outside the package root would most naturally
 * arrive. A mutation test caught this: two gates below stayed green while the mutation they
 * exist to catch sat in the file.
 */
export function specifiers(text) {
  const out = []
  // `import … from 'x'` and `export … from 'x'`
  for (const m of text.matchAll(/\bfrom\s+['"]([^'"]+)['"]/g)) out.push(m[1])
  // `import 'x'` — side effect only, no binding
  for (const m of text.matchAll(/\bimport\s+['"]([^'"]+)['"]/g)) out.push(m[1])
  // `await import('x')` and `require('x')`
  for (const m of text.matchAll(/\b(?:import|require)\(\s*['"]([^'"]+)['"]\s*\)/g)) out.push(m[1])
  return out
}

test('the scan sees a population', () => {
  // The guard against the guard. Every assertion below is a scan, and a scan over nothing
  // passes silently — which is the failure mode that makes a green gate worse than no gate.
  assert.ok(FILES.length >= 8, `found only ${FILES.length} source files under src/`)
  const withImports = FILES.filter((f) => specifiers(f.text).length > 0)
  assert.ok(withImports.length >= 5, `found only ${withImports.length} files with imports`)

  // All four spellings, because a scan that reads three of them is a scan that reports
  // nothing about the fourth. The side-effect form is how the stylesheets arrive, and a
  // `from`-only regex dropped it silently until a mutation test said so.
  const probe = [
    "import { a } from 'x/from'",
    "export { b } from 'x/reexport'",
    "import 'x/side-effect'",
    "await import('x/dynamic')",
    "require('x/require')",
  ].join('\n')
  assert.deepEqual(specifiers(probe).sort(), [
    'x/dynamic',
    'x/from',
    'x/reexport',
    'x/require',
    'x/side-effect',
  ])
})

test('nothing in bin/ escapes the package root', () => {
  // `bin/` is published as source and run by node directly — nothing bundles it — so its
  // imports must exist in the tarball. `npm publish` packs only what lives under the package
  // root, which is the identical property `packaging.rs` records for `cargo package` under
  // the crate root, with the identical failure signature: the import resolves in the working
  // tree and in every CI job, and is absent from the tarball. Two near-miss releases already
  // paid for that lesson one ecosystem over.
  assert.ok(BIN_FILES.length >= 2, `found only ${BIN_FILES.length} files under bin/`)
  for (const { rel, text } of BIN_FILES) {
    for (const spec of specifiers(text)) {
      if (!spec.startsWith('.')) continue
      const resolved = path.resolve(path.dirname(path.join(pkg, rel)), spec)
      assert.ok(
        resolved.startsWith(pkg + path.sep),
        `${rel} imports ${spec}, which resolves outside the package root. ` +
          'bin/ ships unbundled, so npm publish would not pack it.',
      )
    }
  }
})

test('src/ escapes the package root only for the design system', () => {
  // Vite inlines what `src/` imports, so a build-time asset is in `dist/` before anything is
  // packed. That is a different question from `bin/`'s, and answering both with one rule was
  // what pushed an earlier draft into committing a copy of the palette — which the raw-colour
  // gate then failed, correctly.
  //
  // One allowed directory, not a general escape hatch: `yidam/design/`, the way
  // `yidam/web/docs` already imports it.
  for (const { rel, text } of FILES) {
    for (const spec of specifiers(text)) {
      if (!spec.startsWith('.')) continue
      const resolved = path.resolve(path.dirname(path.join(pkg, rel)), spec)
      if (resolved.startsWith(pkg + path.sep)) continue
      assert.ok(
        resolved.startsWith(BUILD_TIME_ALLOWED + path.sep),
        `${rel} imports ${spec}, which resolves outside the package root and outside ` +
          'yidam/design/. Only the design system may be reached at build time.',
      )
      assert.ok(
        spec.endsWith('.css'),
        `${rel} imports ${spec} from yidam/design/. Only stylesheets: a JavaScript module ` +
          'from there would be a shared runtime this package has not argued for.',
      )
    }
  }
})

test('nothing imports a corpus-evaluating module', () => {
  // `@yidam/core` is a parity surface with real corpus logic in it, which makes it the single
  // most likely accidental route to a re-derivation. Types and affordances are fine; the
  // evaluating functions are not, and the cheapest place to draw that line is the import.
  const FORBIDDEN = ['@yidam/core', 'yidam-core']
  for (const { rel, text } of FILES) {
    for (const spec of specifiers(text)) {
      assert.ok(
        !FORBIDDEN.some((f) => spec === f || spec.startsWith(`${f}/`)),
        `${rel} imports ${spec}. Verdicts come from the binary; see src/lib/cli.ts.`,
      )
    }
  }

  const manifest = JSON.parse(readFileSync(path.join(pkg, 'package.json'), 'utf8'))
  for (const dep of Object.keys(manifest.dependencies ?? {})) {
    assert.ok(!FORBIDDEN.includes(dep), `package.json depends on ${dep}`)
  }
})

test('a report is parsed in exactly one place', () => {
  // The only route from bytes to a verdict is `spawnReport`, and the only `JSON.parse` of a
  // report envelope lives beside it. A second parse is how a second contract starts: a page
  // reading a reshaped envelope can disagree with `yidam lint` without anybody editing a
  // check.
  const ALLOWED = new Set([
    path.join('src', 'lib', 'cli.ts'),
    path.join('src', 'lib', 'handshake.ts'),
  ])
  const parsers = FILES.filter((f) => f.text.includes('JSON.parse')).map((f) => f.rel)
  for (const rel of parsers) {
    assert.ok(
      ALLOWED.has(rel),
      `${rel} calls JSON.parse. Reports enter this process through spawnReport and nowhere else.`,
    )
  }
  assert.ok(
    parsers.length >= 2,
    'expected cli.ts and handshake.ts to parse — the scan has stopped seeing them',
  )
})

test('every verdict route goes through the spawn', () => {
  // A route under `src/pages/api/` that never mentions the spawn is a route serving something
  // this process made up.
  const routes = FILES.filter((f) => f.rel.startsWith(path.join('src', 'pages', 'api')))
  assert.ok(routes.length >= 3, `found only ${routes.length} API routes`)
  for (const { rel, text } of routes) {
    assert.ok(
      text.includes('spawnReport') || text.includes('reportRoute'),
      `${rel} serves a payload that did not come from the binary.`,
    )
  }
})
