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
  phases: {
    name: string
    ref_name: string
    owner: string
    started: string
    commits: number
    /**
     * `active`, `settled`, or `position`. Optional because a pinned binary older than the
     * field omits it, and a view that renders `undefined` is worse than one that renders
     * nothing.
     */
    state?: string
  }[]
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

/**
 * `catalog-audit`. The provenance layer, and the one report that names both ends of an edge.
 *
 * `cited_by` is what makes a source placeable under the node that draws on it. Without it a
 * client wanting that had to re-resolve every link in the corpus — which is the
 * re-implementation this whole contract exists to prevent, arrived at from the other side.
 */
export interface CatalogAuditReport extends Envelope {
  sources: SourceRow[]
}

export interface SourceRow {
  /** File name within `.yidam/catalog/`. */
  entry: string
  type: string
  description: string
  /** Absent in the entry means obtained; only an explicit `false` claims otherwise. */
  obtained: boolean
  /** `nodes` + `elsewhere`, retained with the meaning it had before the two were split. */
  citations: number
  /** Corpus instances linking here. **The number every gate reads.** */
  nodes: number
  /** Class definitions and READMEs linking here. Never added to `nodes`. */
  elsewhere: number
  /** Repo-relative paths of those instances, sorted. Exactly `nodes` of them. */
  cited_by: string[]
  /** The entry's declared `used-by`, verbatim. Empty when it declares none. */
  used_by: string[]
  /**
   * How that list disagrees with the citations, or null when none is declared.
   *
   * Null and an empty drift are different answers. **Computed by the CLI**, from the same
   * function `catalog-used-by-drift` gates on — recomputing it here would be the editor
   * forming a second opinion about a verdict.
   */
  drift: { claimed_not_citing: string[]; citing_not_claimed: string[] } | null
}

/**
 * `doctor`. Whether the *setup* is sound, which is a different question from whether the
 * corpus is — and the one a reader hits first.
 *
 * Writes nothing and does no network, which is what makes it safe on the save path.
 */
export interface DoctorReport extends Envelope {
  passed: boolean
  strict: boolean
  failed: number
  warned: number
  checks: DoctorCheck[]
}

export interface DoctorCheck {
  /** Stable identifier. Key on this; the prose is free to change. */
  id: string
  question: string
  verdict: 'ok' | 'warn' | 'fail' | 'skipped'
  detail: string
  /** The command or edit that resolves it, stated and never run. Null when nothing to do. */
  remedy: string | null
}
