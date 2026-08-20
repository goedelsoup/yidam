/**
 * Link navigation, asserted without an editor.
 *
 * The dividing line under test: **the CLI resolves edges, this file reads a line and ranks a
 * list.** Every `resolved` and every `exists` in these fixtures came from
 * `yidam graph --format json`; nothing here recomputes either. What is asserted is the
 * scalar reader, the path arithmetic that has to work on a buffer the report has not seen,
 * and the completion ordering — all of which fail by not helping.
 */

import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import { test } from 'node:test'

import { resolveBinary } from '../src/binary.ts'
import {
  hoverFor,
  lineOfTarget,
  referencesTo,
  relationshipCandidates,
  relativeFrom,
  resolveFrom,
  scaffold,
  scalarAt,
  slugify,
  sorted,
  targetCandidates,
  type GraphReport,
} from '../src/graph.ts'
import { readHandshake } from '../src/handshake.ts'

const GRAPH: GraphReport = {
  format_version: '1',
  yidam: { version: '0.1.0', commit: 'abc1234', features: ['reports'] },
  root: '/r',
  corpus_dir: '.yidam/corpus',
  nodes: [
    {
      node: 'concept/tailwater.yml',
      class: 'concept',
      label: 'Tailwater',
      description: 'Water downstream of a structure.',
      links: [
        {
          target: '../concept/low-flow.yml',
          relationship: 'relates-to',
          resolved: 'concept/low-flow.yml',
          exists: true,
        },
        {
          target: '../concept.ont.yml',
          relationship: 'instance-of',
          resolved: 'concept.ont.yml',
          exists: true,
        },
      ],
    },
    {
      node: 'concept/low-flow.yml',
      class: 'concept',
      label: 'Low flow',
      description: 'A discharge regime.',
      links: [
        {
          target: '../gauge/ohio-river.yml',
          relationship: 'measured-by',
          resolved: 'gauge/ohio-river.yml',
          exists: true,
        },
      ],
    },
    {
      node: 'gauge/ohio-river.yml',
      class: 'gauge',
      label: 'Ohio River gauge',
      description: 'A stage gauge.',
      links: [],
    },
  ],
  classes: [
    {
      class: 'concept',
      label: 'Concept',
      description: 'A unit of understanding.',
      properties: [{ name: 'datum', type: 'string', description: 'Which vertical datum.' }],
      edges: [
        {
          relationship: 'relates-to',
          target: 'concept',
          direction: 'out',
          description: 'A bare association.',
        },
        {
          relationship: 'measured-by',
          target: 'gauge',
          direction: 'out',
          description: 'A gauge measures this.',
        },
        {
          relationship: 'observed-by',
          target: 'gauge',
          direction: 'in',
          description: 'Authored on the gauge.',
        },
        // One relationship, two destinations — the shape a real ontology uses three times.
        {
          relationship: 'derived-from',
          target: 'gauge',
          direction: 'out',
          description: 'Read off an instrument.',
        },
        {
          relationship: 'derived-from',
          target: 'concept',
          direction: 'out',
          description: 'Inferred from another concept.',
        },
      ],
    },
    { class: 'gauge', label: 'Gauge', description: 'An instrument.', properties: [], edges: [] },
  ],
}

// ── reading the line ────────────────────────────────────────────────────────

test('the target scalar is read only when the cursor is inside it', () => {
  const line = '  - target: ../concept/low-flow.yml'
  assert.equal(scalarAt(line, 14, 'target')!.value, '../concept/low-flow.yml')
  // Just past the last character is still the value — that is where a click lands.
  assert.equal(scalarAt(line, line.length, 'target')!.value, '../concept/low-flow.yml')
  assert.equal(scalarAt(line, 4, 'target'), null, 'on the key, not the value')
  assert.equal(scalarAt(line, 14, 'relationship'), null, 'a different key')
})

test('quotes and trailing comments are not part of the path', () => {
  const q = scalarAt('    target: "../gauge/g.yml"', 20, 'target')!
  assert.equal(q.value, '../gauge/g.yml')
  assert.equal('    target: "../gauge/g.yml"'.slice(q.start, q.end), '../gauge/g.yml')
  assert.equal(scalarAt('    target: ../g.yml # a note', 15, 'target')!.value, '../g.yml')
})

test('an empty value is not a scalar', () => {
  assert.equal(scalarAt('  - target:', 11, 'target'), null)
  assert.equal(scalarAt('  - target:   ', 12, 'target'), null)
})

