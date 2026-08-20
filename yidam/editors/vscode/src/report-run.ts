/**
 * One pass of the reports over a workspace.
 *
 * Two passes, in fact, and the split is a measurement rather than a taste. Against a real
 * 105-node corpus with 23 settled resolutions, seven of the eight reports finish in under
 * 200ms each and `phases` takes 1.26s — it spawns three git processes per ref, and a
 * repository running a sangha has dozens. Re-running that on every save would make the
 * editor's cheapest event its most expensive.
 *
 * So: what a *save* can change is one group, and what a *ref* can change is the other.
 * `sangha` sits with the refs because it is mostly a question about branches, and its
 * files change on the order of once a resolution.
 *
 * Kept out of `extension.ts` so the ordering rule — lint owns the diagnostics, graph-check
 * fills the gap — is exercised without an editor. It is the one piece of composition in
 * this feature that could be quietly wrong.
 */

import { fromGraphCheck, fromLint, type Mapped, type Options } from './diagnostics.ts'
import { readHandshake, type Handshake } from './handshake.ts'
import type {
  CorpusIndexReport,
  GraphCheckReport,
  IndexStatusReport,
  LintReport,
  OpenQuestionsReport,
  PhasesReport,
  SanghaReport,
  StatusReport,
} from './reports.ts'
import type { Spawn } from './runner.ts'

export type Outcome =
  | {
      ok: true
      mapped: Mapped
      gatePassed: boolean
      /** Kept so the health view renders the same run the diagnostics did. */
      lint: LintReport
      graph: GraphCheckReport | null
    }
  | { ok: false; handshake: Handshake }

/**
 * Run one report and return it, or null when the binary did not produce a readable one.
 *
 * Null rather than a throw: a view whose report is missing renders as unavailable, and one
 * failed report must not take the other six with it.
 */
async function fetchReport<T>(
  bin: string,
  args: string[],
  cwd: string,
  run: Spawn,
): Promise<T | null> {
  const r = await run(bin, [...args, '--format', 'json'], cwd)
  if (!readHandshake(r.stdout, r.stderr).ok) return null
  try {
    return JSON.parse(r.stdout) as T
  } catch {
    return null
  }
}

export async function runReports(
  bin: string,
  cwd: string,
  opts: Options,
  run: Spawn,
): Promise<Outcome> {
  const lintRun = await run(bin, ['lint', '--format', 'json'], cwd)
  const handshake = readHandshake(lintRun.stdout, lintRun.stderr)
  if (!handshake.ok) return { ok: false, handshake }

  const lintReport = JSON.parse(lintRun.stdout) as LintReport
  const mappedLint = fromLint(lintReport, opts)

  // Lint first, then graph-check against what lint already covered. The order is the
  // whole point: lint carries baseline membership and spans, so where both object to the
  // same node, lint's is the strictly more informative mark.
  const covered = new Set(mappedLint.findings.map((f) => f.file))
  const graphReport = await fetchReport<GraphCheckReport>(bin, ['graph-check'], cwd, run)

  const mappedGraph: Mapped = graphReport
    ? fromGraphCheck(graphReport, covered)
    : { findings: [], conditions: [] }

  return {
    ok: true,
    gatePassed: lintReport.gate.passed,
    lint: lintReport,
    graph: graphReport,
    mapped: {
      findings: [...mappedLint.findings, ...mappedGraph.findings],
      conditions: [...mappedLint.conditions, ...mappedGraph.conditions],
    },
  }
}

/** What a save can change. */
export interface CorpusViews {
  status: StatusReport | null
  corpusIndex: CorpusIndexReport | null
  openQuestions: OpenQuestionsReport | null
  indexStatus: IndexStatusReport | null
}

export async function runCorpusViews(bin: string, cwd: string, run: Spawn): Promise<CorpusViews> {
  const [status, corpusIndex, openQuestions, indexStatus] = await Promise.all([
    fetchReport<StatusReport>(bin, ['status'], cwd, run),
    fetchReport<CorpusIndexReport>(bin, ['corpus-index'], cwd, run),
    fetchReport<OpenQuestionsReport>(bin, ['open-questions'], cwd, run),
    fetchReport<IndexStatusReport>(bin, ['index-status'], cwd, run),
  ])
  return { status, corpusIndex, openQuestions, indexStatus }
}

/** What a ref can change. */
export interface RefViews {
  phases: PhasesReport | null
  sangha: SanghaReport | null
}

export async function runRefViews(bin: string, cwd: string, run: Spawn): Promise<RefViews> {
  const [phases, sangha] = await Promise.all([
    fetchReport<PhasesReport>(bin, ['phases'], cwd, run),
    fetchReport<SanghaReport>(bin, ['sangha'], cwd, run),
  ])
  return { phases, sangha }
}
