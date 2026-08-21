/**
 * The neighbourhood panel: grouping, escaping, and the offline line.
 *
 * The traversal is not tested here because it is not here — `yidam neighbors` performs it,
 * from the same function `serve --mcp` calls. What is asserted is layout, and the two
 * properties a page rendering authored prose inside an editor has to hold: nothing it
 * fetches, and nothing it executes on someone else's behalf.
 */

import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'

import { capture, contractBinary, SKIP, stageFixture } from './stage.ts'
import {
  escape,
  groups,
  render,
  TOKENS,
  type NeighborRow,
  type NeighborsReport,
} from '../src/neighborhood.ts'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const REPO = path.resolve(HERE, '../../../..')

function row(over: Partial<NeighborRow>): NeighborRow {
  return {
    node: 'concept/x.yml',
    class: 'concept',
    label: 'X',
    description: '',
    relationship: 'relates-to',
    direction: 'out',
    hops: 1,
    is_node: true,
    ...over,
  }
}

function report(neighbors: NeighborRow[], over: Partial<NeighborsReport> = {}): NeighborsReport {
  return {
    format_version: '1',
    yidam: { version: '0.1.0', commit: 'abc1234', features: ['reports'] },
    root: '/r',
    corpus_dir: '.yidam/corpus',
    node: 'concept/tailwater.yml',
    class: 'concept',
    label: 'Tailwater',
    description: 'Water downstream of a structure.',
    depth: 2,
    neighbors,
    ...over,
  }
}

// ── grouping ────────────────────────────────────────────────────────────────

/**
 * Hop first because distance is the reader's question. Outbound before inbound: what this
 * node asserts, then what asserts about it.
 */
test('groups order by hop, then direction, then relationship', () => {
  const gs = groups(
    report([
      row({ node: 'a.yml', hops: 2, direction: 'out', relationship: 'zeta' }),
      row({ node: 'b.yml', hops: 1, direction: 'in', relationship: 'alpha' }),
      row({ node: 'c.yml', hops: 1, direction: 'out', relationship: 'omega' }),
      row({ node: 'd.yml', hops: 1, direction: 'out', relationship: 'beta' }),
    ]),
  )
  assert.deepEqual(
    gs.map((g) => `${g.hops}${g.direction}:${g.relationship}`),
    ['1out:beta', '1out:omega', '1in:alpha', '2out:zeta'],
  )
})

/**
 * Direction is part of the key, not a column.
 *
 * At hops > 1 it is relative to the node the edge was reached *from*, so putting an inbound
 * and an outbound row under one arrow would make the arrow a lie.
 */
test('the same relationship in both directions is two groups', () => {
  const gs = groups(
    report([
      row({ node: 'a.yml', direction: 'out', relationship: 'cites' }),
      row({ node: 'b.yml', direction: 'in', relationship: 'cites' }),
    ]),
  )
  assert.equal(gs.length, 2)
})

test('rows inside a group sort by label, falling back to the path', () => {
  const gs = groups(
    report([
      row({ node: 'z.yml', label: 'Alpha' }),
      row({ node: 'a.yml', label: '' }),
      row({ node: 'y.yml', label: 'Beta' }),
    ]),
  )
  assert.deepEqual(
    gs[0].rows.map((r) => r.label || r.node),
    ['a.yml', 'Alpha', 'Beta'],
  )
})

// ── the offline line ────────────────────────────────────────────────────────

/**
 * RFC-0016: "Fully offline — no CDN. The repo's CI is hermetic and the extension should hold
 * the same line."
 *
 * Asserted against the rendered bytes rather than against intent, because the way this
 * regresses is somebody adding one convenient `<link>`.
 */
