import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { presentValue, futureValue, simpleInterest, sharpeRatio } from '../src/index.ts'

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

describe('parity: finance.present_value', () => {
  const fixtures = loadFixtures('finance.present_value')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(presentValue(inp['fv'], inp['rate'], inp['periods']) - exp['pv'])).toBeLessThan(EPSILON)
    })
  }
})

describe('parity: finance.future_value', () => {
  const fixtures = loadFixtures('finance.future_value')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(futureValue(inp['pv'], inp['rate'], inp['periods']) - exp['fv'])).toBeLessThan(EPSILON)
    })
  }
})

describe('parity: finance.simple_interest', () => {
  const fixtures = loadFixtures('finance.simple_interest')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(simpleInterest(inp['principal'], inp['rate'], inp['time']) - exp['interest'])).toBeLessThan(EPSILON)
    })
  }
})

describe('parity: finance.sharpe_ratio', () => {
  const fixtures = loadFixtures('finance.sharpe_ratio')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(sharpeRatio(inp['ret'], inp['risk_free'], inp['std_dev']) - exp['ratio'])).toBeLessThan(EPSILON)
    })
  }
})
