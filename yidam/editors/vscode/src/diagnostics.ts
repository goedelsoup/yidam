/**
 * Report JSON to editor findings. **This mapping is the whole issue.**
 *
 * `yidam lint` does not ask *is the corpus clean?* It asks *did this change make it less
 * clean?*, gating against a committed baseline. Its own module doc says conflating the two
 * "produces a gate that is either permanently red or permanently ignored".
 *
 * An extension that renders every finding as an Error reproduces that failure one layer up.
 * A Problems panel permanently full of inherited debt is a Problems panel nobody reads —
 * and the debt is exactly what the ratchet exists to hold still rather than to shout about.
 *
 * So **severity is a function of baseline membership, not of check severity alone.**
 *
 * No `vscode` import: the mapping is decided here and rendered elsewhere, so the part worth
 * getting right is exercised by plain node.
 */

import type { GraphCheckReport, LintReport } from './reports.ts'

/** Editor-neutral severity. Mapped to `vscode.DiagnosticSeverity` at the boundary. */
export type Level = 'error' | 'warning' | 'information' | 'hint'

export interface Finding {
  /** Repo-relative path of the file to mark. */
  file: string
  /** 1-based. Line 1 when the report carried no span — a stated fallback, not a guess. */
  line: number
  level: Level
  /** The check id, surfaced as the diagnostic's `code`. */
  code: string
  message: string
  /** The check's rationale, surfaced as hover — `--explain` without a second command. */
  rationale: string
  baselined: boolean
  /** `yidam` or `yidam (baseline)`, so the panel's own grouping separates debt from news. */
  source: string
}

/**
 * A condition about the repository rather than about any file.
 *
 * A baseline entry that no longer occurs fails the gate — but rendering it per-file would
 * put a squiggle on a file whose problem is that it *no longer has one*. It is a
 * repository-level condition with a repository-level fix, and it belongs in a view with a
 * Bless action, not in the Problems panel.
 */
export interface RepoCondition {
  kind: 'stale-baseline' | 'graph-gate'
  message: string
  /** For a stale entry: the check that lists it. */
  check?: string
  node?: string
}

export interface Mapped {
  findings: Finding[]
  conditions: RepoCondition[]
}

export interface Options {
  /**
   * Whether baselined violations appear at all. Default true, as Hints.
   *
   * A repository carrying heavy inherited debt can quiet them; the debt is still in the
   * baseline file and still in the gate, so hiding it here loses nothing a reader needed.
   */
  showBaselined: boolean
}

export const DEFAULT_OPTIONS: Options = { showBaselined: true }

function levelFor(severity: 'error' | 'warn' | 'info', inBaseline: boolean): Level {
  // Baseline membership outranks check severity. An inherited error is not news; it is the
  // state the ratchet was installed to hold, and the thing that fails CI is a *change* to
  // it.
  if (inBaseline) return 'hint'
  switch (severity) {
    case 'error':
      return 'error'
    case 'warn':
      return 'warning'
    case 'info':
      return 'information'
  }
}

/**
 * Split a node identity into a file and a line.
 *
 * Three checks encode a line in the identity itself — the prose-link and annotation checks
 * — because a file can trip them many times and the path alone would collapse those into
 * one. That line is exact; prefer it over anything else available.
 */
function locate(node: string, span?: { line: number }): { file: string; line: number } {
  const m = /^(.*):(\d+)$/.exec(node)
  if (m) return { file: m[1], line: Number(m[2]) }
  return { file: node, line: span?.line ?? 1 }
}

export function fromLint(report: LintReport, opts: Options = DEFAULT_OPTIONS): Mapped {
  const findings: Finding[] = []
  for (const check of report.checks) {
    for (const v of check.violations) {
      if (v.in_baseline && !opts.showBaselined) continue
      const { file, line } = locate(v.node, v.span)
      findings.push({
        file,
        line,
        level: levelFor(check.severity, v.in_baseline),
        code: check.id,
        message: v.detail,
        rationale: check.rationale,
        baselined: v.in_baseline,
        source: v.in_baseline ? 'yidam (baseline)' : 'yidam',
      })
    }
  }

  const conditions: RepoCondition[] = report.gate.stale_baseline_entries.map((e) => ({
    kind: 'stale-baseline',
    check: e.check,
    node: e.node,
    message:
      `${e.check} lists ${e.node}, which no longer occurs. ` +
      'Fixing a violation is good; leaving it listed is not — a baseline permitted to be ' +
      'wrong drifts. Run `yidam lint --bless`.',
  }))

  return { findings, conditions }
}

/**
 * Graph-check contributes what lint did not already say about a file, and a verdict.
 *
 * Its node checks are a subset of lint's, and it carries neither a baseline nor spans. On a
 * real 90-node corpus it finds nothing where lint finds 92; on the reports fixture it
 * reports the *same* broken edge as `dangling-edge` — but with no span, so rendering both
 * puts one accurate mark on the offending line and a second, redundant one at line 1.
 *
 * So lint owns the diagnostics, and this fills only the gap: any node graph-check objects
 * to that lint did not mention. That keeps the strictly-more-informative source in front
 * without silently dropping a gate CI actually runs.
 */
export function fromGraphCheck(report: GraphCheckReport, alreadyReported: Set<string>): Mapped {
  const findings: Finding[] = []
  for (const n of report.nodes_with_issues) {
    if (alreadyReported.has(n.node)) continue
    for (const issue of n.issues) {
      findings.push({
        file: n.node,
        // graph-check carries no spans. Line 1 is the contract's stated fallback.
        line: 1,
        level: 'error',
        code: 'graph-check',
        message: issue,
        rationale:
          'The graph gate: every instance must carry a class, a label, and at least one ' +
          'edge that resolves. It runs from the genesis commit onward and exits nonzero ' +
          'on any finding — there is no baseline to inherit against.',
        baselined: false,
        source: 'yidam',
      })
    }
  }

  const conditions: RepoCondition[] = report.passed
    ? []
    : [
        {
          kind: 'graph-gate',
          message: `The graph gate fails: ${report.nodes_with_issues.length} of ${report.total_instances} instance(s) have issues.`,
        },
      ]

  return { findings, conditions }
}

/** Files a set of findings touches — the collection keys to clear and re-set. */
export function filesTouched(findings: Finding[]): Set<string> {
  return new Set(findings.map((f) => f.file))
}