test('the page reaches for nothing', () => {
  const html = render(report([row({})]), 'NONCE')
  assert.ok(!/https?:\/\//.test(html), 'no absolute URL anywhere')
  assert.ok(!/<link\b/i.test(html), 'no stylesheet link')
  assert.ok(!/<img\b/i.test(html), 'no image')
  assert.ok(!/@import/.test(html), 'no CSS import')
  assert.ok(!/<script[^>]+src=/i.test(html), 'no external script')
})

/**
 * The CSP admits the one inline style and the one inline script, by nonce, and nothing else.
 */
test('the content security policy is default-src none', () => {
  const html = render(report([]), 'NONCE')
  assert.match(html, /default-src 'none'/)
  assert.match(html, /style-src 'nonce-NONCE'/)
  assert.match(html, /script-src 'nonce-NONCE'/)
  assert.ok(!html.includes("'unsafe-inline'"))
  assert.match(html, /<style nonce="NONCE">/)
  assert.match(html, /<script nonce="NONCE">/)
})

/**
 * A corpus is authored prose from a repository an editor merely opened.
 *
 * Escaping is what keeps a `<` in a description from truncating the panel; it also makes
 * injected markup inert, which is the version that matters.
 */
test('authored text cannot become markup', () => {
  const html = render(
    report([row({ label: '<img src=x onerror=alert(1)>', description: '"quoted" & <angled>' })], {
      label: '</h1><script>alert(1)</script>',
    }),
    'NONCE',
  )
  assert.ok(!html.includes('<img src=x'), 'the label did not become a tag')
  assert.ok(!html.includes('<script>alert(1)'), 'nor did the centre node')
  assert.match(html, /&lt;img src=x/)
  assert.match(html, /&quot;quoted&quot; &amp; &lt;angled&gt;/)
  // Exactly one script tag: ours.
  assert.equal(html.match(/<script/g)!.length, 1)
})

test('escape covers every context this page uses', () => {
  assert.equal(escape(`<&>"'`), '&lt;&amp;&gt;&quot;&#39;')
  assert.equal(escape('&amp;'), '&amp;amp;', 'ampersand first, so nothing double-escapes')
})

// ── states ──────────────────────────────────────────────────────────────────

test('no open node is an invitation rather than an empty panel', () => {
  const html = render(report([], { node: '', label: '', class: '', description: '' }), 'N')
  assert.match(html, /Open a corpus node/)
  // The script's selector string mentions `data-depth`; the *button* must not exist.
  assert.ok(!/<button/.test(html), 'no depth control with nothing to control')
})

test('a node with no neighbours says which two things are absent', () => {
  const html = render(report([], { depth: 1 }), 'N')
  assert.match(html, /no edges out, and nothing points here/)
})

/**
 * A reached target that is not a corpus node — an `.ont.yml`, or a broken edge — is shown
 * and not linked. Hiding it would make the neighbourhood disagree with the graph.
 */
test('a reached non-node is shown, unlinked, and labelled', () => {
  const html = render(report([row({ node: 'concept.ont.yml', is_node: false, label: '' })]), 'N')
  assert.match(html, /not a node/)
  assert.ok(!html.includes('data-node="concept.ont.yml"'), 'nothing to open')
})

test('the depth control marks the depth in force', () => {
  const html = render(report([], { depth: 2 }), 'N')
  assert.match(html, /data-depth="2" aria-pressed="true"/)
  assert.match(html, /data-depth="1" aria-pressed="false"/)
})

// ── design tokens ───────────────────────────────────────────────────────────

/**
 * Spacing and radii are the design system's to decide, and the extension ships without it.
 *
 * Colour is deliberately absent from this transcription: the palette is the reader's theme,
 * because the design system has no dark mode outside the claim triad and a light card inside
 * a dark editor is worse than one that is not brand-coloured.
 */
test('the spacing and radius tokens match the design system', () => {
  const css = ['spacing', 'borders']
    .map((f) => fs.readFileSync(path.join(REPO, `yidam/design/tokens/${f}.css`), 'utf8'))
    .join('\n')
  for (const [name, value] of Object.entries(TOKENS)) {
    const m = new RegExp(`${name}:\\s*([^;]+);`).exec(css)
    assert.ok(m, `${name} is not in the design tokens at all`)
    assert.equal(m![1].trim(), value, name)
  }
})

/**
 * The brand font layer is an `@import` from the Google Fonts CDN, so an offline surface
 * cannot use it. This pins the reason rather than leaving it to a comment.
 */
test('the font token layer is CDN-bound, which is why this page uses the editor’s font', () => {
  const fonts = fs.readFileSync(path.join(REPO, 'yidam/design/tokens/fonts.css'), 'utf8')
  assert.match(fonts, /@import url\('https:\/\/fonts\.googleapis\.com/)
  assert.match(render(report([]), 'N'), /--vscode-font-family/)
})

// ── against the real binary ─────────────────────────────────────────────────





/**
 * The panel, rendered from what the binary actually reports.
 *
 * The fixture reaches a broken edge at hop 2, so this covers the arm a hand-written fixture
 * would be tempted to leave out: a neighbour that exists in the graph and not on disk.
 */
test('the panel renders the real report, broken edge and all', async (t) => {
  const dir = stageFixture('yidam-nbhd-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  const parsed = JSON.parse(
    capture(bin, ['neighbors', 'concept/tailwater.yml', '--depth', '2', '--format', 'json'], dir),
  ) as NeighborsReport

  assert.equal(parsed.node, 'concept/tailwater.yml')
  assert.equal(parsed.label, 'Tailwater')
  assert.equal(parsed.neighbors.length, 4)

  const broken = parsed.neighbors.filter((n) => !n.is_node)
  assert.equal(broken.length, 1, 'the fixture carries one deliberately broken edge')
  assert.equal(broken[0].hops, 2)

  const html = render(parsed, 'NONCE')
  assert.match(html, /Low flow/)
  assert.match(html, /not a node/)
  assert.ok(!/https?:\/\//.test(html))
  // An inbound group, which is the one this panel could not show before the fixture
  // carried a second class: the gauge authors `measured-by`, so tailwater is reached
  // from it rather than reaching it.
  assert.deepEqual(
    groups(parsed).map((g) => `${g.hops}${g.direction}:${g.relationship}`),
    ['1out:relates-to', '2out:depends-on', '2in:measured-by'],
  )
})
