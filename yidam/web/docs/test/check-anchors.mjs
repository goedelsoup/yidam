// The link and anchor checker, including the mistake it made on its first run in CI.
//
// A fixture tree rather than an Astro build: the shapes that matter are a live link, a dead
// one, a live fragment, a dead one, and a framework fragment — and a build produces those
// only by accident, if at all.

import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const script = join(here, '../scripts/check-anchors.mjs');

let failures = 0;
function check(name, condition, detail = '') {
  if (condition) return;
  failures++;
  console.error(`  ✗ ${name}${detail ? `\n      ${detail}` : ''}`);
}

/**
 * Run the checker, returning its exit code and everything it said.
 *
 * `spawnSync` rather than `execFileSync`: the checker writes its summary to stderr so a
 * caller can pipe its findings, and `execFileSync` hands back only stdout on success — which
 * made the first version of this file assert against an empty string and pass for it.
 */
function run(dir, ...args) {
  const r = spawnSync('node', [script, dir, ...args], { encoding: 'utf8' });
  return { code: r.status, out: `${r.stdout || ''}${r.stderr || ''}` };
}

function tree(base, { deadLink = false, deadAnchor = false } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'yidam-anchors-'));
  const write = (p, body) => {
    mkdirSync(dirname(join(dir, p)), { recursive: true });
    writeFileSync(join(dir, p), body);
  };
  write(
    'a/index.html',
    `<!doctype html><html><body>
      <h2 id="a-heading">A</h2>
      <a href="${base}/b/">to b</a>
      <a href="${base}/b/#b-heading">to b's heading</a>
      <a href="#a-heading">to my own</a>
      <a href="#_top">framework</a>
      <a href="https://example.com/#nope">external</a>
      ${deadLink ? `<a href="${base}/gone/">gone</a>` : ''}
      ${deadAnchor ? `<a href="${base}/b/#not-a-heading">missing</a>` : ''}
     </body></html>`,
  );
  write('b/index.html', '<!doctype html><html><body><h2 id="b-heading">B</h2></body></html>');
  write('sitemap.xml', '<?xml version="1.0"?><urlset/>');
  return dir;
}

console.log('a clean tree passes, at whatever base it was built for…');
{
  for (const base of ['/yidam', '/yidam/main', '/yidam/v0.7']) {
    const dir = tree(base);
    const r = run(dir, '--base', base);
    check(`clean at ${base}`, r.code === 0, r.out);
    check(`and says so`, r.out.includes('0 dead link(s), 0 dead anchor(s)'), r.out);
    rmSync(dir, { recursive: true, force: true });
  }
}

console.log('the base is an argument, and getting it wrong is loud rather than silent…');
{
  // The CI failure this test exists for. `main` builds at `/yidam/main`; the checker's
  // default is the site root. Every link on every page then fails to resolve, and the gate
  // reports thousands of dead links on a tree with none.
  const dir = tree('/yidam/main');
  const wrong = run(dir, '--base', '/yidam');
  check('a mismatched base is reported, not passed over', wrong.code !== 0, wrong.out);
  const right = run(dir, '--base', '/yidam/main');
  check('and the same tree is clean at its own base', right.code === 0, right.out);
  rmSync(dir, { recursive: true, force: true });
}

console.log('a real dead link and a real dead anchor are both caught…');
{
  const dir = tree('/yidam', { deadLink: true });
  const r = run(dir, '--base', '/yidam');
  check('a link to a page that does not exist fails', r.code !== 0);
  check('and names it', r.out.includes('/yidam/gone/'), r.out);
  rmSync(dir, { recursive: true, force: true });

  const dir2 = tree('/yidam', { deadAnchor: true });
  const r2 = run(dir2, '--base', '/yidam');
  check('a fragment no heading matches fails', r2.code !== 0);
  check('and names it', r2.out.includes('#not-a-heading'), r2.out);
  check(
    'while the page it points at is not called dead',
    r2.out.includes('0 dead link(s)'),
    r2.out,
  );
  rmSync(dir2, { recursive: true, force: true });
}

console.log("and the framework's own fragment is not this repository's problem…");
{
  const dir = tree('/yidam');
  const r = run(dir, '--base', '/yidam');
  check(
    '#_top is excluded by name',
    r.code === 0 && !r.out.includes('_top'),
    'Starlight emits it on every page and no page defines it',
  );
  rmSync(dir, { recursive: true, force: true });
}

console.log('an empty tree is a failure, not a pass…');
{
  const dir = mkdtempSync(join(tmpdir(), 'yidam-anchors-empty-'));
  const r = run(dir, '--base', '/yidam');
  check(
    'checking nothing does not report success',
    r.code !== 0,
    'a gate pointed at the wrong directory would otherwise be green and vacuous',
  );
  rmSync(dir, { recursive: true, force: true });
}

if (failures > 0) {
  console.error(`\n${failures} anchor-checker assertion(s) failed.`);
  process.exit(1);
}
console.log('\nall anchor-checker assertions hold.');
