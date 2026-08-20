/**
 * The three guards — and the two copies they force this extension to hold.
 *
 * Claim colours live in `yidam/design/tokens/colors.css` and claim tokens live in
 * `yidam/cli/src/claims.rs`. The extension needs both at runtime and can reach neither: it
 * ships without the design directory, and it is not going to parse Rust. So it transcribes
 * them — and every transcription in this repository has to be checked rather than trusted,
 * because a copy nobody compares is a copy that has already drifted and nobody knows yet.
 */

import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'

import { DARK, findClaims, LIGHT, shouldDecorate, TAGS } from '../src/claims.ts'
import { readonlyUpdate, schemaDelta, TASKS, VENDOR_GLOB } from '../src/settings.ts'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const REPO = path.resolve(HERE, '../../../..')

// ── the two transcriptions ──────────────────────────────────────────────────

/**
 * The palette is the design system's to decide. This asserts the copy, in both themes.
 *
 * The dark triad was added for this feature and derived rather than picked: the light
 * theme's border tone becomes the dark foreground, because it is already the mid-lightness
 * member and therefore the one legible against both grounds.
 */
test('the claim palette matches the design tokens', () => {
  const css = fs.readFileSync(path.join(REPO, 'yidam/design/tokens/colors.css'), 'utf8')
  const value = (name: string): string | null => {
    const m = new RegExp(`--${name}:\\s*([^;]+);`).exec(css)
    return m ? m[1].trim() : null
  }
  for (const tag of TAGS) {
    for (const part of ['bg', 'fg', 'border'] as const) {
      assert.equal(value(`${tag}-${part}`), LIGHT[tag][part], `--${tag}-${part}`)
      assert.equal(value(`${tag}-${part}-dark`), DARK[tag][part], `--${tag}-${part}-dark`)
    }
  }
})

/** The tokens are the prelude's, defined in `guidelines/agent-conduct.md` and counted in Rust. */
test('the claim tokens match the ones the CLI counts', () => {
  const rs = fs.readFileSync(path.join(REPO, 'yidam/cli/src/claims.rs'), 'utf8')
  const declared = [...rs.matchAll(/pub const [A-Z]+: &str = "\[(\w+)\]";/g)].map((m) => m[1])
  assert.deepEqual(declared.sort(), [...TAGS].sort())
})

// ── matching ────────────────────────────────────────────────────────────────

/**
 * Wherever the CLI counts, which is anywhere in the file.
 *
 * Not "inside a `description:` block": the corpus records absence in properties too, and a
 * decoration that skipped those would disagree with `yidam status` about what a claim is.
 */
test('claims are found in prose and in property values alike', () => {
  const hits = findClaims(
    ['description: A regime. [verified]', 'estimate: "[open] — not computed"'].join('\n'),
  )
  assert.deepEqual(
    hits.map((h) => [h.tag, h.line]),
    [
      ['verified', 0],
      ['open', 1],
    ],
  )
})

test('the span covers the bracketed token exactly', () => {
  const [hit] = findClaims('x [inference] y')
  assert.deepEqual([hit.start, hit.end], [2, 13])
  assert.equal('x [inference] y'.slice(hit.start, hit.end), '[inference]')
})

/**
 * Exact tokens, and the same trade the CLI makes.
 *
 * Corpus prose is dense with markdown links, so a looser bracket match would read
 * `[open questions](…)` as an open claim. Exact matching has no false positives to trade
 * against *except* a link whose text is exactly `[open]` — which `count_in_source` also
 * counts. Agreeing with the CLI matters more than being right in isolation: an editor that
 * tinted a different set than `yidam status` counts would be a disagreement nobody sees.
 */
test('the match is exact, and matches what the CLI counts', () => {
  assert.deepEqual(findClaims('see [open questions](../README.md) and [verifiedish]'), [])
  assert.equal(findClaims('see [open](x) really').length, 1, 'as the CLI counts it too')
})

test('several tags on one line are all found', () => {
  const hits = findClaims('[open] then [open] then [verified]')
  assert.deepEqual(
    hits.map((h) => h.start),
    [24, 0, 12],
  )
})

/**
 * A high-contrast theme is a stated accessibility choice. Tinting text against it overrides
 * a decision the reader made deliberately.
 */
