import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { entropy, klDivergence } from '../src/index.ts'

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

describe('parity: information_theory.entropy', () => {
  const fixtures = loadFixtures('information_theory.entropy')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(entropy(inp['probs'])).toBe(exp['entropy'])
    })
  }
})

describe('parity: information_theory.kl_divergence', () => {
  const fixtures = loadFixtures('information_theory.kl_divergence')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number[]>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(klDivergence(inp['p'], inp['q'])).toBe(exp['kl'])
    })
  }
})
