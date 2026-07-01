import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { mean, variance, zScore, pearsonCorrelation } from '../src/index.ts'

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

describe('parity: statistics.mean', () => {
  const fixtures = loadFixtures('statistics.mean')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(mean(inp['values'])).toBe(exp['mean'])
    })
  }
})

describe('parity: statistics.variance', () => {
  const fixtures = loadFixtures('statistics.variance')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(variance(inp['values'])).toBe(exp['variance'])
    })
  }
})

describe('parity: statistics.z_score', () => {
  const fixtures = loadFixtures('statistics.z_score')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(zScore(inp['value'], inp['mean'], inp['std_dev'])).toBe(exp['z_score'])
    })
  }
})

describe('parity: statistics.pearson_correlation', () => {
  const fixtures = loadFixtures('statistics.pearson_correlation')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(pearsonCorrelation(inp['xs'], inp['ys'])).toBe(exp['r'])
    })
  }
})
