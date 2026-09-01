// Reading `quality-report.json` at build time.
//
// The measurements are produced by `ci.yml` and this site is built by `docs.yml`; the two
// are separate workflows that start at the same moment, so the report a build can see is the
// one the previous CI run left behind. That is not a defect to hide — it is why the envelope
// carries `yidam.commit`, and why every page here states which commit it is describing.
//
// There is no committed fallback. A page that renders a stale fixture when the fetch failed
// is a page that looks measured and is not, which is the failure this whole epic is about.
// Absent means absent, and the pages say so.

import { readFileSync } from 'node:fs';

export interface QualityReport {
  format_version: string;
  yidam: { version: string; commit: string; features: string[] };
  root: string;
  quality: { gates: Gate[]; sections: Record<string, Section> };
}

export interface Gate {
  gate: string;
  features: string[];
  totals: Totals;
  suites: Suite[];
  skipped: SkipRecord[];
  coverage: Coverage | null;
}

export interface Totals {
  cases: number;
  failed: number;
  /** Includes every gated skip: a runner records one as a pass. Use `asserted` to draw. */
  passed: number;
  skipped: number;
  gated: number;
  ignored: number;
  /** `passed` minus the skips among it — what actually exercised anything. */
  asserted: number;
}

export interface Suite {
  suite: string;
  totals: Totals;
  tests: Test[];
}

export interface Test {
  name: string;
  status: 'passed' | 'failed' | 'skipped';
  seconds: number | null;
  failure: string | null;
  skip_reason: string | null;
}

export interface SkipRecord {
  suite: string;
  test: string;
  reason: string;
  kind: string;
}

export interface Coverage {
  features: string[];
  added: number;
  uncovered: number;
  files: { path: string; added: number; uncovered: number[] }[];
  unmeasured: { path: string; added: number; reason: string; feature: string | null }[];
}

export interface Section {
  measured: boolean;
  why: string;
}

/** The contract version this site knows how to read. */
export const KNOWN_FORMAT_VERSION = '1';

export type Load =
  | { report: QualityReport; problem: null }
  | { report: null; problem: string };

/**
 * The report, or the reason there isn't one.
 *
 * A version this site does not know is refused rather than rendered. RFC-0016 makes the rule
 * absolute for the report contract: a consumer reading an unknown major version must say so
 * and disable verdict features, and must never mis-parse. Drawing a bar out of a document
 * whose fields may have changed meaning is exactly mis-parsing.
 */
export function loadReport(): Load {
  const path = process.env.YIDAM_QUALITY_REPORT;
  if (!path) {
    return {
      report: null,
      problem:
        'No quality report was available to this build. YIDAM_QUALITY_REPORT is unset — on a ' +
        'pull request that is expected, and each gate posts its own numbers to its job summary.',
    };
  }

  let raw: string;
  try {
    raw = readFileSync(path, 'utf8');
  } catch (e) {
    return { report: null, problem: `YIDAM_QUALITY_REPORT points at ${path}, which could not be read: ${e}` };
  }

  let parsed: QualityReport;
  try {
    parsed = JSON.parse(raw);
  } catch (e) {
    return { report: null, problem: `${path} is not JSON: ${e}` };
  }

  if (parsed.format_version !== KNOWN_FORMAT_VERSION) {
    return {
      report: null,
      problem:
        `${path} declares format_version ${JSON.stringify(parsed.format_version)}; this site ` +
        `reads ${KNOWN_FORMAT_VERSION}. Refusing to render rather than guess at fields whose ` +
        `meaning may have changed.`,
    };
  }
  if (!parsed.quality?.gates?.length) {
    return { report: null, problem: `${path} carries no gates, so there is nothing measured to show.` };
  }

  return { report: parsed, problem: null };
}

/** Every gate's numbers, added up. */
export function overall(report: QualityReport): Totals {
  return report.quality.gates
    .map((g) => g.totals)
    .reduce(
      (a, b) => ({
        cases: a.cases + b.cases,
        failed: a.failed + b.failed,
        passed: a.passed + b.passed,
        skipped: a.skipped + b.skipped,
        gated: a.gated + b.gated,
        ignored: a.ignored + b.ignored,
        asserted: a.asserted + b.asserted,
      }),
      { cases: 0, failed: 0, passed: 0, skipped: 0, gated: 0, ignored: 0, asserted: 0 },
    );
}

/**
 * Suites worth expanding: anything that failed, and anything that ran without asserting.
 *
 * The second half is the point. A page listing 1,369 green rows is a page nobody reads, and
 * scrolling past them is how the four suites that asserted nothing stayed invisible in the
 * first place. These are the ones that have something to say.
 */
export function notable(gate: Gate): Suite[] {
  return gate.suites.filter((s) => s.totals.failed > 0 || s.totals.asserted === 0);
}

/**
 * `2 gates`, `1 gate`.
 *
 * A helper rather than a ternary in the markup, because the markup is JSX: two adjacent
 * expressions are two text nodes, and the renderer puts a space between them. The first
 * draft of the overview page read "3 ignored test s" and "across 1 gate ." — visible only by
 * looking at the built page, which is the argument for the render assertions in
 * `test/quality-render.mjs`.
 */
export function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? '' : 's'}`;
}

/** `11, 12, 13, 20` → `11–13, 20`. A list of forty consecutive line numbers is not read. */
export function joinRanges(lines: number[]): string {
  const out: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const start = lines[i];
    let end = start;
    while (i + 1 < lines.length && lines[i + 1] === end + 1) end = lines[++i];
    out.push(start === end ? `${start}` : `${start}–${end}`);
  }
  return out.join(', ');
}