// ── path arithmetic ─────────────────────────────────────────────────────────

/**
 * Mirrors the CLI's `normalize(dir.join(target))`. It exists on this side only because a
 * buffer can be edited after the report was taken — the *authority* on what an edge resolves
 * to is still the report, which carries `resolved` for every committed edge.
 */
test('a target resolves the way the CLI resolved it', () => {
  assert.equal(resolveFrom('concept/tailwater.yml', '../gauge/g.yml'), 'gauge/g.yml')
  assert.equal(resolveFrom('concept/tailwater.yml', './low-flow.yml'), 'concept/low-flow.yml')
  assert.equal(resolveFrom('concept/tailwater.yml', '../concept.ont.yml'), 'concept.ont.yml')
  // Climbing above the corpus root is the empty string, as the CLI reports it.
  assert.equal(resolveFrom('concept/tailwater.yml', '../../elsewhere.yml'), '')
})

test('a path back out is the one a person would have written', () => {
  assert.equal(relativeFrom('concept/tailwater.yml', 'gauge/g.yml'), '../gauge/g.yml')
  assert.equal(relativeFrom('concept/tailwater.yml', 'concept/low-flow.yml'), 'low-flow.yml')
  assert.equal(relativeFrom('concept/tailwater.yml', 'concept.ont.yml'), '../concept.ont.yml')
})

test('resolving and un-resolving are inverses', () => {
  for (const id of ['gauge/g.yml', 'concept/low-flow.yml', 'concept.ont.yml']) {
    assert.equal(resolveFrom('concept/tailwater.yml', relativeFrom('concept/tailwater.yml', id)), id)
  }
})

// ── references and hover ────────────────────────────────────────────────────

/**
 * The traversal nothing surfaced: `used-by` covers catalog entries only, and `orphan-in`
 * reports the *absence* of inbound edges without ever naming the present ones.
 */
test('inbound edges are found from the resolved side', () => {
  const refs = referencesTo(GRAPH, 'concept/low-flow.yml')
  assert.equal(refs.length, 1)
  assert.equal(refs[0].from, 'concept/tailwater.yml')
  assert.equal(refs[0].relationship, 'relates-to')
  // The raw text comes along, because that is what has to be found in the file.
  assert.equal(refs[0].target, '../concept/low-flow.yml')
  assert.deepEqual(referencesTo(GRAPH, 'concept/tailwater.yml'), [])
})

test('the line of an inbound edge is found, and missing is line 0 rather than dropped', () => {
  const text = ['class: concept', 'links:', '  - target: ../a.yml', '  - target: ../b.yml'].join('\n')
  assert.equal(lineOfTarget(text, '../b.yml'), 3)
  assert.equal(lineOfTarget(text, '../nowhere.yml'), 0)
})

test('hover names the node, its class, and its degree in both directions', () => {
  const text = hoverFor(GRAPH, 'concept/low-flow.yml')!
  assert.match(text, /Low flow/)
  assert.match(text, /`concept`/)
  assert.match(text, /1 out · 1 in/)
  assert.match(text, /A discharge regime/)
})

/**
 * An `.ont.yml` is a legitimate target — every node carries an `instance-of` edge to one —
 * and it is not a node. Saying what it is beats saying nothing.
 */
test('hovering a class definition says what the class is', () => {
  const text = hoverFor(GRAPH, 'concept.ont.yml')!
  assert.match(text, /Concept/)
  assert.match(text, /A unit of understanding/)
  assert.equal(hoverFor(GRAPH, 'nowhere/at-all.yml'), null)
})

// ── completion ──────────────────────────────────────────────────────────────

/**
 * The ontology is a guide, not a closed list.
 *
 * Measured on a live derived repository at 90 nodes: 17 (class, relationship) pairs in use
 * are undeclared, and `instance-of` — carried by every node — is declared by none. A list
 * restricted to declared edges would be stricter than any rule in the system.
 */
test('relationships offer the declared ones first and the ones in use beside them', () => {
  const cs = sorted(relationshipCandidates(GRAPH, 'concept'))
  assert.deepEqual(
    cs.map((c) => c.label),
    ['derived-from', 'measured-by', 'relates-to', 'instance-of'],
  )
  assert.equal(cs[0].rank, 0)
  const inUse = cs.find((c) => c.label === 'instance-of')!
  assert.equal(inUse.rank, 1, 'in use, undeclared')
  assert.match(inUse.detail, /in use/)
  assert.match(inUse.documentation, /does not describe it/)
})

