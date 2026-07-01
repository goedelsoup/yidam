import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { haversineKm, bearingDeg, centralAngleDeg } from '../src/index.ts'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const FIXTURES_DIR = join(__dirname, '../../../parity/fixtures')
const EPSILON = 1e-4

function loadFixtures(fn: string): Record<string, unknown>[] {
  const dir = join(FIXTURES_DIR, fn)
  if (!existsSync(dir)) return []
  return readdirSync(dir)
    .filter(f => f.endsWith('.toml'))
    .sort()
    .map(f => parse(readFileSync(join(dir, f), 'utf8')) as Record<string, unknown>)
}

describe('parity: geodesics.haversine_km', () => {
  const fixtures = loadFixtures('geodesics.haversine_km')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(haversineKm(inp['lat1'], inp['lon1'], inp['lat2'], inp['lon2']) - exp['km'])).toBeLessThan(EPSILON)
    })
  }
})

describe('parity: geodesics.bearing_deg', () => {
  const fixtures = loadFixtures('geodesics.bearing_deg')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(bearingDeg(inp['lat1'], inp['lon1'], inp['lat2'], inp['lon2']) - exp['degrees'])).toBeLessThan(EPSILON)
    })
  }
})

describe('parity: geodesics.central_angle_deg', () => {
  const fixtures = loadFixtures('geodesics.central_angle_deg')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))
  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, number>
    const exp = fx['expected'] as Record<string, number>
    it(fx['description'] as string, () => {
      expect(Math.abs(centralAngleDeg(inp['lat1'], inp['lon1'], inp['lat2'], inp['lon2']) - exp['degrees'])).toBeLessThan(EPSILON)
    })
  }
})
