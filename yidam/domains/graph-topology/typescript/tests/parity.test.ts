import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { clusteringCoefficient, connectedComponents } from '../src/index.ts'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const FIXTURES_DIR = join(__dirname, '../../../parity/fixtures')

function loadFixtures(fn: string): Record<string, unknown>[] {
  const dir = join(FIXTURES_DIR, fn)
  if (!existsSync(dir)) return []
  return readdirSync(dir)
    .filter(f => f.endsWith('.toml'))
    .sort()
    .map(f => parse(readFileSync(join(dir, f), 'utf8')) as Record<string, unknown>)
}

describe('parity: graph_topology.clustering_coefficient', () => {
  const fixtures = loadFixtures('graph_topology.clustering_coefficient')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(clusteringCoefficient(inp['degree'], inp['triangle_count'])).toBe(exp['coefficient'])
    })
  }
})

describe('parity: graph_topology.connected_components', () => {
  const fixtures = loadFixtures('graph_topology.connected_components')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, unknown>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      const nodeCount = inp['node_count'] as number
      const edges = (inp['edges'] as number[][] ?? []).map(e => [e[0], e[1]] as [number, number])
      expect(connectedComponents(nodeCount, edges)).toBe(exp['components'])
    })
  }
})