/**
 * One relationship may be declared against several classes, and this listed it once per
 * declaration.
 *
 * Found by reading a real ontology rather than by thinking: three of its classes do it —
 * `maneuver -[operates-on]->` legislation, ballot-measure and election. The visible symptom
 * was `concerns` appearing twice in a completion list. The invisible one was worse, and it
 * is the next test.
 */
test('a relationship declared against several classes is one offer', () => {
  const cs = relationshipCandidates(GRAPH, 'concept').filter((c) => c.label === 'derived-from')
  assert.equal(cs.length, 1)
  assert.equal(cs[0].detail, '→ gauge | concept')
  assert.match(cs[0].documentation, /\*\*gauge\*\* — Read off an instrument/)
  assert.match(cs[0].documentation, /\*\*concept\*\* — Inferred from another concept/)
})

/**
 * …and it offered targets from only the first of them.
 *
 * That is the half a user would never report as a bug: two thirds of the legal targets
 * simply absent, which reads as "those nodes do not exist".
 */
test('a relationship declared against several classes offers all of their instances', () => {
  const labels = sorted(targetCandidates(GRAPH, 'concept/tailwater.yml', 'derived-from')).map(
    (c) => c.label,
  )
  assert.deepEqual(labels, ['../gauge/ohio-river.yml', 'low-flow.yml'])
})

/** An `in` edge is authored on the other side, so it is not offered here. */
test('inbound-declared relationships are not offered', () => {
  const labels = relationshipCandidates(GRAPH, 'concept').map((c) => c.label)
  assert.ok(!labels.includes('observed-by'))
})

test('targets are class-filtered by the declared edge, as paths from the editing node', () => {
  const cs = sorted(targetCandidates(GRAPH, 'concept/tailwater.yml', 'measured-by'))
  assert.deepEqual(
    cs.map((c) => c.label),
    ['../gauge/ohio-river.yml'],
  )
  assert.match(cs[0].detail, /gauge/)
})

/** Never offer the node you are editing as its own target. */
test('a node is not offered as its own target', () => {
  const labels = targetCandidates(GRAPH, 'concept/tailwater.yml', 'relates-to').map((c) => c.label)
  assert.deepEqual(labels, ['low-flow.yml'])
})

/**
 * `instance-of` is the case no class filter could ever find: declared by nobody, and
 * pointing at an `.ont.yml` rather than at an instance.
 */
test('an undeclared relationship falls back to what the corpus already does', () => {
  const cs = targetCandidates(GRAPH, 'concept/low-flow.yml', 'instance-of')
  assert.deepEqual(
    cs.map((c) => c.label),
    ['../concept.ont.yml'],
  )
  assert.match(cs[0].detail, /in use/)
})

/**
 * Offering nothing because the ontology is silent would be worse than offering too much,
 * and the ontology is silent for 17 of the pairs a real corpus uses.
 */
test('a relationship nobody has used offers the whole corpus', () => {
  const cs = targetCandidates(GRAPH, 'concept/tailwater.yml', 'invented-just-now')
  assert.equal(cs.length, GRAPH.nodes.length - 1, 'everything but itself')
  assert.ok(cs.every((c) => c.rank === 2))
})

// ── new node ────────────────────────────────────────────────────────────────

test('a label becomes a stable kebab-case filename', () => {
  assert.equal(slugify('Tailwater regime'), 'tailwater-regime')
  assert.equal(slugify('  Issue 1 — August 2023 '), 'issue-1-august-2023')
})

/**
 * The scaffold carries the class's declared properties and exactly one edge.
 *
 * A node with no outgoing edge is a lint error the moment it exists, so `relationship` and
 * `target` are required fields of the input rather than optional ones — the type refuses to
 * express the file that would break the gate.
 */