test('decoration is off in high-contrast themes whatever the setting says', () => {
  assert.equal(shouldDecorate(1, true), true, 'light')
  assert.equal(shouldDecorate(2, true), true, 'dark')
  assert.equal(shouldDecorate(3, true), false, 'high contrast')
  assert.equal(shouldDecorate(4, true), false, 'high contrast light')
  assert.equal(shouldDecorate(1, false), false, 'the setting still wins downward')
})

// ── the vendor guard ────────────────────────────────────────────────────────

/**
 * Activation runs on every window. A guard that rewrote the setting each time would put a
 * workspace-settings change in every session's git status.
 */
test('the vendor guard is idempotent', () => {
  assert.deepEqual(readonlyUpdate(undefined, true), { [VENDOR_GLOB]: true })
  assert.equal(readonlyUpdate({ [VENDOR_GLOB]: true }, true), null)
})

test('the vendor guard leaves other patterns alone', () => {
  const next = readonlyUpdate({ '**/generated/**': true }, true)
  assert.deepEqual(next, { '**/generated/**': true, [VENDOR_GLOB]: true })
})

/**
 * Turning the guard off removes the entry rather than setting it false, so the setting is
 * left as it was found rather than carrying our own "not read-only" claim forever.
 */
test('turning the guard off removes the entry it added', () => {
  assert.deepEqual(readonlyUpdate({ [VENDOR_GLOB]: true, 'x': true }, false), { x: true })
  assert.equal(readonlyUpdate({ x: true }, false), null, 'nothing of ours to remove')
})

/** An explicit `false` the user wrote is ours to correct, because it is our pattern. */
test('an explicitly disabled guard is re-enabled when asked for', () => {
  assert.deepEqual(readonlyUpdate({ [VENDOR_GLOB]: false }, true), { [VENDOR_GLOB]: true })
})

// ── schema wiring ───────────────────────────────────────────────────────────

const DESIRED = {
  'yaml.schemas': {
    './.yidam/schemas/corpus-instance.json': '.yidam/corpus/**/*.yml',
    './.yidam/schemas/ontology.json': '.yidam/corpus/*.ont.yml',
  },
  'files.associations': { '*.ont.yml': 'yaml' },
}

test('nothing to offer when the settings already say it', () => {
  assert.deepEqual(
    schemaDelta(DESIRED, (k) => DESIRED[k as keyof typeof DESIRED]),
    [],
  )
})

/**
 * Merged, never replaced.
 *
 * `yaml.schemas` is somewhere people put their own mappings, and an "apply" that overwrote
 * them would be a worse failure than the copy-paste it replaces.
 */
test('applying merges rather than replacing', () => {
  const mine = { './my-schema.json': 'src/**/*.yml' }
  const deltas = schemaDelta(DESIRED, (k) => (k === 'yaml.schemas' ? mine : undefined))
  const yaml = deltas.find((d) => d.key === 'yaml.schemas')!
  assert.equal(yaml.value['./my-schema.json'], 'src/**/*.yml')
  assert.equal(yaml.value['./.yidam/schemas/ontology.json'], '.yidam/corpus/*.ont.yml')
  assert.equal(yaml.changed.length, 2)
})

/** A mapping pointing at the wrong glob is stale, not absent — and is still offered. */
test('a stale mapping counts as changed', () => {
  const stale = { './.yidam/schemas/ontology.json': 'wrong/**' }
  const deltas = schemaDelta(
    { 'yaml.schemas': DESIRED['yaml.schemas'] },
    () => stale,
  )
  assert.deepEqual(deltas[0].changed.sort(), [
    './.yidam/schemas/corpus-instance.json',
    './.yidam/schemas/ontology.json',
  ])
})

// ── tasks ───────────────────────────────────────────────────────────────────

/**
 * The five RFC-0016 names, and not every task in `mise.yidam.toml`.
 *
 * That file carries a dozen REGEN generators which `regen` runs in one pass; offering them
 * individually re-creates the two-lists problem `yidam regen` exists to have solved.
 */
test('the offered tasks are the ones the inherited layer actually defines', () => {
  const toml = fs.readFileSync(path.join(REPO, 'mise.yidam.toml'), 'utf8')
  for (const t of TASKS) {
    assert.ok(toml.includes(`[${t.name}]`), `mise.yidam.toml has no [${t.name}] task`)
  }
  assert.deepEqual(
    TASKS.map((t) => t.name),
    ['regen', 'graph-check', 'graph-lint', 'embed', 'index-build'],
  )
})
