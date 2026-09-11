/**
 * The lint report, as the CLI reported it.
 *
 * Shapes and one transcription, and no judgement of any kind — the same rule `graph.ts`
 * states and for the same reason. `yidam lint` decided which findings gate before this
 * process saw the envelope; everything here reads the field that carries that decision.
 *
 * These types lived inline in `pages/reports.astro` until #655, which is part of why that
 * page could read the wrong field for a year without a test noticing: a shape declared in a
 * template is a shape nothing can be exercised against. `test/severity.mjs` is what holds
 * this one now.
 */

export interface Span {
  line: number
}

export type Severity = 'error' | 'warn' | 'info'

export interface Violation {
  node: string
  detail: string
  /**
   * This finding's own severity, and the one that decides whether it **gates**.
   *
   * Normally its check's, and not always: `Check::severity_of` in the CLI returns an
   * override or an age escalation before falling back to the check's level, so
   * `missing-property` is declared `warn` and raises the one property a class marked
   * `required: true` to `error`.
   *
   * **Optional because a binary older than #645 does not emit it, not because a violation
   * may lack one.** `report.schema.json` makes it required, and adding a field does not
   * raise `format_version`, so the handshake cannot tell the two apart — see `severityOf`.
   */
  severity?: Severity
  /**
   * Whether the committed baseline already records this violation.
   *
   * Only meaningful when *this violation's* severity is `error`: the baseline records
   * error-severity violations and nothing else. Read it with `severityOf` and never with
   * the check's declared level, which is a different question.
   */
  in_baseline: boolean
  span?: Span
}

export interface Check {
  id: string
  title: string
  /**
   * The severity the check is *declared* at — what the check is **for**, not what its
   * findings are. Render it in the check's heading; never use it to render a finding.
   */
  severity: Severity
  rationale: string
  violations: Violation[]
}

/** A baseline entry with no corresponding violation. **These fail the gate.** */
export interface StaleEntry {
  check: string
  node: string
}

/**
 * A baseline entry that has outlived the expiry the baseline declares.
 *
 * It no longer forgives the violation it lists, so that violation gates again — and it is
 * **not** the same event as an introduced one. An introduced violation is something this
 * change did; an expired one is something the repository agreed to deal with and has not.
 * Telling somebody they introduced a finding that has sat in their baseline for two hundred
 * commits would be wrong, so this is rendered apart from `new_violations` and apart from
 * `StaleEntry`. Mirrors the extension's `reports.ts`, deliberately: a second opinion about
 * these three states is how this page came to hold none of them (#751).
 */
export interface ExpiredEntry {
  check: string
  node: string
  /**
   * How many corpus-touching commits the entry survived.
   *
   * Commits rather than days: a day count is a function of when you ask, a commit count is a
   * function of HEAD.
   */
  commits: number
}

export interface LintReport {
  gate?: {
    passed: boolean
    new_violations: number
    baselined_violations: number
    stale_baseline_entries: StaleEntry[]
    /**
     * Entries the baseline no longer forgives. **These fail the gate.**
     *
     * Counted in `baselined_violations` as well — the baseline does list them — so the two
     * numbers overlap and neither can be derived from the other.
     *
     * **Optional because a binary older than the expiry clock does not emit it, not because
     * a report may lack it.** `report.schema.json` makes it required, and adding a field does
     * not raise `format_version`, so the handshake cannot tell the two apart — the same drift
     * `Violation.severity` documents above. Absent means *this binary has no expiry clock*,
     * which is the same answer as an empty list.
     */
    expired_baseline_entries?: ExpiredEntry[]
  }
  checks?: Check[]
}

/** Why the gate failed, for the two causes that are not a violation in a file. */
export interface GateCause {
  kind: 'expired' | 'stale'
  check: string
  node: string
  /** What to do about it. The remedies are opposites, which is why they are not one row. */
  detail: string
}

/**
 * The verdict sentence, and the causes behind it — rendered *from* the gate so the page and
 * the report cannot say different things.
 *
 * **A page may not list the reasons a gate did not fail.** This one did. It was written
 * against three of the gate's five fields, so a gate failing on an expired entry read
 * `Gate failed — 1 new, 2 in baseline.`: one violation is new and two are inherited, both
 * true, neither the reason. With `new_violations: 0` and a stale entry it was worse —
 * `Gate failed — 0 new, N in baseline` is a page stating a failure and then listing only
 * things that do not fail (#751).
 *
 * `summary` carries the four counts in the extension's order and omits a zero *cause*, which
 * is `tree/model.ts`'s convention for the same four numbers. `new` and `in baseline` are
 * always shown — zero new is a real and readable state, and it is the one a reader most wants
 * when the gate failed for another reason. A permanent `· 0 expired` is a number nobody can
 * act on, so it is absent unless there is one.
 */
export function gateVerdict(gate: NonNullable<LintReport['gate']>): {
  passed: boolean
  summary: string
  causes: GateCause[]
} {
  const expired = gate.expired_baseline_entries ?? []
  const stale = gate.stale_baseline_entries ?? []
  const counts = [`${gate.new_violations} new`, `${gate.baselined_violations} in baseline`]
  if (expired.length > 0) counts.push(`${expired.length} expired`)
  if (stale.length > 0) counts.push(`${stale.length} stale`)

  return {
    passed: gate.passed,
    summary: `Gate ${gate.passed ? 'passed' : 'failed'} — ${counts.join(', ')}.`,
    // Expired first: it is the one of the two that is owed rather than merely untidy — the
    // order `tree/model.ts` renders them in.
    causes: [
      ...expired.map((e): GateCause => ({
        kind: 'expired',
        check: e.check,
        node: e.node,
        // No backticks in any of this prose: it is rendered as page text rather than as
        // markdown, unlike the extension's tooltips, so a backtick reaches the reader as a
        // backtick.
        detail:
          `out of time after ${e.commits} commit(s) — nothing introduced it, and blessing ` +
          'will not clear it, because the entry\u2019s recorded commit is carried forward ' +
          'rather than restamped. Fix the finding, or raise expire_after in the baseline.',
      })),
      ...stale.map((e): GateCause => ({
        kind: 'stale',
        check: e.check,
        node: e.node,
        detail:
          'no longer violated — fixing it was good, leaving it listed is not, because a ' +
          'baseline permitted to be wrong drifts. Run yidam lint --bless.',
      })),
    ],
  }
}

/**
 * The severity to render a finding at: **its own**, and its check's only as a fallback.
 *
 * Nothing is decided here. `Check::severity_of` in the CLI already answered and
 * `violation.severity` is what it returned; this names the one case where the field is
 * absent — a binary older than #645, whose own gate read the check's level, so the check's
 * level is the answer that binary would have given.
 *
 * Reading `check.severity` instead is #655: a `missing-property` finding on a
 * `required: true` property fails CI, and this page rendered it as `warn` with the baseline
 * tag suppressed, because both were keyed off the level the check is declared at.
 */
export function severityOf(v: Violation, check: Check): Severity {
  return v.severity ?? check.severity
}

/**
 * Whether to show a finding's own severity beside it.
 *
 * Only where it disagrees with the heading above it, which is what `yidam lint`'s own text
 * report does — it appends `[ERROR]` to an escalated finding and nothing to the rest. A tag
 * on every violation would repeat the heading five times and bury the one line that differs
 * from it, which is the line a reader is looking for.
 */
export function escalated(v: Violation, check: Check): boolean {
  return severityOf(v, check) !== check.severity
}
