// TypeScript runner for the embedding reproducibility contract
// (parity/fixtures/embed_config/). Mirrors the Rust reference runner in
// yidam/cli/tests/embed_parity.rs.
//
// Downloads model weights on first run, so it only executes when
// YIDAM_EMBED_PARITY=1 is set (see the `embed-parity` mise task).
//
// The dtype must match the contract's model_file: quantized (q8) and fp32
// exports of the same model drift ~1e-3 per element, far outside the
// contract tolerance.

import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

const FIXTURES_DIR = join(__dirname, '../../parity/fixtures/embed_config')

interface EmbedFixture {
  input: {
    text: string
    model_id: string
    model_file: string
    pooling: string
    normalize: boolean
  }
  expected: { embedding_dim: number; tolerance: number; prefix: number[] }
}

function loadFixtures(): [string, EmbedFixture][] {
  if (!existsSync(FIXTURES_DIR)) return []
  return readdirSync(FIXTURES_DIR)
    .filter(f => f.endsWith('.toml'))
    .sort()
    .map(f => [f, parse(readFileSync(join(FIXTURES_DIR, f), 'utf8')) as unknown as EmbedFixture])
}

const enabled = process.env['YIDAM_EMBED_PARITY'] === '1'

describe.skipIf(!enabled)('parity: embed_config', () => {
  const fixtures = loadFixtures()
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const [name, fx] of fixtures) {
    it(name, async () => {
      const { pipeline } = await import('@huggingface/transformers')
      const dtype = fx.input.model_file.includes('quantized') ? 'q8' : 'fp32'
      const extractor = await pipeline('feature-extraction', fx.input.model_id, { dtype })
      const output = await extractor(fx.input.text, {
        pooling: fx.input.pooling as 'mean',
        normalize: fx.input.normalize,
      })
      const vector = output.data as Float32Array

      expect(vector.length).toBe(fx.expected.embedding_dim)

      let norm = 0
      for (const x of vector) norm += x * x
      expect(Math.abs(Math.sqrt(norm) - 1)).toBeLessThan(1e-3)

      const actualPrefix = Array.from(vector.slice(0, fx.expected.prefix.length))
      for (let i = 0; i < fx.expected.prefix.length; i++) {
        expect(
          Math.abs(actualPrefix[i] - fx.expected.prefix[i]),
          `prefix[${i}] = ${actualPrefix[i]}, expected ${fx.expected.prefix[i]}\n` +
            `full actual prefix: [${actualPrefix.join(', ')}]`,
        ).toBeLessThanOrEqual(fx.expected.tolerance)
      }
    }, 300_000)
  }
})
