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

console.log('a section nothing measured says so');
{
  const t = text(trends);
  check('the bench section declares itself unmeasured', t.includes('not measured'));
  check('and says why', t.includes('#468'));
  check('no chart was drawn from nothing', !trends.includes('<svg'));
}

// ── with a report that measured nothing ──────────────────────────────────────
//
// Derived from the golden rather than written beside it: a second hand-authored report would
// be a second opinion about the contract and the first thing to drift. What it changes is the
// one field — a change that touched no Rust under the measured source root, which is what the
// first real CI run of this phase produced, its own diff being tests and workflows.

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
