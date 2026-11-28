/**
 * Typed views of the report contract — RFC-0016 Phase 0's JSON, as this extension reads it.
 *
 * These describe what the CLI emits. They are not a model of the corpus and nothing here
 * derives anything: every field is transcribed. The moment this file starts *computing* a
 * verdict, the boundary the whole RFC set exists to hold has moved.
 *
 * # Where the shapes come from
 *
 * **`report.schema.json`, by way of `report-schema.generated.d.ts`** (#1645). Nothing below
 * restates a field's type. Each view names the fields this extension reads and takes their
 * types from the generated envelope, so a field the schema renames or drops is a compile
 * error here rather than an `undefined` at the point of use, and `test/report-schema.test.ts`
 * holds the generated file to the schema and every committed golden to these views.
 *
 * The schema is one envelope with every command's fields flattened beside each other, so it
 * cannot say which fields a given command emits; that is what each view below adds, and the
 * only thing. Three helpers say how a view departs from the schema's optionality, and each
 * use is a statement about this command's output rather than about the type:
 *
 * - `Present` — the command always emits these. The envelope marks them optional because
 *   another command does not.
 * - `Older` — the schema requires these, and a binary older than the field omits them. A
 *   repository pins its binary while this extension auto-updates, and adding a field does not
 *   raise `format_version`, so the handshake cannot tell the two apart. The `?` is what makes
 *   the compiler require a guard at every read rather than trusting that they match.
 * - `Pick` — the fields read here. A field the CLI adds is not an error until something
 *   wants it.
 *
 * What this does **not** do is check anything at runtime: `fetchReport` still casts. The
 * goldens are checked against these types by the type checker, in a test; a live binary's
 * output is checked by the handshake's `format_version` and by nothing else.
 */

import type {
  ADoctorCheck,
  ALintCheck,
  Span as SchemaSpan,
  YidamReportEnvelope,
} from './report-schema.generated.d.ts'

type R = YidamReportEnvelope

/** These fields of `T`, required. See the header. */
export type Present<T, K extends keyof T> = { [P in K]-?: Exclude<T[P], undefined> }

/** These fields of `T`, optional, because a binary older than them omits them. See the header. */
export type Older<T, K extends keyof T> = { [P in K]?: T[P] }

/**
 * One row of an array-valued envelope field.
 *
 * `Extract` because three keys — `nodes`, `open_questions`, `positions` — are an array in one
 * command and an integer in another, and the row is the array arm's.
 */
export type Row<K extends keyof R> = Extract<Exclude<R[K], undefined>, readonly unknown[]>[number]

/** One command's integer arm of a key that is an array elsewhere. */
type Count<K extends keyof R> = Extract<Exclude<R[K], undefined>, number>

/**
 * The fields every report carries.
 *
 * `format_version` is `string` here where the schema says the constant `"1"`, deliberately:
 * the handshake exists to read a value this build does not understand and refuse it by name,
 * and a type that says the value can only be `"1"` would be the type lying about the one
 * case it is read for.
 */
export type Envelope = Present<R, 'yidam' | 'root'> & { format_version: string }

export type Span = SchemaSpan

type LintViolation = ALintCheck['violations'][number]

export type Violation = Pick<LintViolation, 'node' | 'detail' | 'in_baseline' | 'span'> &
  /**
   * `severity` is this finding's own severity, and the one that decides whether it
   * **gates** — normally the check's, and not always: `Check::severity_of` returns an
   * override or an age escalation first, so `missing-property` is declared `warn` and raises
   * the one property a class marked `required: true` to `error`. Read it here and never from
   * the check (#655).
   *
   * Older than #645 a binary does not emit it. Absent means *this binary predates the field*
   * — fall back to the check's severity, which is what that binary's own gate used.
   *
   * `in_baseline` is only meaningful when this violation's severity is `error`: the baseline
   * records error-severity violations and nothing else.
   */
  Older<LintViolation, 'severity'>

