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
import {
  captureStreams,
  contractBinary,
  contractRefusal,
  FIXTURE_DIR,
  SKIP,
  stageFixture,
} from './stage.ts'

// ── the probe that decides whether the rest of this file means anything ──────

/** A golden, as text, the way `contractRefusal` receives a binary's stdout. */
const goldenText = (name: string): string =>
  fs.readFileSync(path.join(FIXTURE_DIR, `expected/${name}.json`), 'utf8')

/**
 * The refusal arm, reached without a stale yidam on hand.
 *
 * Every other test here needs a binary and skips without one, so the probe's *refusal* had
 * never run anywhere — and a guard whose failing arm is never taken is a guard that may have
 * stopped guarding without going red. This drives it over the committed goldens instead,
 * which is also the only way to reach the case #752 is about: 0.9.0 is not something CI can
 * be asked to have on hand.
 */
test('a binary agreeing about status and disagreeing about lint is refused, by name (#752)', () => {
  const status = goldenText('status')
  const lint = goldenText('lint')

  // The goldens describe the binary that produced them, so agreement is the baseline this
  // test mutates away from. Without this line every assertion below would also pass against
  // a `contractRefusal` that refused everything.
  assert.equal(contractRefusal('yidam', { status, lint }), null)

  // What 0.9.0 looked like from here: `status` byte-identical — it reports node, question,
  // catalog and claim counts, and none of those changed — while `lint` disagrees, because
  // that binary does not carry `resolution-elector-unregistered` at all, so the baseline
  // entry for it reads as stale rather than as inherited debt. `status` alone accepted it,
  // and the disagreement then surfaced as `1 !== 2` three assertions downstream (#657).
  const old = JSON.parse(lint) as LintReport
  old.gate.baselined_violations = 1
  old.gate.stale_baseline_entries.push({
    check: 'resolution-elector-unregistered',
    node: '.yidam/sangha/resolutions/silt-budget.md',
  })
  const why = contractRefusal('/opt/homebrew/bin/yidam', { status, lint: JSON.stringify(old) })
  assert.ok(why, 'a lint gate that disagrees with the golden is what this probe is for')
  // Named: which report, which command, and what to do about it.
  assert.match(why, /`lint` golden/)
  assert.match(why, /yidam lint --format json/)
  assert.match(why, /cargo install --path yidam\/cli/)
  assert.match(why, /opt\/homebrew\/bin\/yidam/)

  // And the other direction still names `status`, so the message is derived from the probe
  // that failed rather than written once for the loudest case.
  const counts = JSON.parse(status) as { open_questions: number }
  counts.open_questions = 1
  const other = contractRefusal('yidam', { status: JSON.stringify(counts), lint })
  assert.match(other ?? '', /`status` golden/)
  assert.match(other ?? '', /yidam status --format json/)
})

/**
 * Why the lint probe compares `gate` and not the whole report.
 *
 * The rest of that golden is redacted — `age.first_commit` is `<FIRST_COMMIT>`, because a
 * sha is a function of the tree that holds it — so comparing all of it would need
 * `redact()`'s rules transcribed a third time. `gate` needs none. This says so by mutating
 * the redacted field to a plausible real value: a whole-payload comparison would refuse a
 * healthy binary here, which is the failure that would make everyone turn the probe off.
 */
