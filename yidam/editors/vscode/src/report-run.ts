/**
 * One pass of the reports over a workspace: run both, map both, merge.
 *
 * Kept out of `extension.ts` so the ordering rule — lint owns the diagnostics, graph-check
 * fills the gap — is exercised without an editor. It is the one piece of composition in
 * this feature that could be quietly wrong.
 */

import { fromGraphCheck, fromLint, type Mapped, type Options } from './diagnostics.ts'
import { readHandshake, type Handshake } from './handshake.ts'
import type { GraphCheckReport, LintReport } from './reports.ts'
import type { Spawn } from './runner.ts'

export type Outcome =
  | { ok: true; mapped: Mapped; gatePassed: boolean }
  | { ok: false; handshake: Handshake }

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
  const graphRun = await run(bin, ['graph-check', '--format', 'json'], cwd)
  const graphHandshake = readHandshake(graphRun.stdout, graphRun.stderr)

  let mappedGraph: Mapped = { findings: [], conditions: [] }
  if (graphHandshake.ok) {
    mappedGraph = fromGraphCheck(JSON.parse(graphRun.stdout) as GraphCheckReport, covered)
  }

  return {
    ok: true,
    gatePassed: lintReport.gate.passed,
    mapped: {
      findings: [...mappedLint.findings, ...mappedGraph.findings],
      conditions: [...mappedLint.conditions, ...mappedGraph.conditions],
    },
  }
}
