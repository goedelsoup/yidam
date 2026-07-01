import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { rationalProduct, manningVelocity, returnPeriod } from '../src/index.ts'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const FIXTURES_DIR = join(__dirname, '../../../parity/fixtures')
const EPSILON = 1e-9

function loadFixtures(fn: string): Record<string, unknown>[] {
  const dir = join(FIXTURES_DIR, fn)
  if (!existsSync(dir)) return []
  return readdirSync(dir)
    .filter(f => f.endsWith('.toml'))
    .sort()
    .map(f => parse(readFileSync(join(dir, f), 'utf8')) as Record<string, unknown>)
}

describe('parity: hydrology.rational_product', () => {
  const fixtures = loadFixtures('hydrology.rational_product')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(rationalProduct(inp['c'], inp['i'], inp['a']) - exp['result'])).toBeLessThan(EPSILON)
    })
  }
})

describe('parity: hydrology.manning_velocity', () => {
  const fixtures = loadFixtures('hydrology.manning_velocity')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(manningVelocity(inp['n'], inp['r'], inp['s']) - exp['velocity'])).toBeLessThan(EPSILON)
    })
  }
})

describe('parity: hydrology.return_period', () => {
  const fixtures = loadFixtures('hydrology.return_period')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(returnPeriod(inp['record_years'], inp['rank']) - exp['years'])).toBeLessThan(EPSILON)
    })
  }
})
