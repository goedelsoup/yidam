/**
 * `sourceClasses` — the derivation `orphan-in` exempts on.
 *
 * Held here rather than as a parity fixture: it is not on the parity function list, so
 * `parity-check` neither requires nor runs one. That is a gap worth closing — three
 * transcriptions of one subtle rule pinned only by themselves is the failure the parity
 * loop exists to prevent, one function over — but promoting it is a parity-surface change
 * and belongs in its own PR.
 */
import { describe, expect, it } from 'vitest'

import { sourceClasses, type OntologyClass } from '../src/ontology.ts'

function cls(name: string, edges: [string, string][]): OntologyClass {
  return {
    name,
    label: '',
    description: '',
    properties: [],
    edges: edges.map(([target, direction]) => ({
      relationship: 'r',
      target,
      direction,
      description: '',
    })),
  }
}

describe('sourceClasses', () => {
  it('does not exempt a class another class points at', () => {
    const sources = sourceClasses([cls('gage', [['concept', 'out']]), cls('concept', [['concept', 'out']])])
    expect(sources.has('concept')).toBe(false)
    expect(sources.has('gage')).toBe(true)
  })

  it('does not treat a self-edge as being pointed at', () => {
    expect(sourceClasses([cls('reach', [['reach', 'out']])])).toEqual(new Set(['reach']))
  })

  it('does not exempt a class that declared no edges', () => {
    expect(sourceClasses([cls('quiet', [])])).toEqual(new Set())
  })

  it('exempts neither end of a directionless declaration', () => {
    expect(sourceClasses([cls('a', [['b', '']]), cls('b', [['c', 'out']])])).toEqual(new Set())
  })
})
