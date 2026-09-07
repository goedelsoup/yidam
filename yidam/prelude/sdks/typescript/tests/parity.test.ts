import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'smol-toml'
import { describe, it, expect } from 'vitest'
import { parseInstance, instanceToJson } from '../src/corpus.ts'
import { classifyCommit, isRecognizedVerb } from '../src/git.ts'
import { findReachable, findCitations, type GraphEdge } from '../src/graph.ts'
import { scanMarkers, updateRegen } from '../src/markers.ts'
import { parseClass, compileClassSchema } from '../src/ontology.ts'

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

// ── parse_instance ────────────────────────────────────────────────────────────
//
// Compared as parsed JSON, for `compile_class_schema`'s reason: key order is not part of the
// contract and three languages will not agree on it.
//
// Non-string scalar resolution is out of contract and the fixtures avoid it — `010` is the
// string "010" to serde_yaml, the number 10 here, and 8 to PyYAML. The one such divergence a
// corpus actually hits is the timestamp, and it IS in contract:
// `unquoted-dates-stay-text.toml` fails if this file stops passing `version: '1.2'` or the
// Python SDK puts its timestamp resolver back.

describe('parity: parse_instance', () => {
  const fixtures = loadFixtures('parse_instance')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const exp = fx['expected'] as Record<string, string>
    it(fx['description'] as string, () => {
      const got = instanceToJson(parseInstance(inp['content']))
      expect(got).toEqual(JSON.parse(exp['instance']))
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
      const scan = scanMarkers(inp['content'])
      const parsed = scan.markers
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

      // Absent means none, not "not checked". A fixture written before this field existed
      // is asserting that its input has no malformed block.
      const bad = (fx['expected_malformed'] as Record<string, unknown>[] | undefined) ?? []
      expect(scan.malformed).toHaveLength(bad.length)
      for (let i = 0; i < bad.length; i++) {
        expect(scan.malformed[i].command).toBe(bad[i]['command'])
        expect(scan.malformed[i].line).toBe(bad[i]['line'])
        expect(scan.malformed[i].fault).toBe(bad[i]['fault'])
        expect(scan.malformed[i].swallowedLines).toBe(bad[i]['swallowed_lines'])
        expect(scan.malformed[i].swallowedMarkers).toBe(bad[i]['swallowed_markers'])
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

// ── compile_class_schema ──────────────────────────────────────────────────────

describe('parity: compile_class_schema', () => {
  const fixtures = loadFixtures('compile_class_schema')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, string>
    const exp = fx['expected'] as Record<string, string>
    it(fx['description'] as string, () => {
      const got = compileClassSchema(parseClass(inp['name'], inp['content']))
      // Compared as parsed JSON, not as text: key order and whitespace are not part of the
      // contract, and three languages will not agree on either.
      expect(got).toEqual(JSON.parse(exp['schema']))
    })
  }
})

// ── find_reachable ────────────────────────────────────────────────────────────

function edgesOf(inp: Record<string, unknown>): GraphEdge[] {
  return (inp['edges'] as Record<string, string>[]).map(e => ({ from: e['from'], to: e['to'] }))
}

describe('parity: find_reachable', () => {
  const fixtures = loadFixtures('find_reachable')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, unknown>
    const exp = fx['expected'] as Record<string, string[]>
    it(fx['description'] as string, () => {
      expect(findReachable(edgesOf(inp), inp['node_path'] as string)).toEqual(exp['reachable'])
    })
  }
})

// ── find_citations ────────────────────────────────────────────────────────────

describe('parity: find_citations', () => {
  const fixtures = loadFixtures('find_citations')
  it('has fixtures', () => expect(fixtures.length).toBeGreaterThan(0))

  for (const fx of fixtures) {
    const inp = fx['input'] as Record<string, unknown>
    const exp = fx['expected'] as Record<string, string[]>
    it(fx['description'] as string, () => {
      expect(findCitations(edgesOf(inp), inp['node_path'] as string)).toEqual(exp['citations'])
    })
  }
})