/**
 * A lint check. `severity` here is the level the check is *declared* at, which is what the
 * check is **for** — read it to describe a check, and `Violation.severity` to render a
 * finding.
 */
export type Check = Pick<ALintCheck, 'id' | 'title' | 'severity' | 'rationale'> & {
  violations: Violation[]
}

type Gate = Exclude<R['gate'], undefined>

export type StaleEntry = Gate['stale_baseline_entries'][number]

/**
 * A baseline entry that has outlived the expiry the baseline declares.
 *
 * It no longer forgives the violation it lists, so that violation gates again — and it is
 * **not** the same event as an introduced one. An introduced violation is something this
 * change did, an expired one is something the repository agreed to deal with and has not,
 * so these are rendered apart from `new_violations` and apart from `stale_baseline_entries`.
 *
 * The violation itself still carries `in_baseline: true`, because the baseline does still
 * list it. That is why an expired entry cannot be found by reading the violations alone.
 * `commits` is corpus-touching commits rather than days: a day count is a function of when
 * you ask, and a commit count is a function of HEAD.
 */
export type ExpiredEntry = Gate['expired_baseline_entries'][number]

export type LintReport = Envelope & {
  gate: Pick<
    Gate,
    'passed' | 'new_violations' | 'baselined_violations' | 'stale_baseline_entries'
  > &
    /**
     * Entries the baseline no longer forgives. **These fail the gate.** Counted in
     * `baselined_violations` as well, so the two numbers overlap and neither can be derived
     * from the other. Absent from a binary with no expiry clock, which is the same answer as
     * an empty list.
     */
    Older<Gate, 'expired_baseline_entries'>
  checks: Check[]
}

export type GraphCheckReport = Envelope &
  Present<
    R,
    | 'passed'
    | 'corpus_empty'
    | 'total_instances'
    | 'clean_instances'
    | 'classes_defined'
    | 'nodes_with_issues'
    | 'classes_without_instances'
  >

/**
 * `status`. The `edge_claims_*` three are the same counts over the corpus's edges (#857),
 * beside the node counts and never added to them; the schema itself leaves them optional,
 * because a CLI released before them must still validate.
 *
 * `genesis` is `string | null`: null in a projection, whose first commit is the host
 * repository's (RFC-0064 §2.3). The hand-written type before #1645 said `string`.
 */
export type StatusReport = Envelope &
  Present<
    R,
    | 'catalog_entries'
    | 'claims_verified'
    | 'claims_inference'
    | 'claims_open'
    | 'index_present'
    | 'active_phases'
    | 'genesis'
  > &
  Pick<R, 'edge_claims_verified' | 'edge_claims_inference' | 'edge_claims_open'> & {
    nodes: Count<'nodes'>
    open_questions: Count<'open_questions'>
  }

type NodeRow = Row<'nodes'>

/**
 * A `corpus-index` row. The schema's `nodes` row is a flat union of this and `graph`'s, so
 * it marks the counts optional; `corpus-index` emits all of them. The `edge_claims_*` three
 * are optional in the schema too, for `StatusReport`'s reason.
 */
export type IndexRow = Present<
  NodeRow,
  | 'node'
  | 'class'
  | 'label'
  | 'links_out'
  | 'claims_verified'
  | 'claims_inference'
  | 'claims_open'
  | 'lines'
> &
  Pick<NodeRow, 'edge_claims_verified' | 'edge_claims_inference' | 'edge_claims_open'>

export type CorpusIndexReport = Envelope & { nodes: IndexRow[] }

type OpenQuestionRow = Row<'open_questions'>

/**
 * `scope` is `node` or `edge` — which of the two asked it (#857). An edge tagged `open` is
 * listed in its own right beside the node ones, so two entries may name the same `node` path
 * and a view that deduplicated by path would drop one. `relationship` and `target` are
 * `scope: 'edge'` only.
 */
export type OpenQuestionsReport = Envelope & {
  open_questions: Pick<OpenQuestionRow, 'node' | 'label' | 'scope' | 'relationship' | 'target'>[]
}

