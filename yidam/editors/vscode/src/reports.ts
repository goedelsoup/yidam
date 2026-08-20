/**
 * Typed views of the report contract — RFC-0016 Phase 0's JSON, as this extension reads it.
 *
 * These describe what the CLI emits. They are not a model of the corpus and nothing here
 * derives anything: every field is transcribed. The moment this file starts *computing* a
 * verdict, the boundary the whole RFC set exists to hold has moved.
 */

export interface Envelope {
  format_version: string
  yidam: { version: string; commit: string; features: string[] }
  root: string
}

export interface Span {
  line: number
}

export interface Violation {
  node: string
  detail: string
  /**
   * Whether the committed baseline already records this violation.
   *
   * **Only meaningful when the check's severity is `error`** — the baseline records
   * error-severity violations and nothing else, so a warn or info violation is always
   * `false`. Read with `severity`, never alone.
   */
  in_baseline: boolean
  span?: Span
}

export interface Check {
  id: string
  title: string
  severity: 'error' | 'warn' | 'info'
  rationale: string
  violations: Violation[]
}

export interface StaleEntry {
  check: string
  node: string
}

export interface LintReport extends Envelope {
  gate: {
    passed: boolean
    new_violations: number
    baselined_violations: number
    stale_baseline_entries: StaleEntry[]
  }
  checks: Check[]
}

export interface GraphCheckReport extends Envelope {
  passed: boolean
  corpus_empty: boolean
  total_instances: number
  clean_instances: number
  classes_defined: number
  nodes_with_issues: { node: string; issues: string[] }[]
  classes_without_instances: string[]
}
