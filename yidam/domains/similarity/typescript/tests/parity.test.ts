import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { cosine, jaccard, editDistance } from '../src/index.ts'

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

describe('parity: similarity.cosine', () => {
  const fixtures = loadFixtures('similarity.cosine')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(cosine(inp['a'], inp['b'])).toBe(exp['similarity'])
    })
  }
})

describe('parity: similarity.jaccard', () => {
  const fixtures = loadFixtures('similarity.jaccard')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string[]>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(jaccard(inp['a'], inp['b'])).toBe(exp['similarity'])
    })
  }
})

describe('parity: similarity.edit_distance', () => {
  const fixtures = loadFixtures('similarity.edit_distance')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(editDistance(inp['s1'], inp['s2'])).toBe(exp['distance'] as number)
    })
  }
})
