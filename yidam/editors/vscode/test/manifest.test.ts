/**
 * The manifest must agree with the code it points at.
 *
 * This is the class of error unit tests cannot reach and a type-checker does not see: a
 * `main` that names a file the build does not produce, a contributed command id nothing
 * registers, an activation event that never fires. Each fails at runtime in an editor, and
 * each is a string typo.
 *
 * It is checked here in plain node rather than under `@vscode/test-electron` deliberately.
 * The `vscode`-importing code is the status bar, the diagnostic collection, and one
 * `TreeDataProvider` adapter with no judgement in it — every shape decision is settled in
 * `tree/model.ts` against plain data. An Electron download that asserts VS Code renders a
 * `TreeItem` is testing VS Code. These assertions catch the defects that are actually ours
 * — a view id nothing provides, a command id nothing registers — and run anywhere.
 */

import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'

import { TASKS } from '../src/settings.ts'

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

/**
 * A menu naming a `viewItem` no row carries is a menu that never appears, and a row carrying
 * a `contextValue` no menu names is scaffolding. Both look exactly like working code. Six
 * context values sat in `tree/model.ts` with zero menus reading them until this test existed.
 */
test('every menu names a context value some row actually carries, and every view exists', () => {
  const modelSrc = fs.readFileSync(path.join(ROOT, 'src/tree/model.ts'), 'utf8')
  const declared = new Set([...modelSrc.matchAll(/context: '([^']+)'/g)].map((m) => m[1]))
  assert.ok(declared.size > 0, 'no context values found — the scan is broken')

  const viewIds = new Set<string>(
    Object.values(manifest.contributes?.views ?? {})
      .flat()
      .map((v) => (v as { id: string }).id),
  )
  const items: { command: string; when?: string }[] =
    manifest.contributes?.menus?.['view/item/context'] ?? []
  assert.ok(items.length > 0, 'no view/item/context menus — this test is asserting nothing')

  for (const { command, when } of items) {
    const viewItem = /viewItem == ([\w.]+)/.exec(when ?? '')?.[1]
    assert.ok(viewItem, `${command}: a row menu must name a viewItem — ${when}`)
    assert.ok(
      declared.has(viewItem),
      `${command} is offered on \`${viewItem}\`, which no row in tree/model.ts sets`,
    )
    const view = /view == ([\w.]+)/.exec(when ?? '')?.[1]
    assert.ok(view && viewIds.has(view), `${command} names view \`${view}\`, which is not contributed`)
  }
})

/**
 * These two are handed the row they were invoked on. From the palette there is no row, so
 * they would silently do nothing — which reads as a broken command rather than an
 * inapplicable one.
 */
test('row-only commands are hidden from the palette', () => {
  const hidden = new Set(
    (manifest.contributes?.menus?.commandPalette ?? [])
      .filter((m: { when?: string }) => m.when === 'false')
      .map((m: { command: string }) => m.command),
  )
  for (const command of ['yidam.neighborhoodOf', 'yidam.newNodeIn']) {
    assert.ok(hidden.has(command), `${command} needs a row and must not be offered without one`)
  }
})

test('every contributed command is registered in the source', () => {
  const commands: { command: string }[] = manifest.contributes?.commands ?? []
  assert.ok(commands.length > 0, 'no contributed commands — the scan is broken')
  const taskNames = new Set(TASKS.map((t) => t.name))
  for (const { command } of commands) {
    // The `yidam.task.*` family is registered by iterating TASKS, so a literal scan cannot
    // see it. Pinning it to that list is the stronger check anyway: the manifest and the
    // task provider offer the same set or this fails.
    const asTask = command.startsWith('yidam.task.') ? command.slice('yidam.task.'.length) : null
    if (asTask !== null) {
      assert.ok(taskNames.has(asTask), `${command} is contributed but not in TASKS`)
      continue
    }
    assert.ok(
      extensionSrc.includes(`registerCommand('${command}'`),
      `${command} is contributed but never registered`,
    )
  }
  for (const name of taskNames) {
    assert.ok(
      commands.some((c) => c.command === `yidam.task.${name}`),
      `TASKS carries ${name} and the manifest does not contribute a command for it`,
    )
  }
})

/**
 * A task provider whose type nothing declares does not appear in `Run Task`.
 */
test('the task type the provider registers is the one the manifest declares', () => {
  const defs: { type: string }[] = manifest.contributes?.taskDefinitions ?? []
  assert.equal(defs.length, 1)
  assert.ok(
    extensionSrc.includes(`registerTaskProvider('${defs[0].type}'`),
    `${defs[0].type} is declared but nothing provides it`,
  )
})

/** A setting the code reads and the manifest does not declare is invisible in Settings. */
test('every yidam setting the code reads is declared', () => {
  const props: Record<string, unknown> = manifest.contributes?.configuration?.properties ?? {}
  for (const m of extensionSrc.matchAll(/getConfiguration\('yidam'\)[\s\S]{0,40}?get<[^>]+>\('([^']+)'\)/g)) {
    assert.ok(`yidam.${m[1]}` in props, `yidam.${m[1]} is read but not declared`)
  }
  for (const m of extensionSrc.matchAll(/affectsConfiguration\('(yidam\.[^']+)'\)/g)) {
    assert.ok(m[1] in props, `${m[1]} is watched but not declared`)
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

test('every contributed view is provided at activation', () => {
  const views: { id: string }[] = Object.values(manifest.contributes?.views ?? {}).flat() as {
    id: string
  }[]
  assert.equal(views.length, 5, 'RFC-0016 Phase 1 is five views')
  for (const { id } of views) {
    assert.ok(
      extensionSrc.includes(`createTreeView('${id}'`),
      `${id} is contributed but nothing provides it`,
    )
  }
})

test('the views live in the container the manifest declares, and its icon exists', () => {
  const containers = manifest.contributes?.viewsContainers?.activitybar ?? []
  assert.equal(containers.length, 1)
  const container = containers[0]
  assert.ok(
    Object.keys(manifest.contributes?.views ?? {}).every((k) => k === container.id),
    'a view group keyed to no declared container renders nowhere',
  )
  assert.ok(
    fs.existsSync(path.join(ROOT, container.icon)),
    `${container.icon} is declared but not shipped`,
  )
})

/**
 * The sangha view is gated on a context key, and something has to set it.
 *
 * Ungated, every repository that governs itself individually gets a permanently empty box
 * for a mode it has not opted into.
 */
test('the sangha view is gated on a context key the code sets', () => {
  const views: { id: string; when?: string }[] = Object.values(
    manifest.contributes?.views ?? {},
  ).flat() as { id: string; when?: string }[]
  const sangha = views.find((v) => v.id === 'yidam.sangha')!
  assert.equal(sangha.when, 'yidam.collective')
  assert.ok(
    extensionSrc.includes("'yidam.collective'"),
    'the context key is declared in `when` but never set',
  )
})

/** A menu pointing at a command id nothing declares renders a button that does nothing. */
test('every menu entry names a contributed command', () => {
  const declared = new Set(
    (manifest.contributes?.commands ?? []).map((c: { command: string }) => c.command),
  )
  const entries: { command: string }[] = Object.values(manifest.contributes?.menus ?? {}).flat() as {
    command: string
  }[]
  assert.ok(entries.length > 0, 'no menu entries — the scan is broken')
  for (const { command } of entries) {
    assert.ok(declared.has(command), `${command} is in a menu but not contributed`)
  }
})
