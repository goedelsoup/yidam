/**
 * The two modules this surface shares with the extension are byte-identical copies.
 *
 * They are copies rather than imports because `npm publish` packs only what lives under the
 * package root — the same property `packaging.rs` records for `cargo package`, with the same
 * failure signature. A shared module would be a fourth Layer 4 artifact unless it stayed
 * private, and RFC-0030 leaves that extraction open rather than forcing it.
 *
 * What it does not leave open is drift. A copy nothing compares is the failure #465 measured
 * in the design system: two consumers that did not import the system but retyped it, ten of
 * twenty-four values drifted, and two colour families swapped outright.
 *
 * **Byte-identical, deliberately.** A fuzzy comparison — same exports, same signatures — would
 * pass a copy whose resolution *order* had changed, and the order is the whole content of
 * `binary.ts`: an explicit setting outranks discovery, the repository's own build outranks
 * PATH, and a shim never silently overrides an explicit install. The strictness is the point.
 *
 * The cost is that two of `handshake.ts`'s user-facing strings name the VS Code extension.
 * `src/lib/messages.ts` owns this surface's wording instead, keyed off the failure kind.
 */

import { strict as assert } from 'node:assert'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const pkg = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const extension = path.join(pkg, '..', 'vscode', 'src')
const here = path.join(pkg, 'src', 'lib')

const SHARED = ['binary.ts', 'handshake.ts']

for (const name of SHARED) {
  test(`${name} is byte-identical to the extension's`, () => {
    const theirs = readFileSync(path.join(extension, name), 'utf8')
    const ours = readFileSync(path.join(here, name), 'utf8')
    assert.equal(
      ours,
      theirs,
      `yidam/editors/web/src/lib/${name} has drifted from yidam/editors/vscode/src/${name}. ` +
        'Copy it across, or make the change in both.',
    )
  })
}

test('the copies carry the rules they exist to carry', () => {
  // A file comparison passes on two empty files. These are the two sentences that make the
  // copies worth having, and their absence would mean the wrong file was compared.
  const binary = readFileSync(path.join(here, 'binary.ts'), 'utf8')
  assert.ok(
    binary.includes('never bundles, downloads, or builds a binary'),
    'binary.ts no longer states the rule it exists for',
  )
  const handshake = readFileSync(path.join(here, 'handshake.ts'), 'utf8')
  assert.ok(
    handshake.includes('never mis-parse, never guess'),
    'handshake.ts no longer states the rule it exists for',
  )
})
