// The quality pages, built and read back.
//
// Everything else about this phase is asserted on the model: `quality.rs` grades the totals,
// `quality_report.rs` grades the document. Neither of them can catch a template that receives
// `asserted: 0` and draws a green bar anyway, and RFC-0025 names that as the assertion this
// phase is most likely to lose:
//
//   "A fully-skipped suite must not render as a passing one. P6's pages take a run with a
//    failing suite, an empty suite, and a fully-skipped suite as three distinct fixtures. The
//    last is easy to lose in a template, and losing it discards the entire argument of P1's
//    skip census."
//
// So this builds the site — twice, once with a report and once without — and reads the HTML.
// It found two bugs the first time it ran: the overview page said "3 ignored test s", and the
// no-report build had never been rendered at all.
//
// The report it builds against is `ci-report`'s committed golden, not a fixture of its own. A
// second fixture would be a second opinion about the contract, and the first thing to drift.

import { execFileSync } from 'node:child_process';
import { readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const site = resolve(here, '..');
const golden = resolve(
  site,
  '../../tests/harness/ci-report/tests/goldens/quality-report.json',
);

let failures = 0;

function check(name, condition, detail = '') {
  if (condition) return;
  failures++;
  console.error(`  ✗ ${name}${detail ? `\n      ${detail}` : ''}`);
}

/** Build into a directory of its own, so a fixture render can never become the deploy. */
function build(outDir, env) {
  rmSync(join(site, outDir), { recursive: true, force: true });
  execFileSync('npx', ['astro', 'build', '--outDir', outDir], {
    cwd: site,
    env: { ...process.env, ...env },
    stdio: 'pipe',
  });
}

function page(outDir, route) {
  return readFileSync(join(site, outDir, route, 'index.html'), 'utf8');
}

/** The markup from a heading to the next one, so an assertion is about one suite. */
function section(html, heading) {
  const at = html.indexOf(`>${heading}<`);
  if (at < 0) return '';
  const next = html.indexOf('<section', at + 1);
  return html.slice(at, next < 0 ? html.length : next);
}

/** Tags stripped, entities left alone — enough to assert on prose without matching markup. */
function text(html) {
  return html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ');
}

/**
 * The same, but a tag contributes no space.
 *
 * For punctuation only. `text()` turns `<code>main</code>.` into `main .` and would report a
 * stray space that a reader never sees — which it did, on the footer, the first time the
 * spacing check ran. What that check is about is a space the *renderer* inserted between two
 * adjacent JSX expressions, and those are text nodes rather than elements.
 */
function inline(html) {
  return html.replace(/<[^>]+>/g, '').replace(/\s+/g, ' ');
}

// ── with a report ────────────────────────────────────────────────────────────

console.log('building the quality pages against the committed golden…');
build('dist-quality-test', { YIDAM_QUALITY_REPORT: golden });

const overview = page('dist-quality-test', 'quality');
const tests = page('dist-quality-test', 'quality/tests');
const coverage = page('dist-quality-test', 'quality/coverage');
const trends = page('dist-quality-test', 'quality/trends');

console.log('a fully-skipped suite does not render as a passing one');
{
  const skipped = section(tests, 'yidam::vault_s3');
  check('the fully-skipped suite is on the page', skipped.length > 0);
  check(
    'it says it asserted nothing',
    text(skipped).includes('Ran and asserted nothing'),
    text(skipped).slice(0, 300),
  );
  check(
    'its meter draws no pass segment',
    !skipped.includes('--run-passed-fill'),
    'the suite passed to a runner and asserted nothing; a green segment is the claim this ' +
      'phase exists to stop the page from making',
  );
  check(
    'the skip reason survives to the page',
    text(skipped).includes('YIDAM_S3_TEST'),
    'a skip nobody can act on is a skip nobody counted',
  );
}

console.log('a failing suite is legible without opening a log');
{
  const failing = section(tests, 'yidam::design_tokens');
  check('the failing suite is on the page', failing.length > 0);
  check('its meter draws a pass segment', failing.includes('--run-passed-fill'));
  check('its meter draws a fail segment', failing.includes('--run-failed-fill'));
  check(
    'the failure output is on the page',
    text(failing).includes('reference tokens the design system does not declare'),
  );
}

console.log('a suite with no cases is on the page at all');
{
  const ignored = section(tests, 'yidam::example_corpus');
  check(
    'the ignored-only suite is on the page',
    ignored.length > 0,
    'it exists only in `nextest list`; a page built on the JUnit alone would not have it',
  );
  check('it draws no pass segment', !ignored.includes('--run-passed-fill'));
}

console.log('the overview counts what ran, not what a runner called a pass');
{
  const t = text(overview);
  check('the measured commit is stated', t.includes('measured at'));
  check('the gates are named', t.includes('ci (cli)') && t.includes('ci (harness)'));
  check(
    'plurals are not split across text nodes',
    !/\b(test|case|gate|skip) s\b/.test(t),
    'an adjacent JSX expression put a space before the "s"',
  );
  check(
    'no stray space before a full stop',
    !/ \./.test(inline(overview)),
    'two adjacent JSX expressions are two text nodes, and the renderer separates them',
  );
}

console.log('unmeasured is not uncovered');
{
  const t = text(coverage);
  check('the coverage percentage excludes the gated file', t.includes('50%'), t.slice(0, 400));
  check('the gated file is listed as unmeasured', t.includes('embedding.rs'));
  check('and says which feature gates it', t.includes('gated behind index'));
  check(
    'the gated file is not listed as uncovered lines',
    !text(section(coverage, 'Added and not executed')).includes('embedding.rs'),
  );
  check('the feature set the number is about is stated', t.includes('reports, tonpa, vault-s3'));
}

console.log('a section nothing measured says so, and one that is measured does not');
{
  const t = text(trends);
  // Named sections, not a substring of the page. This check used to be `includes('not
  // measured')` against the whole document, and when #468 made `bench` measured it went on
  // passing — matching the *mutation* section instead. A guard that can be satisfied by a
  // different section than the one it names is the prose-answers-for-code shape, in a test.
  check(
    'the mutation section declares itself unmeasured',
    t.includes('Mutation survivors — not measured'),
    t.slice(0, 500),
  );
  check(
    'and says where its numbers are instead',
    t.includes('weekly schedule'),
    'an unmeasured section that does not say why reads as an empty one',
  );
  check(
    'bench is not rendered as unmeasured',
    !t.includes('bench series — not measured'),
    'the bench section is measured now; rendering it as absent would send a reader looking ' +
      'for a ratchet that exists',
  );
  check(
    'no chart was drawn from a series that is not there',
    !trends.includes('<path'),
    'this build passed no series, so any line on the page came from nothing',
  );
}

// ── with a report that measured nothing ──────────────────────────────────────
//
// Derived from the golden rather than written beside it: a second hand-authored report would
// be a second opinion about the contract and the first thing to drift. What it changes is the
// one field — a change that touched no Rust under the measured source root, which is what the
// first real CI run of this phase produced, its own diff being tests and workflows.

console.log('a job that failed is not hidden by its tests passing');
{
  // The golden's `ci (harness)` gate passed every test it ran and its job concluded
  // `failure` — a lint, a coverage step, a packaging check, any of the things a job does
  // that JUnit never sees. Before #516 this page said "0 failed" about that run, and the
  // reader was shown a clean bill of health for a red main.
  const overview = text(page('dist-quality-test', 'quality'));
  check(
    'the overview names the jobs that did not succeed',
    overview.includes('ci (cli · full features)') && overview.includes('did not succeed'),
    overview.slice(0, 400),
  );
  check(
    'including a job that produced no gate at all',
    overview.includes('ci (cli · full features)'),
    'a job that failed before writing a fragment is absent from `gates`, and absent reads ' +
      'exactly like never configured',
  );
  check(
    'a job still running is named rather than judged',
    overview.includes('ci (quality report)') && overview.includes('Still running'),
    overview.slice(0, 600),
  );

  const tests = page('dist-quality-test', 'quality/tests');
  const harness = text(section(tests, 'ci (harness)'));
  check(
    'a gate whose tests all passed says so when its job did not',
    harness.includes('failure'),
    `the tests page renders ci (harness) without its job's verdict:\n      ${harness.slice(0, 300)}`,
  );
  check(
    'and it is not drawn as a clean bill of health',
    !/Every suite here failed nothing and asserted something/.test(harness) ||
      harness.includes('failure'),
    'the gate reads "every suite passed" with nothing to say the job did not',
  );
}

console.log('a report with no job list says so, rather than showing green…');
{
  // The pre-#516 shape, and the shape of any report merged without the run's job list.
  // `undefined` must render as its own state: treating "nobody asked" as "nothing failed"
  // is the defect, and it is one `?.` away from coming back.
  const report = JSON.parse(readFileSync(golden, 'utf8'));
  delete report.quality.run;
  for (const gate of report.quality.gates) delete gate.conclusion;
  const path = join(site, 'quality.no-jobs.json');
  writeFileSync(path, JSON.stringify(report));

  build('dist-quality-nojobs', { YIDAM_QUALITY_REPORT: path });
  const t = text(page('dist-quality-nojobs', 'quality'));
  check(
    'a report from before the conclusions existed still renders',
    t.includes('Every gate'),
    t.slice(0, 300),
  );
  check(
    'and it states that it cannot speak for the jobs',
    t.includes('No job outcomes'),
    'an absent job list rendered silently, which is indistinguishable from all-green',
  );
  check(
    'no gate claims an outcome nobody reported',
    t.includes('job outcome unknown'),
    t.slice(0, 600),
  );
  rmSync(path, { force: true });
  rmSync(join(site, 'dist-quality-nojobs'), { recursive: true, force: true });
}

console.log('a change with no measured lines says so, rather than claiming coverage…');
{
  const report = JSON.parse(readFileSync(golden, 'utf8'));
  for (const gate of report.quality.gates) {
    if (!gate.coverage) continue;
    gate.coverage.added = 0;
    gate.coverage.uncovered = 0;
    gate.coverage.files = [];
    gate.coverage.unmeasured = [];
  }
  const path = join(site, 'quality-report.nothing-measured.json');
  writeFileSync(path, JSON.stringify(report));
  build('dist-quality-none', { YIDAM_QUALITY_REPORT: path });
  const t = text(page('dist-quality-none', 'quality/coverage'));
  check('it says no lines were added', t.includes('added no Rust lines'), t.slice(0, 400));
  check(
    'it does not claim every line was executed',
    !t.includes('was executed by a test'),
    'zero lines covered out of zero is not a hundred percent, and it is not a pass',
  );
  rmSync(path, { force: true });
  rmSync(join(site, 'dist-quality-none'), { recursive: true, force: true });
}

// ── the series ───────────────────────────────────────────────────────────────
//
// #468's assertions, and the second one is the one the issue names: a malformed record must
// not blank the history around it. The series is append-only and written by a job that can be
// cancelled mid-push, so one truncated line is a thing that will happen.

console.log('the series draws its shapes, and a bad line does not take the good ones…');
{
  const record = (commit, asserted, seconds, tokens, unsuccessful_jobs, recorded_at = 1788000000) =>
    JSON.stringify({
      commit,
      recorded_at,
      gates: 4,
      totals: { cases: asserted, failed: 0, passed: asserted, skipped: 3, gated: 0, ignored: 3, asserted },
      test_seconds: seconds,
      coverage: { added: 0, uncovered: 0, features: ['reports'] },
      bench: { nodes: 4096, focused_tokens: tokens, full_scan_tokens: 6041600, focused_precision: 0.013 },
      ...(unsuccessful_jobs ? { unsuccessful_jobs } : {}),
    });
  const path = join(site, 'series.test.jsonl');
  writeFileSync(
    path,
    [
      record('aaa1111', 1640, 68.2, 392000),
      '{ truncated',
      // Zero failed tests and a red run — the shape #516 is about, in the series.
      // A later instant than the first record, so "the newest" is a claim that can be wrong.
      record('bbb2222', 1699, 74.9, 378521, ['ci (cli · full features)'], 1788304687),
      '',
    ].join('\n'),
  );
  build('dist-quality-series', { YIDAM_QUALITY_REPORT: golden, YIDAM_QUALITY_SERIES: path });
  const html = page('dist-quality-series', 'quality/trends');
  const t = text(html);

  check('the records that parsed are counted', t.includes('2 records'), t.slice(0, 400));
  check('a line that did not parse is reported', t.includes('could not be read'), t.slice(0, 400));
  check(
    'and the rest is still drawn',
    (html.match(/<path/g) || []).length >= 3,
    'a bad line blanked the history around it',
  );
  check(
    'a rising cost is drawn as a regression',
    html.includes('--run-failed-fill'),
    'test seconds went up across the series and nothing said so',
  );
  check(
    'a falling cost is not',
    html.includes('--run-passed-fill'),
    'bench tokens went down across the series and it was drawn as a regression',
  );
  check(
    'a commit whose run failed is named, though no test failed',
    t.includes('bbb2222') && t.includes('did not pass CI'),
    `every record has failed: 0 and one of them was red:\n      ${t.slice(0, 500)}`,
  );
  check(
    'and the job that failed is named with it',
    t.includes('ci (cli · full features)'),
    'the series says a run failed and not which part of it did',
  );

  // #526. The count used to stand alone — "N records, one per push to main" — which is a
  // claim about the branch made by a page holding a snapshot of it. The masthead states the
  // *report's* commit, and the report and the series are two fetches with two lags.
  check(
    'the series names the newest record it actually read',
    t.includes('up to') && t.includes('bbb2222') && t.includes('recorded 2026-09-01 23:18 UTC'),
    `the count is stated with no moment attached, or with the first record's:\n      ${t.slice(0, 500)}`,
  );
  check(
    'and not the oldest',
    !t.includes('recorded 2026-08-29'),
    'the page read records[0] where it meant the last one',
  );
  check(
    'and says the branch may have moved since',
    t.includes('may have grown since this build read it'),
    'a snapshot is presented as the whole history',
  );
  check(
    'a series describing a different commit from the report says so',
    t.includes('two moments, fetched from two places'),
    `the report is at <commit> and the series ends at bbb2222:\n      ${t.slice(0, 500)}`,
  );
  // The defect this line was written with: `{expr}` on one line and `{expr}` on the next are
  // two children with a text node of whitespace between them, and the page read
  // "oldest first , up to bbb2222 , recorded".
  // Scoped to the card, not the document: the inlined stylesheet holds `} .damaged`, and a
  // whole-page scan for " ." reports the CSS on every run.
  const card = (() => {
    const flat = inline(html);
    const from = flat.indexOf('The series');
    const to = flat.indexOf('Tests asserting', from);
    return flat.slice(from, to < 0 ? from + 600 : to);
  })();
  check(
    'and the punctuation around it is not spaced off the words',
    !card.includes(' ,') && !card.includes(' .'),
    `a space the renderer inserted between two adjacent expressions:\n      ${card}`,
  );
  rmSync(path, { force: true });
  rmSync(join(site, 'dist-quality-series'), { recursive: true, force: true });
}

// The notice has to be able to *not* fire. A sentence on every build is a sentence nobody
// reads, and one that cannot be absent is one no assertion above is really testing.
console.log('a series that agrees with the report says nothing about disagreeing…');
{
  const golden_commit = JSON.parse(readFileSync(golden, 'utf8')).yidam.commit;
  const path = join(site, 'series.agree.jsonl');
  const rec = (commit, at) =>
    JSON.stringify({
      commit,
      recorded_at: at,
      gates: 1,
      totals: { cases: 10, failed: 0, passed: 10, skipped: 0, gated: 0, ignored: 0, asserted: 10 },
      test_seconds: 1,
      coverage: null,
      bench: null,
    });
  writeFileSync(path, [rec('aaa1111', 1788000000), rec(golden_commit, 1788308751)].join('\n') + '\n');

  // Twice, in two zones. `recorded_at` is a unix instant and the build machine's clock is
  // UTC; a page that rendered it in the builder's local zone would be a number two readers
  // read as two different moments, and every check on it would still pass on the runner.
  const rendered = ['UTC', 'Asia/Tokyo'].map((TZ) => {
    build(`dist-quality-tz-${TZ.replace('/', '-')}`, {
      YIDAM_QUALITY_REPORT: golden,
      YIDAM_QUALITY_SERIES: path,
      TZ,
    });
    return text(page(`dist-quality-tz-${TZ.replace('/', '-')}`, 'quality/trends'));
  });

  check(
    'a series ending at the report\'s own commit raises nothing',
    !rendered[0].includes('two moments, fetched from two places'),
    `the notice fired on a build where both halves name ${golden_commit}`,
  );
  check(
    'and the record it read is still named',
    rendered[0].includes('recorded 2026-09-02 00:25 UTC'),
    rendered[0].slice(0, 400),
  );
  check(
    'the timestamp does not move with the builder\'s time zone',
    rendered[0] === rendered[1],
    'built under TZ=UTC and TZ=Asia/Tokyo and the pages differ',
  );

  rmSync(path, { force: true });
  for (const TZ of ['UTC', 'Asia-Tokyo']) {
    rmSync(join(site, `dist-quality-tz-${TZ}`), { recursive: true, force: true });
  }
}

console.log('one record is not a trend…');
{
  const path = join(site, 'series.one.jsonl');
  writeFileSync(
    path,
    JSON.stringify({
      commit: 'aaa1111',
      recorded_at: 1788000000,
      gates: 1,
      totals: { cases: 10, failed: 0, passed: 10, skipped: 0, gated: 0, ignored: 0, asserted: 10 },
      test_seconds: 1,
      coverage: null,
      bench: null,
    }) + '\n',
  );
  build('dist-quality-one', { YIDAM_QUALITY_REPORT: golden, YIDAM_QUALITY_SERIES: path });
  const html = page('dist-quality-one', 'quality/trends');
  check(
    'a single record says so rather than drawing a flat line',
    text(html).includes('A trend needs two'),
    text(html).slice(0, 400),
  );
  check('and draws no line at all', !html.includes('<path'), 'one point was drawn as a series');
  rmSync(path, { force: true });
  rmSync(join(site, 'dist-quality-one'), { recursive: true, force: true });
}

// ── with no report ───────────────────────────────────────────────────────────

console.log('building with no report at all…');
build('dist-quality-empty', { YIDAM_QUALITY_REPORT: '' });

for (const route of ['quality', 'quality/tests', 'quality/coverage', 'quality/trends']) {
  const t = text(page('dist-quality-empty', route));
  check(`${route} says it is not measured`, t.includes('Not measured'));
  check(
    `${route} draws no zero`,
    !/\b0 asserted\b/.test(t) && !/\b0%\b/.test(t),
    'an absent measurement rendered as a zero one — the failure this phase is about',
  );
}

rmSync(join(site, 'dist-quality-test'), { recursive: true, force: true });
rmSync(join(site, 'dist-quality-empty'), { recursive: true, force: true });

if (failures > 0) {
  console.error(`\n${failures} render assertion(s) failed.`);
  process.exit(1);
}
console.log('\nall render assertions hold.');
