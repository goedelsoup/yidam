/**
 * The extension's reader, driven against the corpora that certify the reports.
 *
 * RFC-0016 asks for this explicitly: the `reports/` golden trees become the repositories
 * the extension is exercised on, so a fixture whose output changes fails the parity run
 * *and* these tests. Two things then cannot drift apart quietly — the contract the CLI
 * emits and the contract this extension believes it emits.
 *
 * No editor is involved. This is the reader, the real binary, and a real corpus.
 */

import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'

import { fromLint } from '../src/diagnostics.ts'
import { hasFeature, readHandshake } from '../src/handshake.ts'
import type { LintReport } from '../src/reports.ts'
import { healthTree } from '../src/tree/model.ts'
import { captureStreams, contractBinary, FIXTURE_DIR, SKIP, stageFixture } from './stage.ts'








test('the handshake accepts what a real yidam actually emits', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    // Skipped rather than failed: a contributor without the CLI on PATH should still be
    // able to run the unit tests. CI installs it, so CI does not take this branch.
    t.skip(SKIP)
    return
  }

  const { stdout, stderr } = captureStreams(bin, ['status', '--format', 'json'], dir)
  const h = readHandshake(stdout, stderr)
  assert.equal(h.ok, true, `handshake failed: ${h.ok === false ? h.message : ''}`)
  assert.equal(hasFeature(h, 'reports'), true, 'the light build always reports `reports`')
})

test('a binary that rejects --format reads as contract skew, not a broken install', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // Simulated rather than requiring a stale binary on hand — but this is the real shape,
  // found by running these tests against a yidam that predated the flag: clap prints a
  // usage message to stderr, writes nothing to stdout, and exits nonzero.
  const h = readHandshake('', "error: unexpected argument '--format' found\n\nUsage: yidam status\n")
  assert.equal(h.ok, false)
  assert.equal(h.ok === false && h.kind, 'not-json')
  assert.match(h.ok === false ? h.message : '', /predates `--format json`/)
})

test('prose from the same binary degrades to a named state', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // Without --format json the very same command prints prose. This is what a binary
  // predating the contract looks like from the extension's side, and it must be a state
  // rather than a stack trace.
  const { stdout: prose } = captureStreams(bin, ['status'], dir)
  const h = readHandshake(prose)
  assert.equal(h.ok, false)
  assert.equal(h.ok === false && h.kind, 'not-json')
})

test('a gating command still yields a readable envelope', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // `lint` exits nonzero on this fixture by design — it carries one deliberate broken
  // edge. The envelope is on stdout regardless, and the extension must read it rather
  // than treating a failing gate as an unusable binary.
  const { stdout, stderr } = captureStreams(bin, ['lint', '--format', 'json'], dir)
  const h = readHandshake(stdout, stderr)
  assert.equal(h.ok, true)
  const doc = JSON.parse(stdout)
  assert.equal(doc.gate.passed, false, 'the fixture carries a deliberate error')
  assert.equal(typeof doc.gate.new_violations, 'number')
})

test('a finding that fails CI reaches the panel as an Error, whatever its check is declared at', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // The fixture's `missing-property` block is the case: the check is declared `warn` and
  // one of its three findings is `error`, because `concept` declares `datum` as
  // `required: true` and `low-flow.yml` omits it. That finding fails CI. While this reader
  // took the level from `check.severity` it rendered as a Warning — the whole of #655, and
  // invisible until the fixture carried a violation whose severity was not its check's.
  //
  // Driven through `fromLint` rather than by reading the JSON: the assertion is about what
  // the Problems panel shows, and the mapping is the thing that was wrong.
  const { stdout } = captureStreams(bin, ['lint', '--format', 'json'], dir)
  const report = JSON.parse(stdout) as LintReport

  const missing = report.checks.find((c) => c.id === 'missing-property')
  assert.ok(missing, 'the fixture trips missing-property')
  assert.equal(missing.severity, 'warn', 'the check is declared warn — that is the premise')

  const findings = fromLint(report).findings.filter((f) => f.code === 'missing-property')
  const required = findings.filter((f) => /required: true/.test(f.message))
  assert.equal(required.length, 1, JSON.stringify(findings, null, 2))
  assert.equal(required[0].level, 'error', JSON.stringify(required[0], null, 2))

  // And the check's other findings are still Warnings. Without this, a mapping that read
  // `violation.severity` and one that ignored severity entirely and always said `error`
  // would both pass.
  const rest = findings.filter((f) => !/required: true/.test(f.message))
  assert.ok(rest.length > 0, JSON.stringify(findings, null, 2))
  assert.deepEqual([...new Set(rest.map((f) => f.level))], ['warning'])
})

test('an expired baseline entry reaches the reader with its cause stated', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // The fixture's baseline lists two violations and declares `expire_after: 2`; one entry
  // carries a `since` three corpus-touching commits back and one carries none at all. So the
  // gate fails with an expired entry rather than an introduced one — the state that reached
  // the Health view as a red row with no children and no stated cause, for as long as this
  // reader's report type omitted the field (#657).
  const { stdout } = captureStreams(bin, ['lint', '--format', 'json'], dir)
  const report = JSON.parse(stdout) as LintReport

  // Against the committed golden rather than a transcription of it. RFC-0016 asks for
  // exactly this — the `reports/` goldens become the repository the extension is exercised
  // on — and it is also what makes a stale binary say so: `contractBinary`'s probe certifies
  // `status` alone, so a yidam predating a *lint check* resolves, passes the probe, and then
  // disagrees here (#752). This names that rather than failing on an arithmetic mismatch,
  // which is what it did when 0.9.0 was on PATH.
  const golden = JSON.parse(
    fs.readFileSync(path.join(FIXTURE_DIR, 'expected/lint.json'), 'utf8'),
  ) as LintReport
  assert.deepEqual(
    report.gate,
    golden.gate,
    'this yidam does not reproduce the fixture\'s committed lint gate — it is stale relative ' +
      'to the fixture. Rebuild it: cargo install --path yidam/cli',
  )
  // Two entries, one expired: the counts overlap and neither renders the other.
  assert.equal(report.gate.baselined_violations, 2)
  assert.equal(report.gate.expired_baseline_entries!.length, 1)
  assert.equal(report.gate.passed, false)

  // And the violation itself is inherited debt, which is exactly why reading the violations
  // cannot find this: it is a Hint in the Problems panel, tagged `yidam (baseline)`.
  const dangling = report.checks.find((c) => c.id === 'dangling-edge')!
  assert.equal(dangling.violations[0].in_baseline, true)
  const finding = fromLint(report).findings.find((f) => f.code === 'dangling-edge')!
  assert.equal(finding.level, 'hint')

  // What a reader sees instead: a named row under Lint, and a repository-level condition.
  const row = healthTree({ lint: report, graph: null, index: null, regen: null, doctor: null })
    .find((r) => r.id === 'health:lint')!
  assert.match(row.description!, / · 1 expired$/)
  assert.equal(row.children!.length, 1)
  assert.equal(row.children![0].file, '.yidam/corpus/concept/low-flow.yml')
  assert.equal(row.command, undefined, 'blessing cannot clear an expired entry')

  const conditions = fromLint(report).conditions
  assert.deepEqual(conditions.map((c) => c.kind), ['expired-baseline'])
})
