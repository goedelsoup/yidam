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

export interface StatusReport extends Envelope {
  nodes: number
  open_questions: number
  catalog_entries: number
  claims_verified: number
  claims_inference: number
  claims_open: number
  index_present: boolean
  active_phases: number
  genesis: string
}

export interface IndexRow {
  node: string
  class: string
  label: string
  links_out: number
  claims_verified: number
  claims_inference: number
  claims_open: number
  lines: number
}

export interface CorpusIndexReport extends Envelope {
  nodes: IndexRow[]
}

export interface OpenQuestionsReport extends Envelope {
  open_questions: { node: string; label: string }[]
}

export interface PhasesReport extends Envelope {
  phases: { name: string; ref_name: string; owner: string; started: string; commits: number }[]
}

/**
 * `index-status`. Carries `built_at` and no age string, deliberately: an age is a function
 * of when you ask, so the CLI reports the stamp and the client renders the age.
 */
export interface IndexStatusReport extends Envelope {
  index_present: boolean
  meta_present: boolean
  built_at: number | null
  built: string | null
  model: string | null
  embedding_dim: number | null
  node_count: number | null
  stale_nodes: number
}

export interface Elector {
  name: string
  branch: string
  role: string
  branch_present: boolean
}

export interface SanghaReport extends Envelope {
  collective: boolean
  electors: Elector[]
  positions: { file: string; elector: string; question: string }[]
  resolutions: {
    file: string
    evolution: string
    date: string
    tips: string[]
    branch_present: boolean
  }[]
}

/** `regen --check`. The verdict that turned the Health view's REGEN row into a gate. */
export interface RegenReport extends Envelope {
  passed: boolean
  stale: { file: string; generator: string }[]
}
