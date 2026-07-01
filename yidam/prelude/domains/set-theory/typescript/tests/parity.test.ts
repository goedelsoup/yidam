import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { union, intersection, difference, isSubset } from '../src/index.ts'

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

describe('parity: set_theory.union', () => {
  const fixtures = loadFixtures('set_theory.union')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number[]>
    it(fx['description'] as string, () => {
      expect(union(inp['a'], inp['b'])).toEqual(exp['elements'])
    })
  }
})

describe('parity: set_theory.intersection', () => {
  const fixtures = loadFixtures('set_theory.intersection')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number[]>
    it(fx['description'] as string, () => {
      expect(intersection(inp['a'], inp['b'])).toEqual(exp['elements'])
    })
  }
})

describe('parity: set_theory.difference', () => {
  const fixtures = loadFixtures('set_theory.difference')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number[]>
    it(fx['description'] as string, () => {
      expect(difference(inp['a'], inp['b'])).toEqual(exp['elements'])
    })
  }
})

describe('parity: set_theory.is_subset', () => {
  const fixtures = loadFixtures('set_theory.is_subset')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, boolean>
    it(fx['description'] as string, () => {
      expect(isSubset(inp['a'], inp['b'])).toBe(exp['result'])
    })
  }
})
