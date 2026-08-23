import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'

import { runReports } from '../src/report-run.ts'
import { spawn, type Spawn } from '../src/runner.ts'
import { contractBinary, SKIP, stageFixture } from './stage.ts'





test('lint owns the diagnostics; graph-check does not double-mark the same node', async (t) => {
  const dir = stageFixture('yidam-diag-')
  const bin = await contractBinary(dir)
  if (!bin) return t.skip(SKIP)

  const out = await runReports(bin, dir, { showBaselined: true }, spawn)
  assert.equal(out.ok, true)
  if (!out.ok) return

  const lowFlow = out.mapped.findings.filter((f) => f.file.endsWith('low-flow.yml'))
  const context = JSON.stringify(lowFlow, null, 2)

  // Both reports object to this node, and only lint's mark survives. Asserted by what
  // graph-check's mark would look like rather than by a count of the node's findings:
  // lint may legitimately report this node more than once — the ontology checks do — and
  // a count would read every new check as the regression this test exists to catch.
  assert.deepEqual(
    lowFlow.filter((f) => f.code === 'graph-check'),
    [],
    context,
  )

  const dangling = lowFlow.filter((f) => f.code === 'dangling-edge')
  assert.equal(dangling.length, 1, context)
  assert.equal(dangling[0].level, 'error')
  assert.ok(dangling[0].line > 1, 'lint carries a span; the redundant mark would sit at line 1')
})

test('the gate verdict comes from the CLI, not from counting findings', async (t) => {
  const dir = stageFixture('yidam-diag-')
  const bin = await contractBinary(dir)
  if (!bin) return t.skip(SKIP)
  const out = await runReports(bin, dir, { showBaselined: true }, spawn)
  assert.equal(out.ok === true && out.gatePassed, false)
})

test('after blessing, the same findings become Hints and the gate passes', async (t) => {
  const dir = stageFixture('yidam-diag-')
  const bin = await contractBinary(dir)
  if (!bin) return t.skip(SKIP)

  execFileSync(bin, ['lint', '--bless'], { cwd: dir, stdio: 'pipe' })
  const out = await runReports(bin, dir, { showBaselined: true }, spawn)
  assert.equal(out.ok, true)
  if (!out.ok) return

  assert.equal(out.gatePassed, true, 'blessed debt does not fail the gate')
  const errors = out.mapped.findings.filter((f) => f.level === 'error')
  assert.deepEqual(errors, [], 'nothing is an Error once it is inherited debt')
  const hints = out.mapped.findings.filter((f) => f.level === 'hint')
  assert.ok(hints.length > 0, 'the debt is still visible, faded')
  assert.ok(hints.every((h) => h.source === 'yidam (baseline)'))
})

test('showBaselined:false hides the debt without hiding a regression', async (t) => {
  const dir = stageFixture('yidam-diag-')
  const bin = await contractBinary(dir)
  if (!bin) return t.skip(SKIP)

  execFileSync(bin, ['lint', '--bless'], { cwd: dir, stdio: 'pipe' })
  // A brand-new broken edge, on top of a fully blessed corpus.
  fs.writeFileSync(
    path.join(dir, '.yidam/corpus/concept/probe.yml'),
    'class: concept\nlabel: Probe\ndescription: d\nlinks:\n  - target: ../concept/absent.yml\n',
  )
  const out = await runReports(bin, dir, { showBaselined: false }, spawn)
  assert.equal(out.ok, true)
  if (!out.ok) return

  assert.equal(out.mapped.findings.some((f) => f.baselined), false, 'debt hidden')
  const probe = out.mapped.findings.filter((f) => f.file.includes('probe.yml'))
  assert.ok(probe.some((f) => f.level === 'error'), 'the regression still shows')
})

test('a stale baseline entry surfaces as a condition, not a diagnostic', async (t) => {
  const dir = stageFixture('yidam-diag-')
  const bin = await contractBinary(dir)
  if (!bin) return t.skip(SKIP)

  execFileSync(bin, ['lint', '--bless'], { cwd: dir, stdio: 'pipe' })
  // Repair the deliberate defect: its baseline entry now lists something that no longer
  // occurs, which fails the gate and belongs to the repository rather than to a file.
  fs.writeFileSync(path.join(dir, '.yidam/corpus/concept/assimilative-capacit.yml'),
    'class: concept\nlabel: Assimilative capacity\ndescription: d\nlinks:\n  - target: ../concept/low-flow.yml\n')
  const out = await runReports(bin, dir, { showBaselined: true }, spawn)
  assert.equal(out.ok, true)
  if (!out.ok) return

  const stale = out.mapped.conditions.filter((c) => c.kind === 'stale-baseline')
  assert.ok(stale.length > 0, JSON.stringify(out.mapped.conditions, null, 2))
  assert.equal(out.gatePassed, false)
  assert.match(stale[0].message, /--bless/)
})

test('contract skew is reported rather than parsed through', async () => {
  // No binary needed: a stub that behaves like a stale yidam.
  const stale: Spawn = async () => ({
    stdout: '',
    stderr: "error: unexpected argument '--format' found\n",
    code: 2,
  })
  const out = await runReports('yidam', '/tmp', { showBaselined: true }, stale)
  assert.equal(out.ok, false)
  assert.equal(out.ok === false && out.handshake.ok, false)
})