type PhaseRow = Row<'phases'>

/**
 * `state` is `git::REF_STATES`, a closed enum in the schema and now in this type. `source` is
 * `record` or `ref` — which evidence decided `state` (RFC-0028 §3). Both are required by the
 * schema and absent from a binary older than them, and a view that renders `undefined` is
 * worse than one that renders nothing.
 *
 * `type` is absent — never blank — where the ref carries no record, which a view must render
 * as unknown rather than filling in a default (RFC-0028 Erratum 1).
 */
export type PhasesReport = Envelope & {
  phases: (Pick<PhaseRow, 'name' | 'ref_name' | 'owner' | 'started' | 'commits' | 'type'> &
    Older<PhaseRow, 'state' | 'source'>)[]
}

/**
 * `index-status`. Carries `built_at` and no age string, deliberately: an age is a function
 * of when you ask, so the CLI reports the stamp and the client renders the age. `row_count`
 * (RFC-0060) is absent from a CLI older than it.
 */
export type IndexStatusReport = Envelope &
  Present<
    R,
    | 'index_present'
    | 'meta_present'
    | 'built_at'
    | 'built'
    | 'model'
    | 'embedding_dim'
    | 'node_count'
    | 'stale_nodes'
  > &
  Older<R, 'row_count'>

export type Elector = Pick<Row<'electors'>, 'name' | 'branch' | 'role' | 'branch_present'>

export type SanghaReport = Envelope &
  Present<R, 'collective'> & {
    electors: Elector[]
    positions: Pick<Row<'positions'>, 'file' | 'elector' | 'question'>[]
    resolutions: Pick<
      Row<'resolutions'>,
      'file' | 'evolution' | 'date' | 'tips' | 'branch_present'
    >[]
  }

/** `regen --check`. The verdict that turned the Health view's REGEN row into a gate. */
export type RegenReport = Envelope &
  Present<R, 'passed'> & { stale: Pick<Row<'stale'>, 'file' | 'generator'>[] }

type SchemaSourceRow = Row<'sources'>

/**
 * A `catalog-audit` row — the provenance layer, and the one report that names both ends of
 * an edge.
 *
 * `cited_by` is what makes a source placeable under the node that draws on it; without it a
 * client had to re-resolve every link in the corpus, which is the re-implementation this
 * contract exists to prevent. A CLI older than 0.6.0 does not emit it. `used_by` and `drift`
 * are retained for compatibility (RFC-0053): empty and null from the CLI that retired the
 * stored list, an object from an older one, and absent from one older still. `drift` is
 * **computed by the CLI** — recomputing it here would be the editor forming a second opinion
 * about a verdict.
 */
export type SourceRow = Pick<
  SchemaSourceRow,
  'entry' | 'type' | 'description' | 'obtained' | 'citations' | 'nodes' | 'elsewhere'
> &
  Older<SchemaSourceRow, 'cited_by' | 'used_by' | 'drift'>

/** `catalog-audit`. */
export type CatalogAuditReport = Envelope & { sources: SourceRow[] }

/**
 * A `doctor` check. Key on `id`; the prose is free to change. `declined` is an offer decided
 * against in writing — not actionable, like `ok`. `remedy` is stated and never run.
 */
export type DoctorCheck = Pick<ADoctorCheck, 'id' | 'question' | 'verdict' | 'detail' | 'remedy'>

/**
 * `doctor`. Whether the *setup* is sound, which is a different question from whether the
 * corpus is — and the one a reader hits first. Writes nothing and does no network, which is
 * what makes it safe on the save path.
 *
 * `declined` and `unknown_declines` (RFC-0066) are absent from a binary older than them.
 */
export type DoctorReport = Envelope &
  Present<R, 'passed' | 'strict' | 'failed' | 'warned'> &
  Older<R, 'declined' | 'unknown_declines'> & { checks: DoctorCheck[] }
