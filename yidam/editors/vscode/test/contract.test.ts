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
import { test } from 'node:test'

import { fromLint } from '../src/diagnostics.ts'
import { hasFeature, readHandshake } from '../src/handshake.ts'
import type { LintReport } from '../src/reports.ts'
import { captureStreams, contractBinary, SKIP, stageFixture } from './stage.ts'








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
