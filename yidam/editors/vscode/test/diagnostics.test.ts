import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'

import { parse } from 'smol-toml'

import { DEFAULT_OPTIONS, fromGraphCheck, fromLint, type Level } from '../src/diagnostics.ts'
import type { Check, GraphCheckReport, LintReport } from '../src/reports.ts'

const envelope = {
  format_version: '1',
  yidam: { version: '0.1.0', commit: 'abc1234', features: ['reports'] },
  root: '/r',
}

function lint(checks: Check[], stale: { check: string; node: string }[] = []): LintReport {
  return {
    ...envelope,
    gate: {
      passed: stale.length === 0,
      new_violations: 0,
      baselined_violations: 0,
      stale_baseline_entries: stale,
    },
    checks,
  }
}

const check = (
  id: string,
  severity: Check['severity'],
  violations: Check['violations'],
): Check => ({ id, title: 'T', severity, rationale: `why ${id} exists`, violations })

const levels = (r: { findings: { level: Level }[] }) => r.findings.map((f) => f.level)

// ── The mapping, against the fixture the CLI is pinned to ────────────────────
//
// `severity_of` in `yidam/cli/src/cmd/lsp.rs` and `levelFor` here are two transcriptions of
// one four-row table. The duplication is deliberate and RFC-0016 licenses it — the
// alternative is an editor that cannot render a diagnostic without a subprocess per
// keystroke — but until these fixtures existed each side was pinned only by its own
// hand-written expectations, so the two could be independently right about different tables.
//
// The fixtures carry a level *name*, not a number, because neither side's numbering is
// shared: LSP counts from 1 and `vscode.DiagnosticSeverity` counts from 0. `Level` is
// already those four names, so the mapping here is identity and the assertion is direct.

const HERE = path.dirname(new URL(import.meta.url).pathname)
const SEVERITY_FIXTURES = path.resolve(
  HERE,
  '../../../prelude/sdks/parity/fixtures/diagnostic_severity',
)

interface SeverityCase {
  description: string
  input: { severity: Check['severity']; in_baseline: boolean }
  expected: { level: Level }
}

function severityCases(): { name: string; fx: SeverityCase }[] {
  return fs
    .readdirSync(SEVERITY_FIXTURES)
    .filter((f) => f.endsWith('.toml'))
    .sort()
    .map((name) => ({
      name,
      fx: parse(fs.readFileSync(path.join(SEVERITY_FIXTURES, name), 'utf8')) as unknown as SeverityCase,
    }))
}

test('the severity table is the shared fixture, not a second opinion about it', () => {
  const cases = severityCases()
  assert.ok(cases.length > 0, `no fixtures in ${SEVERITY_FIXTURES}`)
  for (const { name, fx } of cases) {
    const r = fromLint(
      lint([
        check('dangling-edge', fx.input.severity, [
          { node: 'a.yml', detail: 'd', in_baseline: fx.input.in_baseline },
        ]),
      ]),
    )
    assert.deepEqual(levels(r), [fx.expected.level], `${name}: ${fx.description}`)
  }
})

// ── The mapping, which is the whole issue ────────────────────────────────────

test('a new error is an Error — this is what fails CI', () => {
  const r = fromLint(lint([check('dangling-edge', 'error', [{ node: 'a.yml', detail: 'd', in_baseline: false }])]))
  assert.deepEqual(levels(r), ['error'])
  assert.equal(r.findings[0].source, 'yidam')
})

test('a BASELINED error is a Hint, not an Error', () => {
  // The failure this issue exists to prevent: rendering inherited debt as a regression
  // fills the Problems panel with things no commit caused, and the panel stops being read.
  const r = fromLint(lint([check('dangling-edge', 'error', [{ node: 'a.yml', detail: 'd', in_baseline: true }])]))
  assert.deepEqual(levels(r), ['hint'])
  assert.equal(r.findings[0].source, 'yidam (baseline)')
  assert.equal(r.findings[0].baselined, true)
})

test('baseline membership outranks check severity in both directions', () => {
  const r = fromLint(
    lint([
      check('a', 'error', [{ node: '1.yml', detail: 'd', in_baseline: false }]),
      check('b', 'error', [{ node: '2.yml', detail: 'd', in_baseline: true }]),
      check('c', 'warn', [{ node: '3.yml', detail: 'd', in_baseline: false }]),
      check('d', 'info', [{ node: '4.yml', detail: 'd', in_baseline: false }]),
    ]),
  )
  assert.deepEqual(levels(r), ['error', 'hint', 'warning', 'information'])
})

test('severity is per violation, not per check', () => {
  // One check, two violations, different baseline membership — the case a per-check
  // mapping gets wrong and the reason `in_baseline` is on the violation.
  const r = fromLint(
    lint([
      check('dangling-edge', 'error', [
        { node: 'old.yml', detail: 'd', in_baseline: true },
        { node: 'new.yml', detail: 'd', in_baseline: false },
      ]),
    ]),
  )
  assert.deepEqual(levels(r), ['hint', 'error'])
})

