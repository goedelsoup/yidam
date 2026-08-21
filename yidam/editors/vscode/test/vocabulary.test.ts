/**
 * The commit box: what is offered, where the underline goes, and who decides.
 *
 * The last one is the point. Nothing in `src/vocabulary.ts` knows which verbs are legal —
 * the list and the verdict both cross the process boundary from the pinned binary, where
 * `is_recognized_verb` and `classify_commit` are parity-certified against three SDKs and a
 * Dafny spec. What is asserted here is the part that *is* ours: when to offer, where to
 * draw, and that the severity is transcribed rather than chosen.
 */

import assert from 'node:assert/strict'
import { test } from 'node:test'

import { capture, contractBinary, SKIP, stageFixture } from './stage.ts'
import {
  completions,
  inVerbPosition,
  marks,
  subjectLine,
  type SubjectCheck,
  type VocabularyReport,
} from '../src/vocabulary.ts'

const REPORT: VocabularyReport = {
  format_version: '1',
  yidam: { version: '0.1.0', commit: 'abc1234', features: ['reports'] },
  root: '/r',
  source: '.yidam/.vendor/prelude/GRAPH.md',
  verbs: [
    { verb: 'establish', kind: 'epistemic', when: 'New understanding committed' },
    { verb: 'revise', kind: 'epistemic', when: 'Committed understanding corrected' },
    { verb: 'bundle', kind: 'operational', when: 'The export bundle regenerated' },
  ],
  drift: [],
}

/**
 * Once `: ` is behind the cursor the user is writing prose, and thirty verbs there is
 * noise. Line 0 only — the body of a commit message is not a subject.
 */
test('verbs are offered in the verb position and nowhere else', () => {
  assert.equal(inVerbPosition('est', 3, 0), true)
  assert.equal(inVerbPosition('', 0, 0), true)
  assert.equal(inVerbPosition('establish: the tail', 19, 0), false)
  // The cursor is still before the separator, so still in the verb position.
  assert.equal(inVerbPosition('establish: the tail', 4, 0), true)
  assert.equal(inVerbPosition('anything', 3, 1), false, 'the body is not a subject')
})

/**
 * The two kinds are the distinction the vocabulary exists to make. Interleaving them
 * alphabetically puts `bundle` between `assess` and `close`, which is exactly the
 * confusion a closed vocabulary is for.
 */
test('completion groups epistemic before operational', () => {
  const c = completions(REPORT)
  assert.deepEqual(
    c.map((x) => x.label),
    ['establish', 'revise', 'bundle'],
  )
  const sorted = [...c].sort((a, b) => a.sortText.localeCompare(b.sortText))
  assert.deepEqual(
    sorted.map((x) => x.label),
    ['establish', 'revise', 'bundle'],
  )
  assert.ok(sorted[2].sortText > sorted[1].sortText, 'operational sorts after epistemic')
})

/** The verb is never written without its separator — that is what makes it a verb. */
test('completing a verb completes the separator', () => {
  assert.equal(completions(REPORT)[0].insertText, 'establish: ')
})

test('the When column becomes the detail text, and its kind the documentation', () => {
  const c = completions(REPORT)
  assert.equal(c[0].detail, 'New understanding committed')
  assert.match(c[2].documentation, /Operational/)
})

/** No vendored GRAPH.md costs the prose and nothing else. */
test('a verb with no documented When still completes', () => {
  const bare = completions({ ...REPORT, source: '', verbs: [{ verb: 'establish', kind: 'epistemic', when: '' }] })
  assert.equal(bare[0].label, 'establish')
  assert.equal(bare[0].detail, 'epistemic')
})

function check(over: Partial<SubjectCheck>): SubjectCheck {
  return {
    text: 'lift: something',
    verb: 'lift',
    kind: 'epistemic',
    recognized: false,
    violations: [],
    ...over,
  }
}

/**
 * Under the verb token, not the whole line.
 *
 * A whole-line squiggle for a bad verb points at the subject the author got right.
 */
