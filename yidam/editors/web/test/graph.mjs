/**
 * Traversal over what the CLI already resolved.
 *
 * Every function under test is pure and takes the report as an argument, so none of this
 * needs a binary — the property `ci.yml`'s `editor-web` job depends on and states out loud.
 *
 * The load-bearing test is `an inbound edge is matched on the resolved path`. `target` is
 * relative to the *authoring* node's directory, so two nodes in different directories can
 * carry byte-identical `target` strings meaning two different files. Matching on it would be
 * this process doing the path resolution `dangling_edge` owns, arriving as a three-line
 * convenience rather than as a decision anybody made.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { classOf, nodeById, nodeHref, referencesTo } from '../src/lib/graph.ts'

/**
 * Two nodes in different directories whose raw `target` text is the same string, resolving to
 * two different files. This is the shape the CLI exists to disambiguate.
 */
const NODES = [
  {
    node: 'concept/low-flow.yml',
    class: 'concept',
    links: [
      { target: '../concept/tailwater.yml', relationship: 'relates-to', resolved: 'concept/tailwater.yml', exists: true },
      { target: '../concept/gone.yml', relationship: 'depends-on', resolved: 'concept/gone.yml', exists: false },
    ],
  },
  {
    node: 'gauge/riffle-station.yml',
    class: 'gauge',
    links: [
      // Same raw text as the first link above, one directory over, meaning a different file.
      { target: '../concept/tailwater.yml', relationship: 'measured-by', resolved: 'concept/tailwater.yml', exists: true },
    ],
  },
  { node: 'concept/tailwater.yml', class: 'concept', links: [] },
]

const CLASSES = [
  { class: 'concept', label: 'Concept', properties: [{ name: 'datum', type: 'string' }] },
  { class: 'gauge', label: 'Gauge' },
]

test('a node is found by `node`, which is its identity', () => {
  assert.equal(nodeById(NODES, 'concept/low-flow.yml')?.class, 'concept')
  // `id` is the field name that was guessed once and was wrong. A miss is undefined rather
  // than a throw: the page renders a 404 for it, which is a real state.
  assert.equal(nodeById(NODES, 'concept/nope.yml'), undefined)
})

test('a class declaration is found for a node, and absence is not an error', () => {
  assert.equal(classOf(CLASSES, NODES[0])?.label, 'Concept')
  assert.equal(classOf(CLASSES, { node: 'x.yml' }), undefined)
  assert.equal(classOf([], NODES[0]), undefined)
})

test('an inbound edge is matched on the resolved path, not the raw target', () => {
  const inbound = referencesTo(NODES, 'concept/tailwater.yml')
  assert.deepEqual(inbound, [
    { from: 'concept/low-flow.yml', relationship: 'relates-to' },
    { from: 'gauge/riffle-station.yml', relationship: 'measured-by' },
  ])

  // The mutation this guards: matching `link.target` would report an inbound edge for a path
  // that is only ever a *relative* string. Nothing resolves to it, so nothing may claim it.
  assert.deepEqual(referencesTo(NODES, '../concept/tailwater.yml'), [])
})

test('a node nothing points at reports nothing, rather than reporting an absence', () => {
  assert.deepEqual(referencesTo(NODES, 'concept/low-flow.yml'), [])
  // A dangling edge's target is not a node, so it acquires no inbound edge by being pointed
  // at. `exists: false` is the binary's verdict and this inherits it rather than re-deciding.
  assert.deepEqual(referencesTo(NODES, 'concept/gone.yml'), [
    { from: 'concept/low-flow.yml', relationship: 'depends-on' },
  ])
})

test('a href keeps the separators and escapes the segments', () => {
  // `/` separates the parts of a node's identity; it is not a character to escape. Encoding
  // the whole string would produce one segment full of %2F that matches no node.
  assert.equal(nodeHref('concept/low-flow.yml'), '/node/concept/low-flow.yml')
  assert.equal(nodeHref('concept/a b.yml'), '/node/concept/a%20b.yml')
  assert.equal(nodeHref('concept/a#b.yml'), '/node/concept/a%23b.yml')
  // Round-trips through the decoding a rest route does, which is the only thing that matters.
  for (const id of ['concept/low-flow.yml', 'concept/a b.yml', 'concept/a#b.yml']) {
    const parts = nodeHref(id).slice('/node/'.length).split('/').map(decodeURIComponent)
    assert.equal(parts.join('/'), id)
  }
})
