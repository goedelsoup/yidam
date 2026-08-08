import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { parseNode, extractClaims, extractLinks } from '../src/corpus.ts'
import { classifyCommit, isRecognizedVerb } from '../src/git.ts'
import { parseMarkers, updateRegen } from '../src/markers.ts'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

const FIXTURES_DIR = join(__dirname, '../../parity/fixtures')

function loadFixtures(fn: string): Record<string, unknown>[] {
  const dir = join(FIXTURES_DIR, fn)
  if (!existsSync(dir)) return []
  return readdirSync(dir)
    .filter(f => f.endsWith('.toml'))
    .sort()
    .map(f => parse(readFileSync(join(dir, f), 'utf8')) as Record<string, unknown>)
}

// ── parse_node ────────────────────────────────────────────────────────────────

describe('parity: parse_node', () => {
  const fixtures = loadFixtures('parse_node')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const exp = fx['expected'] as Record<string, unknown>
    it(inp['path'], () => {
      const node = parseNode(inp['path'], inp['content'])
      expect(node.path).toBe(exp['path'])
      expect(node.title).toBe(exp['title'])
      expect(node.kind).toBe(exp['kind'])

      const expClaims = (exp['claims'] as Record<string, string>[] | undefined) ?? []
      expect(node.claims).toHaveLength(expClaims.length)
      for (let i = 0; i < expClaims.length; i++) {
        expect(node.claims[i].text).toBe(expClaims[i]['text'])
        expect(node.claims[i].tag).toBe(expClaims[i]['tag'])
      }

      const expLinks = (exp['links'] as Record<string, string>[] | undefined) ?? []
      expect(node.links).toHaveLength(expLinks.length)
      for (let i = 0; i < expLinks.length; i++) {
        expect(node.links[i].label).toBe(expLinks[i]['label'])
        expect(node.links[i].target).toBe(expLinks[i]['target'])
        expect(node.links[i].anchor).toBe(expLinks[i]['anchor'])
      }
    })
  }
})

// ── extract_claims ────────────────────────────────────────────────────────────

describe('parity: extract_claims', () => {
  const fixtures = loadFixtures('extract_claims')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const expected = fx['expected'] as Record<string, string>[]
    it(fx['description'] as string, () => {
      const claims = extractClaims(inp['content'])
      expect(claims).toHaveLength(expected.length)
      for (let i = 0; i < expected.length; i++) {
        expect(claims[i].text).toBe(expected[i]['text'])
        expect(claims[i].tag).toBe(expected[i]['tag'])
      }
    })
  }
})

// ── extract_links ─────────────────────────────────────────────────────────────

describe('parity: extract_links', () => {
  const fixtures = loadFixtures('extract_links')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const expected = fx['expected'] as Record<string, string>[]
    it(fx['description'] as string, () => {
      const links = extractLinks(inp['content'])
      expect(links).toHaveLength(expected.length)
      for (let i = 0; i < expected.length; i++) {
        expect(links[i].label).toBe(expected[i]['label'])
        expect(links[i].target).toBe(expected[i]['target'])
        expect(links[i].anchor).toBe(expected[i]['anchor'])
      }
    })
  }
})

// ── classify_commit ───────────────────────────────────────────────────────────

describe('parity: classify_commit', () => {
  const fixtures = loadFixtures('classify_commit')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const exp = fx['expected'] as Record<string, string>
    it(fx['description'] as string, () => {
      const event = classifyCommit(inp['hash'], inp['message'])
      expect(event.kind).toBe(exp['kind'])
      expect(event.verb).toBe(exp['verb'])
      expect(event.subject).toBe(exp['subject'])
    })
  }
})

// ── is_recognized_verb ────────────────────────────────────────────────────────

describe('parity: is_recognized_verb', () => {
  const fixtures = loadFixtures('is_recognized_verb')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const exp = fx['expected'] as Record<string, boolean>
    it(fx['description'] as string, () => {
      expect(isRecognizedVerb(inp['verb'])).toBe(exp['recognized'])
    })
  }
})

// ── parse_markers ─────────────────────────────────────────────────────────────

describe('parity: parse_markers', () => {
  const fixtures = loadFixtures('parse_markers')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const expected = fx['expected'] as Record<string, string>[]
    it(fx['description'] as string, () => {
      const parsed = parseMarkers(inp['content'])
      expect(parsed).toHaveLength(expected.length)
      for (let i = 0; i < expected.length; i++) {
        expect(parsed[i].kind).toBe(expected[i]['kind'])
        if (parsed[i].kind === 'Template') {
          expect((parsed[i] as { kind: 'Template'; instruction: string }).instruction)
            .toBe(expected[i]['instruction'])
        } else {
          const m = parsed[i] as { kind: 'Regen'; command: string; content: string }
          expect(m.command).toBe(expected[i]['command'])
          expect(m.content).toBe(expected[i]['content'])
        }
      }
    })
  }
})

// ── update_regen ──────────────────────────────────────────────────────────────

describe('parity: update_regen', () => {
  const fixtures = loadFixtures('update_regen')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const exp = fx['expected'] as Record<string, string>
    it(fx['description'] as string, () => {
      const result = updateRegen(inp['content'], inp['command'], inp['new_content'])
      expect(result).toBe(exp['content'])
    })
  }
})