test('a verb finding underlines the verb', () => {
  const m = marks(
    check({
      verb: 'vendor(yidam)',
      text: 'vendor(yidam): the prelude',
      violations: [{ rule: 'scope-suffix', severity: 'warn', message: 'x' }],
    }),
  )
  assert.deepEqual([m[0].start, m[0].end], [0, 'vendor(yidam)'.length])
  assert.equal(m[0].code, 'scope-suffix')
})

/** A missing verb is about the whole line, because there is no token to point at. */
test('a missing verb underlines the line', () => {
  const m = marks(
    check({
      verb: '',
      text: 'just some words',
      violations: [{ rule: 'no-verb', severity: 'warn', message: 'x' }],
    }),
  )
  assert.deepEqual([m[0].start, m[0].end], [0, 'just some words'.length])
})

/**
 * The severity is transcribed, never chosen.
 *
 * `lint --commits` is Warn severity and correctly so. A commit box that squiggled harder
 * than the gate would be asserting a verdict nobody agreed to — and this is exactly the
 * spot where escalating feels helpful.
 */
test('the severity is whatever the CLI said', () => {
  for (const severity of ['error', 'warn', 'info'] as const) {
    const m = marks(check({ violations: [{ rule: 'unrecognized-verb', severity, message: 'x' }] }))
    assert.equal(m[0].severity, severity)
  }
})

test('only the first line is the subject', () => {
  assert.equal(subjectLine('establish: a node\n\nbody prose\n'), 'establish: a node')
  assert.equal(subjectLine(''), '')
})

// ── against the real binary ──────────────────────────────────────────────────





/**
 * The verdict, from the binary, for the exact subject GRAPH.md uses as its example.
 *
 * `vendor(yidam):` is the form the bootstrap skill used to prescribe — every derived
 * repository's first three commits reported by its own lint, and two operational commits
 * filed as Epistemic. It is the case this feature exists for, so it is the one checked
 * end to end.
 */
test('the real binary reports the scoped-verb case, and the marks land on the verb', async (t) => {
  const dir = stageFixture('yidam-vocab-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  const subject = 'vendor(yidam): the prelude at 4e1a2b0'
  const report = JSON.parse(
    capture(bin, ['vocabulary', '--check', subject, '--format', 'json'], dir),
  ) as VocabularyReport

  assert.ok(report.subject)
  assert.equal(report.subject.verb, 'vendor(yidam)')
  assert.equal(report.subject.recognized, false)
  // The half that is easy to miss: `vendor` is operational, and the scoped form is filed
  // as Epistemic. That is the second of the two costs GRAPH.md names.
  assert.equal(report.subject.kind, 'epistemic')

  const m = marks(report.subject)
  assert.equal(m.length, 1)
  assert.equal(m[0].code, 'scope-suffix')
  assert.deepEqual([m[0].start, m[0].end], [0, 'vendor(yidam)'.length])
  assert.equal(m[0].severity, 'warn', 'never escalated past what the gate reports')

  // And a legal subject says nothing.
  const clean = JSON.parse(
    capture(bin, ['vocabulary', '--check', 'establish: the tailwater node', '--format', 'json'], dir),
  ) as VocabularyReport
  assert.deepEqual(marks(clean.subject!), [])
})

/**
 * The list the box offers is the list the gate uses.
 *
 * A hardcoded copy in TypeScript would be a second source of truth for a rule the prelude
 * owns and re-vendoring may change — `resolve`, `scope` and `adopt` all arrived that way.
 * So this asserts the list came from the binary, and that every verb it offers is one the
 * binary then accepts.
 */
test('every verb offered is one the binary accepts', async (t) => {
  const dir = stageFixture('yidam-vocab-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  const report = JSON.parse(capture(bin, ['vocabulary', '--format', 'json'], dir)) as VocabularyReport
  assert.equal(report.drift.length, 0, report.drift.join('\n'))
  const offered = completions(report)
  assert.ok(offered.length >= 30, `${offered.length} verbs offered`)

  for (const c of offered) {
    const checked = JSON.parse(
      capture(bin, ['vocabulary', '--check', `${c.insertText}a subject`, '--format', 'json'], dir),
    ) as VocabularyReport
    assert.equal(checked.subject!.recognized, true, `offered \`${c.label}\` which the binary rejects`)
    assert.deepEqual(checked.subject!.violations, [], `offered \`${c.label}\` with a finding`)
  }
})