test('a scaffolded node carries its properties and its first edge', () => {
  const body = scaffold(
    {
      class: 'concept',
      name: 'stage-datum',
      label: 'Stage datum',
      description: 'The reference plane. [open]',
      relationship: 'measured-by',
      target: 'gauge/ohio-river.yml',
    },
    GRAPH.classes[0],
  )
  assert.match(body, /^class: concept$/m)
  assert.match(body, /^label: Stage datum$/m)
  assert.match(body, /^  datum: ""   # string$/m)
  assert.match(body, /^  # Which vertical datum\.$/m)
  assert.match(body, /^  - target: \.\.\/gauge\/ohio-river\.yml$/m)
  assert.match(body, /^    relationship: measured-by$/m)
  // The description is quoted, so a `:` or a `#` in it cannot break the file.
  assert.match(body, /^description: "The reference plane\. \[open\]"$/m)
})

test('a class with no declared properties scaffolds without an empty block', () => {
  const body = scaffold(
    {
      class: 'gauge',
      name: 'x',
      label: 'X',
      description: 'y',
      relationship: 'r',
      target: 'concept/tailwater.yml',
    },
    GRAPH.classes[1],
  )
  assert.ok(!body.includes('properties:'))
  assert.match(body, /^  - target: \.\.\/concept\/tailwater\.yml$/m)
})

// ── against the real binary ─────────────────────────────────────────────────

const HERE = path.dirname(new URL(import.meta.url).pathname)
const FIXTURE = path.resolve(HERE, '../../../prelude/sdks/parity/fixtures/reports/basic/repo')
const SKIP = 'no yidam speaking the report contract — set YIDAM_BIN, or `cargo install --path yidam/cli`'

function capture(bin: string, args: string[], cwd: string): string {
  try {
    return execFileSync(bin, args, { cwd, encoding: 'utf8' })
  } catch (err) {
    return (err as { stdout?: string }).stdout ?? ''
  }
}

function stageFixture(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'yidam-graph-'))
  fs.cpSync(FIXTURE, dir, { recursive: true })
  const git = (...args: string[]) => execFileSync('git', args, { cwd: dir, stdio: 'pipe' })
  git('init', '-q', '-b', 'main')
  git('config', 'user.email', 'fixture@yidam.test')
  git('config', 'user.name', 'Fixture')
  git('add', '-A')
  git('commit', '-q', '-m', 'genesis: reports fixture')
  return dir
}

async function contractBinary(cwd: string): Promise<string | null> {
  const r = await resolveBinary({ configured: process.env.YIDAM_BIN ?? '', workspace: cwd })
  const required = (process.env.YIDAM_REQUIRE_CONTRACT ?? '') !== ''
  if (!r.command) {
    if (required) throw new Error(`YIDAM_REQUIRE_CONTRACT is set and no yidam resolved: ${r.reason}`)
    return null
  }
  const h = readHandshake(capture(r.command, ['status', '--format', 'json'], cwd))
  if (!h.ok) {
    if (required) throw new Error(`YIDAM_REQUIRE_CONTRACT is set and ${r.command} does not speak it`)
    return null
  }
  return r.command
}

/**
 * The one that matters: this side's path arithmetic against the CLI's, edge for edge.
 *
 * `resolveFrom` exists only so navigation works in a buffer the report has not seen. If it
 * ever disagreed with `resolved`, ctrl-click would land somewhere the gate does not think
 * the edge points — and nothing else would notice.
 */
test('every edge the binary resolved resolves the same way here', async (t) => {
  const dir = stageFixture()
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  const g = JSON.parse(capture(bin, ['graph', '--format', 'json'], dir)) as GraphReport
  assert.equal(g.corpus_dir, '.yidam/corpus')
  assert.ok(g.nodes.length > 0 && g.classes.length > 0)

  let checked = 0
  for (const n of g.nodes) {
    for (const l of n.links) {
      assert.equal(resolveFrom(n.node, l.target), l.resolved, `${n.node} -> ${l.target}`)
      // `relativeFrom` renders the *shortest* path, which is not always the one the author
      // wrote — `../concept/x.yml` and `x.yml` are the same edge from a sibling, and this
      // corpus uses the long form. So the invariant is the round trip, not the spelling.
      assert.equal(resolveFrom(n.node, relativeFrom(n.node, l.resolved)), l.resolved)
      checked += 1
    }
  }
  assert.ok(checked > 0, 'the fixture has edges')

  // The fixture carries one deliberately broken edge, and this side does not second-guess it.
  const broken = g.nodes.flatMap((n) => n.links).filter((l) => !l.exists)
  assert.equal(broken.length, 1, 'the fixture carries exactly one dangling edge')
  assert.notEqual(broken[0].resolved, '', 'a broken edge still resolves — you can go create it')

  // And the ontology reaches the completion list.
  const cs = sorted(relationshipCandidates(g, 'concept'))
  assert.ok(cs.some((c) => c.rank === 0), 'the fixture class declares out-edges')
  assert.ok(
    !cs.some((c) => c.label === 'measured-by'),
    'a `direction: in` edge is authored on the other side',
  )
  // No relationship is offered twice, whatever the ontology declares.
  assert.equal(new Set(cs.map((c) => c.label)).size, cs.length)
})
