// Assembling one site out of one build per version, and the ways it could publish a lie.
//
// A fixture tree rather than five real Astro builds: the thing under test is the layout and
// the decoration, and a fixture can hold the shapes that matter — a page one version has and
// another does not, a version root redirect pointing at the wrong version, a file with no
// `<body>` — without waiting for five builds to produce them by accident.

import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { decoration, inject, pagePath, retargetRedirect, switchTo, walk } from '../scripts/assemble.mjs';
import { ROOT } from '../src/versions.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const site = join(here, '..');

let failures = 0;
function check(name, condition, detail = '') {
  if (condition) return;
  failures++;
  console.error(`  ✗ ${name}${detail ? `\n      ${detail}` : ''}`);
}

const VERSIONS = [
  { id: 'main', label: 'main', ref: 'main', base: `${ROOT}/main`, latest: false, development: true },
  { id: 'v0.7', label: '0.7.0', ref: 'cli/v0.7.0', base: `${ROOT}/v0.7`, latest: true, development: false },
  { id: 'v0.5', label: '0.5.0', ref: 'cli/v0.5.0', base: `${ROOT}/v0.5`, latest: false, development: false },
];

console.log('the pieces…');
{
  check('a page is its directory', pagePath('cli-reference/index.html') === 'cli-reference/');
  check('an asset is itself', pagePath('_astro/x.css') === '_astro/x.css');

  const pagesOf = (id) => (id === 'v0.5' ? new Set(['what-yidam-is/']) : new Set(['what-yidam-is/', 'vaults/']));
  const v05 = VERSIONS.find((v) => v.id === 'v0.5');
  v05.home = 'what-yidam-is/';
  check(
    'a page the other version has keeps the reader on it',
    switchTo(VERSIONS[1], 'vaults/', (id) => pagesOf(id)) === `${ROOT}/v0.7/vaults/`,
  );
  check(
    'a page it does not have falls back to its front door, not a 404',
    switchTo(v05, 'vaults/', pagesOf) === `${ROOT}/v0.5/what-yidam-is/`,
    switchTo(v05, 'vaults/', pagesOf),
  );

  check(
    'a document with no body is left alone rather than corrupted',
    inject('<?xml version="1.0"?><urlset/>', '<div/>') === null,
  );
  check(
    'and one with a body gets the markup immediately inside it',
    inject('<html><body class="x">hi</body></html>', '<b/>') ===
      '<html><body class="x"><b/>hi</body></html>',
  );

  check(
    'a version root redirect is pointed back inside its own version',
    retargetRedirect(
      '<meta http-equiv="refresh" content="0;url=/yidam/what-yidam-is/">',
      '/yidam/what-yidam-is/',
      '/yidam/v0.5/what-yidam-is/',
    ).includes('/yidam/v0.5/what-yidam-is/'),
  );
}

console.log('the banner says which release, and only when there is one to name…');
{
  const pagesOf = () => new Set(['p/']);
  for (const v of VERSIONS) v.home = 'p/';
  const latest = VERSIONS[1];

  const onLatest = decoration({ self: latest, latest, versions: VERSIONS, page: 'p/', pagesOf });
  check('the latest carries no banner', !onLatest.includes('This documents'), onLatest.slice(0, 120));
  check('but still carries a switcher', onLatest.includes('<select id="yidam-version"'));

  const onOld = decoration({ self: VERSIONS[2], latest, versions: VERSIONS, page: 'p/', pagesOf });
  check('an old release says so', onOld.includes('This documents 0.5.0'), onOld.slice(0, 200));
  check('and names the current one', onOld.includes('>0.7.0</a>'));

  const onMain = decoration({ self: VERSIONS[0], latest, versions: VERSIONS, page: 'p/', pagesOf });
  check(
    'main makes the opposite claim, not the same one',
    onMain.includes('unreleased') && !onMain.includes('an older release'),
    onMain.slice(0, 200),
  );

  const selected = [...onOld.matchAll(/<option[^>]*selected[^>]*>([^<]*)/g)].map((m) => m[1]);
  check('exactly one option is selected', selected.length === 1, selected.join(','));
  check('and it is the version being read', selected[0].startsWith('0.5.0'), selected[0]);
}

