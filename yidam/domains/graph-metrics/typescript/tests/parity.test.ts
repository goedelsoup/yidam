import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { density, degreeCentrality } from '../src/index.ts'

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

describe('parity: graph_metrics.density', () => {
  const fixtures = loadFixtures('graph_metrics.density')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(density(inp['node_count'], inp['edge_count'])).toBe(exp['density'])
    })
  }
})

describe('parity: graph_metrics.degree_centrality', () => {
  const fixtures = loadFixtures('graph_metrics.degree_centrality')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, unknown>
    const exp = fx['expected'] as Record<string, number[]>
    it(fx['description'] as string, () => {
      const result = degreeCentrality(inp['degrees'] as number[], inp['node_count'] as number)
      expect(result).toEqual(exp['centrality'])
    })
  }
})
