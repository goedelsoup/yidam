/**
 * Which severity a finding renders at, and the field that decides it.
 *
 * #655: all four consumers of the report contract read `check.severity` where the contract
 * says to read `violation.severity`, so a `missing-property` finding on a property a class
 * declared `required: true` — which fails CI — rendered as `warn` on this page, with the
 * baseline tag suppressed because that tag was keyed off the same wrong field.
 *
 * It stayed invisible because no fixture had a violation whose severity was not its check's:
 * the wrong field gave the right answer on every input anybody looked at. So the first case
 * below is the one that matters, and the second is what stops it being satisfied by a reader
 * that ignores severity and always says `error`.
 *
 * `escalated` is the other half. It decides whether a finding's severity is shown beside it
 * at all, and a page that renders the tag on everything says nothing by saying it five times.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'

import { escalated, severityOf } from '../src/lib/lint.ts'

/** `missing-property`, as `yidam lint` reports it against the reports fixture. */
const CHECK = {
  id: 'missing-property',
  title: 'Declared property the instance omits',
  severity: 'warn',
  rationale: 'A property declared `required: true` GATES.',
  violations: [],
}

const REQUIRED = {
  node: '.yidam/corpus/concept/low-flow.yml',
  detail: '`datum` is declared by `.yidam/corpus/concept.ont.yml` as `required: true` …',
  severity: 'error',
  in_baseline: false,
}

const OPTIONAL = {
  node: '.yidam/corpus/concept/tailwater.yml',
  detail: '`claim_tag` is declared by `.yidam/corpus/concept.ont.yml` …',
  severity: 'warn',
  in_baseline: false,
}

test('a finding raised past its check renders at its own severity', () => {
  assert.equal(severityOf(REQUIRED, CHECK), 'error')
})

test('and its siblings still render at the check’s', () => {
  // The guard against a reader that stopped consulting severity at all. Without it,
  // `() => 'error'` passes the test above.
  assert.equal(severityOf(OPTIONAL, CHECK), 'warn')
})

test('the baseline tag is gated on the finding, so an escalated one can carry it', () => {
  // The tag's condition, as the page spells it. It read `check.severity === 'error'`, which
  // is false for every finding of a `warn`-declared check — including the ones that gate,
  // which are the only ones the baseline can hold.
  const inherited = { ...REQUIRED, in_baseline: true }
  assert.equal(severityOf(inherited, CHECK) === 'error' && inherited.in_baseline, true)
})

test('an absent severity falls back to the check — a binary older than #645, not an override', () => {
  // `report.schema.json` makes the field required, and adding a field does not raise
  // `format_version`, so a pinned older binary is a normal state rather than a broken one.
  // That binary's own gate read the check's level, so the check's level is its answer.
  const { severity: _dropped, ...older } = REQUIRED
  assert.equal(severityOf(older, CHECK), 'warn')
  assert.equal(escalated(older, CHECK), false)
})

test('the severity beside a finding appears only where it contradicts the heading', () => {
  assert.equal(escalated(REQUIRED, CHECK), true)
  assert.equal(escalated(OPTIONAL, CHECK), false)
  // And never on a check whose findings all carry its own level, however severe.
  const dangling = { ...CHECK, id: 'dangling-edge', severity: 'error' }
  assert.equal(escalated({ ...REQUIRED, severity: 'error' }, dangling), false)
})
