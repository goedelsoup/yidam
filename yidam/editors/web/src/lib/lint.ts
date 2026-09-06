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

export interface LintReport {
  gate?: { passed: boolean; new_violations: number; baselined_violations: number }
  checks?: Check[]
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
