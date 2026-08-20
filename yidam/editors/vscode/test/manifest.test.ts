/**
 * The manifest must agree with the code it points at.
 *
 * This is the class of error unit tests cannot reach and a type-checker does not see: a
 * `main` that names a file the build does not produce, a contributed command id nothing
 * registers, an activation event that never fires. Each fails at runtime in an editor, and
 * each is a string typo.
 *
 * It is checked here in plain node rather than under `@vscode/test-electron` deliberately.
 * The Electron harness is worth its cost once there is behaviour to assert — diagnostics,
 * tree views — and at that point it belongs with the feature that introduced them. Today
 * the only `vscode`-importing code is status-bar wiring, and an Electron download that
 * asserts a status-bar string is ceremony. These assertions catch the same defects and run
 * anywhere.
 */

import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const ROOT = path.resolve(HERE, '..')
const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'))
const extensionSrc = fs.readFileSync(path.join(ROOT, 'src/extension.ts'), 'utf8')

test('main names a file the build actually emits', () => {
  const main = manifest.main as string
  // `out/extension.js` must come from `src/extension.ts` under this tsconfig's rootDir.
  const expected = path.join(ROOT, main.replace(/^\.\//, ''))
  const source = expected.replace(/^.*\/out\//, '').replace(/\.js$/, '.ts')
  assert.ok(
    fs.existsSync(path.join(ROOT, 'src', source)),
    `main is ${main} but src/${source} does not exist`,
  )
})

test('every contributed command is registered in the source', () => {
  const commands: { command: string }[] = manifest.contributes?.commands ?? []
  assert.ok(commands.length > 0, 'no contributed commands — the scan is broken')
  for (const { command } of commands) {
    assert.ok(
      extensionSrc.includes(`registerCommand('${command}'`),
      `${command} is contributed but never registered`,
    )
  }
})

test('every registered command is contributed', () => {
  // The inverse: a command registered and not declared is invisible in the palette,
  // which reads to a user as a feature that does not exist.
  const declared = new Set(
    (manifest.contributes?.commands ?? []).map((c: { command: string }) => c.command),
  )
  for (const m of extensionSrc.matchAll(/registerCommand\('([^']+)'/g)) {
    assert.ok(declared.has(m[1]), `${m[1]} is registered but not contributed`)
  }
})

test('activation is scoped to a yidam repository', () => {
  const events: string[] = manifest.activationEvents ?? []
  assert.ok(
    events.some((e) => e.includes('.yidam')),
    'the extension must not activate in unrelated workspaces',
  )
  assert.ok(!events.includes('*'), 'activating on `*` makes every editor session pay for this')
})

test('the configured setting the code reads is the one the manifest declares', () => {
  const props = manifest.contributes?.configuration?.properties ?? {}
  assert.ok('yidam.path' in props, 'yidam.path must be declared to appear in Settings')
  assert.ok(
    extensionSrc.includes("getConfiguration('yidam')") && extensionSrc.includes("get<string>('path')"),
    'yidam.path is declared but never read',
  )
})