test('the lint probe compares the gate, so a redacted field is not a disagreement', () => {
  // Read loosely rather than as a `LintReport`: this reader does not model `age` at all,
  // which is part of the point — the probe must not refuse over a field nobody here reads.
  const report = JSON.parse(goldenText('lint')) as {
    checks: { violations: { age?: { first_commit: string } }[] }[]
  }
  const aged = report.checks.flatMap((c) => c.violations).filter((v) => v.age !== undefined)
  assert.ok(aged.length > 0, 'the fixture carries an aged violation — that is the premise')
  for (const v of aged) v.age!.first_commit = '9f2c1ab0d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9'

  assert.equal(
    contractRefusal('yidam', { status: goldenText('status'), lint: JSON.stringify(report) }),
    null,
  )
})

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
  // The fixture's baseline lists three entries and declares `expire_after: 2`. One carries a
  // `since` three corpus-touching commits back and has run out of time; one carries none at all
  // and never expires; the third names a check this corpus does not trip, so nothing consumes it
  // and it is stale (#660). Three states, three numbers, and none derives another — which is why
  // the gate fails here with an expired entry rather than an introduced one. That was the state
  // that reached the Health view as a red row with no children and no stated cause, for as long
  // as this reader's report type omitted the field (#657).
  const { stdout } = captureStreams(bin, ['lint', '--format', 'json'], dir)
  const report = JSON.parse(stdout) as LintReport

  // Against the committed golden rather than a transcription of it. RFC-0016 asks for
  // exactly this — the `reports/` goldens become the repository the extension is exercised
  // on. It is also a second statement of what `contractBinary` now certifies: its probe
  // read `status` alone until #752, so a yidam predating a *lint check* resolved, passed,
  // and disagreed here — as `1 !== 2` before this comparison existed, which is what it did
  // when 0.9.0 was on PATH. The probe refuses that binary now; this stays because a
  // disagreement about the gate is exactly what this test is reading.
  const golden = JSON.parse(
    fs.readFileSync(path.join(FIXTURE_DIR, 'expected/lint.json'), 'utf8'),
  ) as LintReport
  assert.deepEqual(
    report.gate,
    golden.gate,
    'this yidam does not reproduce the fixture\'s committed lint gate — it is stale relative ' +
      'to the fixture. Rebuild it: cargo install --path yidam/cli',
  )
  // Three entries: two describe violations that occur (so they are inherited debt, one of it
  // expired) and one describes nothing. The counts overlap and no one of them renders another.
  assert.equal(report.gate.baselined_violations, 2)
  assert.equal(report.gate.expired_baseline_entries!.length, 1)
  assert.equal(report.gate.stale_baseline_entries.length, 1)
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
  // Anchored at the end, so a fourth count appearing has to be accounted for here rather than
  // slipping in behind a substring match. It already caught one: adding the stale entry to the
  // fixture failed this line, which is how the row was confirmed to render both.
  assert.match(row.description!, / · 1 expired · 1 stale$/)
  assert.deepEqual(
    row.children!.map((c) => c.id),
    [
      'health:expired:dangling-edge:.yidam/corpus/concept/low-flow.yml',
      'health:stale:unknown-class:.yidam/corpus/concept/mixing-zone.yml',
    ],
    'both baseline conditions are named rows, and the expired one comes first',
  )
  // The two rows offer opposite affordances, and that is the design rather than an oversight.
  // An expired entry's violation is on a file, so its row opens the file and deliberately does
  // not offer blessing — blessing carries `since` forward and would forgive the debt again. A
  // stale entry's problem is that its file no longer has the violation, so there is nothing to
  // open and blessing is exactly the fix.
  const [expiredRow, staleRow] = row.children!
  assert.equal(expiredRow.file, '.yidam/corpus/concept/low-flow.yml')
  assert.equal(expiredRow.command, undefined, 'an expired row must not offer blessing')
  assert.equal(staleRow.file, undefined, 'a stale entry has no file to point a reader at')
  assert.equal(staleRow.command?.id, 'yidam.blessBaseline')
  // Two disqualifications now, and the assertion cannot tell them apart — blessing is withheld
  // because an entry is expired *and* because the gate has an introduced violation. It is the
  // expired one that is load-bearing: `--bless` carries `since` forward rather than restamping.
  assert.equal(row.command, undefined, 'blessing cannot clear an expired entry')

  const conditions = fromLint(report).conditions
  assert.deepEqual(conditions.map((c) => c.kind), ['expired-baseline', 'stale-baseline'])
})
