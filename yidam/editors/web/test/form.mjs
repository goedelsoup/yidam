/**
 * The node form's affordances and the buffer they become — #607, RFC-0030 Phase 2.
 *
 * Every function here is pure and every assertion is about shape: what is offered, in which
 * order, with what reason, and what YAML comes out. Whether the YAML is a *good* node is the
 * overlay's answer and is not asked here.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import {
  CLAIM_TAGS,
  blankDraft,
  docOf,
  relationshipOffers,
  relativeFrom,
  render,
  slugify,
  targetOffers,
} from '../src/lib/form.ts'
import { emitNode, isPlain, prose, scalar } from '../src/lib/yaml.ts'

const classes = [
  {
    class: 'gage',
    properties: [
      { name: 'parameter', type: 'string', required: true },
      { name: 'units', type: 'string' },
      { name: 'claim_tag', type: 'claim' },
    ],
    edges: [
      { relationship: 'sources-from', target: 'concept', direction: 'out' },
      { relationship: 'sources-from', target: 'reach', direction: 'out' },
      { relationship: 'measured-by', target: 'gage', direction: 'in' },
    ],
  },
  { class: 'concept', properties: [], edges: [] },
  { class: 'reach', properties: [], edges: [] },
]

const nodes = [
  {
    node: 'gage/canyon-outlet.yml',
    class: 'gage',
    label: 'Canyon Outlet',
    links: [
      { target: '../gage.ont.yml', relationship: 'instance-of', resolved: 'gage.ont.yml', exists: true },
      { target: '../concept/low-flow.yml', relationship: 'sources-from', resolved: 'concept/low-flow.yml', exists: true },
      { target: '../../catalog/nwis.md', relationship: 'sourced-from', exists: true },
    ],
  },
  {
    node: 'concept/low-flow.yml',
    class: 'concept',
    label: 'Low flow',
    links: [
      { target: '../concept.ont.yml', relationship: 'instance-of', resolved: 'concept.ont.yml', exists: true },
      { target: '../reach/canyon.yml', relationship: 'observed-in', resolved: 'reach/canyon.yml', exists: true },
    ],
  },
  { node: 'reach/canyon.yml', class: 'reach', label: 'Canyon reach', links: [] },
]

test('the three tags are the closed list, in the order the check names them', () => {
  assert.deepEqual([...CLAIM_TAGS], ['verified', 'inference', 'open'])
})

test('relationships: declared first, then in use by the class, then elsewhere, each with its reason', () => {
  const offers = relationshipOffers(classes, nodes, 'gage')
  assert.deepEqual(
    offers.map((o) => [o.rank, o.relationship, o.reason]),
    [
      [0, 'sources-from', 'declared by gage → concept | reach'],
      [1, 'instance-of', 'in use by 1 gage node, not declared'],
      [1, 'sourced-from', 'in use by 1 gage node, not declared'],
      [2, 'observed-in', 'in use by 1 node of other classes'],
    ],
  )
  // A declared `in` edge is not an offer: the form writes out-edges.
  assert.ok(!offers.some((o) => o.relationship === 'measured-by'))
  // One relationship declared against two targets is one offer, not two.
  assert.equal(offers.filter((o) => o.relationship === 'sources-from').length, 1)
})

test('a class the ontology says nothing about still gets what the corpus does', () => {
  const offers = relationshipOffers(classes, nodes, 'reach')
  assert.deepEqual(
    offers.map((o) => [o.rank, o.relationship]),
    [
      // Not in use by any reach node, so it is the form's own offer, above the other-class tier.
      [1, 'instance-of'],
      [2, 'observed-in'],
      [2, 'sourced-from'],
      [2, 'sources-from'],
    ],
  )
})

test('instance-of is offered even when nothing in the report carries it', () => {
  // A corpus whose nodes list no links yet — or a report from a binary that omits them —
  // would otherwise offer the form's own default row to nobody.
  const offers = relationshipOffers(classes, [], 'gage')
  assert.deepEqual(
    offers.map((o) => [o.rank, o.relationship, o.reason]),
    [
      [0, 'sources-from', 'declared by gage → concept | reach'],
      [1, 'instance-of', 'the edge every node carries to its class file; declared by none'],
    ],
  )
})

test('targets: every declared class, then what the class already points at, then everything', () => {
  const declared = targetOffers(classes, nodes, 'gage', 'sources-from')
  assert.deepEqual(
    declared.map((t) => [t.rank, t.id]),
    [
      [0, 'concept/low-flow.yml'],
      [0, 'reach/canyon.yml'],
    ],
  )
  // `instance-of` points at an .ont.yml that is not a node: the class's own comes first, and
  // the in-use tier finds any other a node already points at.
  const ont = targetOffers(classes, nodes, 'gage', 'instance-of')
  assert.deepEqual(ont.map((t) => [t.rank, t.id, t.detail]), [[0, 'gage.ont.yml', 'this class']])
  const stray = targetOffers(classes, nodes, 'concept', 'instance-of')
  assert.deepEqual(stray.map((t) => [t.rank, t.id]), [[0, 'concept.ont.yml']])
  // Silence from the ontology is not an empty list.
  const anything = targetOffers(classes, nodes, 'gage', 'never-seen')
  assert.deepEqual(anything.map((t) => t.id), ['concept/low-flow.yml', 'gage/canyon-outlet.yml', 'reach/canyon.yml'])
})

test('relativeFrom writes a target the way the corpus does', () => {
  assert.equal(relativeFrom('gage/new.yml', 'gage.ont.yml'), '../gage.ont.yml')
  assert.equal(relativeFrom('gage/new.yml', 'concept/low-flow.yml'), '../concept/low-flow.yml')
  assert.equal(relativeFrom('gage/new.yml', 'gage/old.yml'), 'old.yml')
  assert.equal(relativeFrom('a/b/c.yml', 'a/d.yml'), '../d.yml')
})

test('slugify keeps filenames plain', () => {
  assert.equal(slugify('Tailwater régime (2024)'), 'tailwater-regime-2024')
  assert.equal(slugify('  --  '), '')
})

test('a blank draft carries the one edge every node has, and its doc is under the class', () => {
  const draft = blankDraft('gage')
  assert.deepEqual(draft.links, [{ relationship: 'instance-of', target: 'gage.ont.yml' }])
  assert.equal(docOf(draft), 'gage/untitled.yml')
  assert.equal(docOf({ ...draft, name: 'new-station' }), 'gage/new-station.yml')
})

test('render: declared order, empties left out, targets made relative, prose as a block', () => {
  const draft = {
    class: 'gage',
    name: 'new-station',
    label: 'New station',
    description: 'First line. [verified]\n\nSecond paragraph. [open]\n',
    properties: { units: 'cfs', claim_tag: 'inference', parameter: '', extra: 'yes' },
    links: [
      { relationship: 'instance-of', target: 'gage.ont.yml' },
      { relationship: 'sources-from', target: 'concept/low-flow.yml' },
      { relationship: 'sourced-from', target: '../../catalog/nwis.md', raw: true },
      { relationship: '', target: '' },
    ],
  }
  assert.equal(
    render(draft, classes[0].properties),
    [
      'class: gage',
      'label: New station',
      'description: |',
      '  First line. [verified]',
      '',
      '  Second paragraph. [open]',
      'properties:',
      '  units: cfs',
      '  claim_tag: inference',
      '  extra: "yes"',
      'links:',
      '  - target: ../gage.ont.yml',
      '    relationship: instance-of',
      '  - target: ../concept/low-flow.yml',
      '    relationship: sources-from',
      '  - target: ../../catalog/nwis.md',
      '    relationship: sourced-from',
      '',
    ].join('\n'),
  )
})

test('scalars: plain when safe, JSON-quoted otherwise', () => {
  for (const plain of ['cfs', 'cubic feet per second', 'Canyon Outlet gage', 'a/b.yml', 'v1.2']) {
    assert.equal(isPlain(plain), true, plain)
    assert.equal(scalar(plain), plain)
  }
  for (const [value, quoted] of [
    ['00060', '"00060"'],
    ['yes', '"yes"'],
    ['No', '"No"'],
    ['null', '"null"'],
    ['', '""'],
    ['a: b', '"a: b"'],
    ['x #y', '"x #y"'],
    ['trailing ', '"trailing "'],
    ['- item', '"- item"'],
    ['ü', '"ü"'],
  ]) {
    assert.equal(isPlain(value), false, value)
    assert.equal(scalar(value), quoted)
  }
})

test('prose: a single line is a scalar; a leading space needs the indicator; trailing newlines are clipped', () => {
  assert.equal(prose('one line', 2), 'one line')
  assert.equal(prose('a\nb\n\n\n', 2), '|\n  a\n  b')
  assert.equal(prose(' indented\nb', 2), '|2\n   indented\n  b')
  assert.equal(prose('a\r\nb', 4), '|\n    a\n    b')
})

test('a node with no links writes an empty list rather than a dangling key', () => {
  assert.equal(
    emitNode({ class: 'c', label: 'l', description: 'd', properties: [], links: [] }),
    'class: c\nlabel: l\ndescription: d\nlinks: []\n',
  )
})
