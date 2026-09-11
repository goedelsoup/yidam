/**
 * What the reports page says when the gate fails, and whether it says why.
 *
 * #751: this page's `LintReport` typed three of the gate's five fields, and the sentence was
 * assembled in the template from those three. So a gate failing because a baseline entry had
 * run out of time read `Gate failed — 1 new, 2 in baseline.` — one violation is new and two
 * are inherited, both true, neither the reason. The expired one was down the page in the
 * `in-baseline` class, rendered as forgiven, next to the entry that still forgives it.
 *
 * The stale case is worse and older. A gate failing with `new_violations: 0` and a stale entry
 * read `Gate failed — 0 new, N in baseline`: **a page stating a failure and then listing only
 * the numbers that do not fail.** Nothing it printed was false and nothing it printed was the
 * answer.
 *
 * It stayed invisible for the reason #657 and #660 both record: `stale_baseline_entries` and
 * `expired_baseline_entries` were `[]` in every committed golden, so a reader that dropped
 * them was indistinguishable from one that read them. The fixture carries all three baseline
 * states now, which is why the first test below can be read off it rather than transcribed.
 *
 * **Against the golden, not a hand-written report.** `severity.mjs` transcribes its inputs and
 * says so; this cannot, because the defect *was* a reader disagreeing with the contract. A
 * transcription would have been written from the same three fields.
 */

import { strict as assert } from 'node:assert'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { gateVerdict } from '../src/lib/lint.ts'

const pkg = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const GOLDEN = path.join(
  pkg,
  '..',
  '..',
  'prelude',
  'sdks',
  'parity',
  'fixtures',
  'reports',
  'basic',
  'expected',
  'lint.json',
)

const golden = JSON.parse(readFileSync(GOLDEN, 'utf8'))

test('the fixture still holds all three baseline states', () => {
  // Not ceremony. Every assertion below is read off this gate, and the fixture reaching it
  // took two issues — so a fixture that quietly lost an arm would leave the tests passing
  // while checking a state nobody is in. This is the line that would say so.
  assert.equal(golden.gate.passed, false)
  assert.equal(golden.gate.new_violations, 1, 'one introduced violation')
  assert.equal(golden.gate.baselined_violations, 2, 'two entries describe violations that occur')
  assert.equal(golden.gate.expired_baseline_entries.length, 1, 'one of those is out of time')
  assert.equal(golden.gate.stale_baseline_entries.length, 1, 'and one entry describes nothing')
})

test('a failing gate names every count, including the two this page used to drop', () => {
  const verdict = gateVerdict(golden.gate)
  assert.equal(verdict.passed, false)
  assert.equal(verdict.summary, 'Gate failed — 1 new, 2 in baseline, 1 expired, 1 stale.')
})

test('and states each cause, distinguishably from a new violation', () => {
  const verdict = gateVerdict(golden.gate)
  // Expired first: it is the one of the two that is owed rather than merely untidy.
  assert.deepEqual(
    verdict.causes.map((c) => [c.kind, c.check, c.node]),
    [
      ['expired', 'dangling-edge', '.yidam/corpus/concept/low-flow.yml'],
      ['stale', 'unknown-class', '.yidam/corpus/concept/mixing-zone.yml'],
    ],
  )
  const [expired, stale] = verdict.causes
  // The remedies are opposites, and conflating them is the whole defect. Blessing is the fix
  // for a stale entry and is exactly the wrong move for an expired one.
  assert.match(expired.detail, /out of time after 3 commit\(s\)/)
  assert.match(expired.detail, /blessing will not clear it/)
  assert.doesNotMatch(expired.detail, /--bless/)
  assert.match(stale.detail, /yidam lint --bless/)
  // Rendered as page text rather than markdown, so a backtick reaches the reader as one.
  for (const cause of verdict.causes) {
    assert.doesNotMatch(cause.detail, /`/, `${cause.kind} detail carries a markdown backtick`)
  }
})

test('a gate that fails on a stale entry alone does not read as nothing wrong', () => {
  // The sharpest arm, and the one no golden can hold: this fixture has an introduced violation.
  // `0 new, 1 in baseline` was the whole of what the page said here.
  const verdict = gateVerdict({
    passed: false,
    new_violations: 0,
    baselined_violations: 1,
    stale_baseline_entries: [{ check: 'unknown-class', node: 'a.yml' }],
    expired_baseline_entries: [],
  })
  assert.equal(verdict.summary, 'Gate failed — 0 new, 1 in baseline, 1 stale.')
  assert.equal(verdict.causes.length, 1)
  assert.equal(verdict.causes[0].kind, 'stale')
})

test('zero new is still reported, and a zero cause is not', () => {
  // Opposite treatments, deliberately. Zero new violations is a real and readable state, and
  // it is what a reader most wants to know when the gate failed for another reason. A
  // permanent `0 expired` is a number nobody can act on.
  const verdict = gateVerdict({
    passed: true,
    new_violations: 0,
    baselined_violations: 2,
    stale_baseline_entries: [],
    expired_baseline_entries: [],
  })
  assert.equal(verdict.summary, 'Gate passed — 0 new, 2 in baseline.')
  assert.deepEqual(verdict.causes, [])
})

test('a binary with no expiry clock reports as one with nothing expired', () => {
  // The field is optional because an older pinned binary omits it, not because a report may
  // lack it — and `format_version` cannot tell those apart. Absent must read as empty rather
  // than throw, which is the guard the `?` exists to force.
  const verdict = gateVerdict({
    passed: false,
    new_violations: 1,
    baselined_violations: 0,
    stale_baseline_entries: [],
  })
  assert.equal(verdict.summary, 'Gate failed — 1 new, 0 in baseline.')
  assert.deepEqual(verdict.causes, [])
})