console.log('and end to end, over a tree laid out the way the workflow lays one out…');
{
  const tmp = mkdtempSync(join(tmpdir(), 'yidam-assemble-'));
  const staging = join(tmp, 'staging');
  const out = join(tmp, 'out');

  const page = (title) => `<!doctype html><html><body><h1>${title}</h1></body></html>`;
  const redirect = (url) =>
    `<!doctype html><title>Redirecting to: ${url}</title><meta http-equiv="refresh" content="0;url=${url}"><body><a href="${url}">go</a></body>`;

  const write = (p, body) => {
    mkdirSync(dirname(p), { recursive: true });
    writeFileSync(p, body);
  };

  for (const id of ['main', 'v0.7', 'v0.5', '__root__']) {
    write(join(staging, id, 'what-yidam-is', 'index.html'), page(`what ${id}`));
    // Every build emits the same wrong root redirect: Astro applies `base` to the key only.
    write(join(staging, id, 'index.html'), redirect(`${ROOT}/what-yidam-is/`));
  }
  // A page the older release does not have.
  for (const id of ['main', 'v0.7', '__root__']) {
    write(join(staging, id, 'vaults', 'index.html'), page(`vaults ${id}`));
  }
  // Something with no <body> at all.
  write(join(staging, 'main', 'sitemap.xml'), '<?xml version="1.0"?><urlset/>');

  const versionsFile = join(tmp, 'versions.json');
  writeFileSync(
    versionsFile,
    JSON.stringify(VERSIONS.map(({ home, ...v }) => v)),
  );

  execFileSync(
    'node',
    [
      join(site, 'scripts/assemble.mjs'),
      '--staging', staging,
      '--out', out,
      '--versions', versionsFile,
    ],
    { encoding: 'utf8', stdio: 'pipe' },
  );

  const read = (p) => readFileSync(join(out, p), 'utf8');
  const files = walk(out);

  check(
    'the root serves the latest release',
    files.includes('what-yidam-is/index.html'),
    files.join(', '),
  );
  check('and each version sits under its own path', files.includes('v0.5/what-yidam-is/index.html'));

  const root = read('what-yidam-is/index.html');
  check('exactly one switcher at the root', (root.match(/<div class="yidam-vb"/g) || []).length === 1);
  check('and no banner, because the root is the latest', !root.includes('This documents'));

  const old = read('v0.5/what-yidam-is/index.html');
  check('exactly one switcher on an old version', (old.match(/<div class="yidam-vb"/g) || []).length === 1);
  check('which says which release it is', old.includes('This documents 0.5.0'));

  check(
    "a version's root redirect stays inside that version",
    read('v0.5/index.html').includes(`${ROOT}/v0.5/what-yidam-is/`),
    read('v0.5/index.html'),
  );
  check(
    "and the site root's redirect still points at the root",
    read('index.html').includes(`content="0;url=${ROOT}/what-yidam-is/"`),
    read('index.html'),
  );

  check(
    'a page an old release lacks sends it to its front door',
    read('main/vaults/index.html').includes(`${ROOT}/v0.5/what-yidam-is/`),
    'the switcher offered v0.5 a page that release never had',
  );
  check(
    'the file with no <body> is untouched',
    read('main/sitemap.xml') === '<?xml version="1.0"?><urlset/>',
  );

  // The failure this whole script exists to prevent, asserted rather than assumed: no page
  // may be decorated twice. The root alias is laid down *at* the output root, so every other
  // version is inside its walk.
  const doubled = files
    .filter((f) => f.endsWith('.html'))
    .filter((f) => (read(f).match(/<div class="yidam-vb"/g) || []).length > 1);
  check('no page carries two switchers', doubled.length === 0, doubled.join(', '));

  rmSync(tmp, { recursive: true, force: true });
}

if (failures > 0) {
  console.error(`\n${failures} assembly assertion(s) failed.`);
  process.exit(1);
}
console.log('\nall assembly assertions hold.');
