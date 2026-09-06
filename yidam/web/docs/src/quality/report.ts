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
  quality: {
    gates: Gate[];
    sections: Record<string, Section>;
    /** What the run's jobs concluded (#516). Absent in reports written before it existed. */
    run?: RunJobs | null;
  };
}

/** The jobs of one CI run, as they stood when the report was assembled. */
export interface RunJobs {
  jobs: Job[];
  /** Jobs that had not finished — the reporting job itself, and anything after it. */
  pending: string[];
}

export interface Job {
  name: string;
  /** GitHub's own word: `success`, `failure`, `cancelled`, `skipped`, … */
  conclusion: string;
}

export interface Gate {
  gate: string;
  features: string[];
  totals: Totals;
  suites: Suite[];
  skipped: SkipRecord[];
  coverage: Coverage | null;
  /** The job this gate ran in, when it is not the gate's own name. */
  job?: string | null;
  /**
   * What that job concluded. `undefined`/`null` means nobody could say — a report merged
   * without a job list, or one written before #516 — and that is drawn as its own state.
   * It is never drawn as a pass: a gate whose tests all passed and whose job failed is the
   * case this field exists for.
   */
  conclusion?: string | null;
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

/**
 * Conclusions that mean "nothing to tell the reader".
 *
 * An allow-list, matching `RunJobs::unsuccessful` in the reporter. GitHub has added
 * conclusions before — `timed_out`, `stale`, `action_required` — and a deny-list of
 * `failure` would quietly call each new one fine, which is the shape of defect this whole
 * surface exists to remove.
 */
const BENIGN = new Set(['success', 'skipped', 'neutral']);

/** Every job of the run that did not succeed, and an empty list when nobody could say. */
export function unsuccessfulJobs(report: QualityReport): Job[] {
  return (report.quality.run?.jobs ?? []).filter((j) => !BENIGN.has(j.conclusion));
}

/** How a gate's own job ended, in the three states a page must tell apart. */
export type GateOutcome = 'ok' | 'bad' | 'unknown';

export function gateOutcome(gate: Gate): GateOutcome {
  if (gate.conclusion === undefined || gate.conclusion === null) return 'unknown';
  return BENIGN.has(gate.conclusion) ? 'ok' : 'bad';
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

// ── the series (#468) ────────────────────────────────────────────────────────
//
// One record per push to main, on the `quality-series` orphan branch, fetched by the docs
// workflow. `quality-report.json` describes one commit; this is the sequence.
//
// It is not a second source of truth. Nothing here recomputes anything a gate measured, and
// where a record disagrees with the report on the same page, the report is the one that came
// from the run being described.

export interface SeriesRecord {
  commit: string;
  recorded_at: number;
  gates: number;
  totals: Totals;
  test_seconds: number;
  coverage: { added: number; uncovered: number; features: string[] } | null;
  bench: {
    nodes: number;
    focused_tokens: number;
    full_scan_tokens: number;
    focused_precision: number;
  } | null;
  /**
   * Jobs of that run which did not succeed (#516). `undefined`/`null` for every record
   * written before the field existed, and for a run whose merge could not reach the API —
   * which is not the same as an empty list, and is not drawn as one.
   */
  unsuccessful_jobs?: string[] | null;
}

export interface SeriesLoad {
  records: SeriesRecord[];
  /** 1-indexed line numbers that did not parse. */
  unreadable: number[];
  /** Why there is no series at all, when there is none. */
  problem: string | null;
}

/**
 * The series, or the reason there isn't one.
 *
 * A line that does not parse is skipped and counted, never fatal. The file is append-only and
 * written by a job that can be cancelled mid-push; one truncated write must not blank a year
 * of history, and a parser that refused the whole file would turn a single bad append into a
 * page with nothing on it.
 */
export function loadSeries(): SeriesLoad {
  const path = process.env.YIDAM_QUALITY_SERIES;
  if (!path) {
    return {
      records: [],
      unreadable: [],
      problem:
        'No series was available to this build. YIDAM_QUALITY_SERIES is unset — the docs ' +
        'workflow fetches it from the `quality-series` branch, which does not exist until the ' +
        'first push to main after #468.',
    };
  }

  let raw: string;
  try {
    raw = readFileSync(path, 'utf8');
  } catch (e) {
    return { records: [], unreadable: [], problem: `${path} could not be read: ${e}` };
  }

  const records: SeriesRecord[] = [];
  const unreadable: number[] = [];
  raw.split('\n').forEach((line, i) => {
    if (!line.trim()) return;
    try {
      records.push(JSON.parse(line));
    } catch {
      unreadable.push(i + 1);
    }
  });

  return {
    records,
    unreadable,
    problem: records.length === 0 ? `${path} holds no readable records.` : null,
  };
}

/**
 * A unix instant as `2026-09-02 00:25 UTC`.
 *
 * Built from the `getUTC*` accessors rather than the locale ones, and with the zone in the
 * string. A build machine's clock is UTC, a reader's is not, and a timestamp rendered at
 * build time in the builder's zone is a number two people read as two different moments.
 */
export function utcMinute(unixSeconds: number | null | undefined): string | null {
  if (typeof unixSeconds !== 'number' || !Number.isFinite(unixSeconds)) return null;
  const d = new Date(unixSeconds * 1000);
  if (Number.isNaN(d.getTime())) return null;
  const pad = (n: number) => `${n}`.padStart(2, '0');
  return (
    `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ` +
    `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())} UTC`
  );
}

/** Which moment the series describes, and whether it is the masthead's. */
export interface SeriesProvenance {
  /** The newest record's commit, or null when the build read no records. */
  commit: string | null;
  /** When that record was written, or null when it carries no usable timestamp. */
  recordedAt: string | null;
  /** True only when both commits are known and they differ. */
  disagrees: boolean;
}

/**
 * When the build read the series, and whether that is the moment the masthead names.
 *
 * The two halves of the trends page come from two places with two lags. The report is
 * downloaded from the last CI run on main that *succeeded*, so a stretch of red leaves it
 * several commits back — main-only jobs were red across four merges once, which is not a
 * hypothetical shape. The series is fetched from the `quality-series` branch at build time,
 * so it is behind by however far `ci.yml`'s parallel `series` job has got.
 *
 * The report's lag has always been disclosed: that is what `yidam.commit` and the masthead's
 * `measured at` line are for. The series had no such line, and a masthead describing the
 * other half of the page is not one. This is that line.
 *
 * The last record in file order is the newest: `series::append` drops any record for the same
 * commit and pushes the new one at the end, so position is write order.
 */
export function seriesProvenance(
  records: SeriesRecord[],
  reportCommit: string | null | undefined,
): SeriesProvenance {
  const newest = records.length > 0 ? records[records.length - 1] : null;
  const commit = newest?.commit ?? null;
  return {
    commit,
    recordedAt: utcMinute(newest?.recorded_at),
    // A plain comparison, because both sides are the same field: `series::record` copies
    // `yidam.commit` off a report. What differs is *which* report, which is the whole point.
    disagrees: Boolean(commit && reportCommit && commit !== reportCommit),
  };
}

/** One metric down the series, oldest first — the order the file is appended in. */
export function column(records: SeriesRecord[], pick: (r: SeriesRecord) => number | null): number[] {
  return records.map(pick).filter((v): v is number => v !== null && Number.isFinite(v));
}