// ── Stale baseline entries ───────────────────────────────────────────────────

test('a stale baseline entry is a repo condition, never a diagnostic', () => {
  // Its problem is that the file NO LONGER has a problem. A squiggle would point at
  // nothing, on a line that is now correct.
  const r = fromLint(lint([], [{ check: 'orphan-in', node: 'gone.yml' }]))
  assert.deepEqual(r.findings, [])
  assert.equal(r.conditions.length, 1)
  assert.equal(r.conditions[0].kind, 'stale-baseline')
  assert.match(r.conditions[0].message, /--bless/)
})

// ── showBaselined ────────────────────────────────────────────────────────────

test('showBaselined:false drops baselined findings and keeps the rest', () => {
  const r = fromLint(
    lint([
      check('a', 'error', [
        { node: '1.yml', detail: 'd', in_baseline: true },
        { node: '2.yml', detail: 'd', in_baseline: false },
      ]),
    ]),
    { showBaselined: false },
  )
  assert.deepEqual(levels(r), ['error'])
})

test('showBaselined defaults to true', () => {
  assert.equal(DEFAULT_OPTIONS.showBaselined, true)
})

// ── Location ─────────────────────────────────────────────────────────────────

test('a span places the finding on the offending line', () => {
  const r = fromLint(lint([check('dangling-edge', 'error', [{ node: 'a.yml', detail: 'd', in_baseline: false, span: { line: 5 } }])]))
  assert.equal(r.findings[0].line, 5)
  assert.equal(r.findings[0].file, 'a.yml')
})

test('no span anchors at line 1 rather than guessing', () => {
  const r = fromLint(lint([check('orphan-in', 'info', [{ node: 'a.yml', detail: 'd', in_baseline: false }])]))
  assert.equal(r.findings[0].line, 1)
})

test('a path:line identity is split, and beats an absent span', () => {
  // The prose-link and annotation checks encode the line in the identity itself.
  const r = fromLint(lint([check('broken-prose-link', 'error', [{ node: 'docs/x.md:14', detail: 'd', in_baseline: false }])]))
  assert.equal(r.findings[0].file, 'docs/x.md')
  assert.equal(r.findings[0].line, 14)
})

test('a windows-ish path with no line is not mistaken for path:line', () => {
  const r = fromLint(lint([check('a', 'error', [{ node: 'a.yml', detail: 'd', in_baseline: false }])]))
  assert.equal(r.findings[0].file, 'a.yml')
})

// ── Hover and code ───────────────────────────────────────────────────────────

test('every finding carries its check id and rationale', () => {
  const r = fromLint(lint([check('class-asserts-purpose', 'warn', [{ node: 'a.yml', detail: 'd', in_baseline: false }])]))
  assert.equal(r.findings[0].code, 'class-asserts-purpose')
  assert.match(r.findings[0].rationale, /why class-asserts-purpose exists/)
})

// ── graph-check ──────────────────────────────────────────────────────────────

const graph = (over: Partial<GraphCheckReport> = {}): GraphCheckReport => ({
  ...envelope,
  passed: true,
  corpus_empty: false,
  total_instances: 2,
  clean_instances: 2,
  classes_defined: 1,
  nodes_with_issues: [],
  classes_without_instances: [],
  ...over,
})

test('graph-check does not duplicate a node lint already reported', () => {
  // Measured on the reports fixture: graph-check reports the same broken edge as
  // `dangling-edge`, but with no span — so rendering both puts one accurate mark on the
  // offending line and a redundant one at line 1.
  const g = graph({
    passed: false,
    clean_instances: 1,
    nodes_with_issues: [{ node: 'low-flow.yml', issues: ['broken link: ../gone.yml'] }],
  })
  const r = fromGraphCheck(g, new Set(['low-flow.yml']))
  assert.deepEqual(r.findings, [])
  // The verdict still surfaces — it is a gate CI runs.
  assert.equal(r.conditions.length, 1)
  assert.equal(r.conditions[0].kind, 'graph-gate')
})

test('graph-check still reports a node lint did not mention', () => {
  const g = graph({
    passed: false,
    nodes_with_issues: [{ node: 'orphan.yml', issues: ["missing 'class:' field"] }],
  })
  const r = fromGraphCheck(g, new Set())
  assert.equal(r.findings.length, 1)
  assert.equal(r.findings[0].level, 'error')
  assert.equal(r.findings[0].line, 1)
  assert.equal(r.findings[0].code, 'graph-check')
})

test('a passing graph gate contributes no condition', () => {
  assert.deepEqual(fromGraphCheck(graph(), new Set()).conditions, [])
})

test('graph-check findings are never baselined — it has no baseline', () => {
  const g = graph({ passed: false, nodes_with_issues: [{ node: 'x.yml', issues: ['i'] }] })
  assert.equal(fromGraphCheck(g, new Set()).findings[0].baselined, false)
})
