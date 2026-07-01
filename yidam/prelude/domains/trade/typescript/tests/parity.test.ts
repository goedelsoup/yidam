import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { tradeBalance, termsOfTrade, tariffRevenue, revealedComparativeAdvantage } from '../src/index.ts'

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

describe('parity: trade.trade_balance', () => {
  const fixtures = loadFixtures('trade.trade_balance')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(tradeBalance(inp['exports'], inp['imports'])).toBe(exp['balance'])
    })
  }
})

describe('parity: trade.terms_of_trade', () => {
  const fixtures = loadFixtures('trade.terms_of_trade')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(termsOfTrade(inp['export_index'], inp['import_index'])).toBe(exp['tot'])
    })
  }
})

describe('parity: trade.tariff_revenue', () => {
  const fixtures = loadFixtures('trade.tariff_revenue')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(tariffRevenue(inp['import_value'], inp['rate'])).toBe(exp['revenue'])
    })
  }
})

describe('parity: trade.revealed_comparative_advantage', () => {
  const fixtures = loadFixtures('trade.revealed_comparative_advantage')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(revealedComparativeAdvantage(inp['country_share'], inp['world_share'])).toBe(exp['rca'])
    })
  }
})
