// The version list, and the ways it could quietly document the wrong thing.
//
// Every case here is a real tag namespace from this repository, not an invented one. The
// `editor/v*`, `sdk/rust/v*` and unprefixed `v0.1.0` entries exist; the point of most of
// these assertions is that they are ignored.

import { execFileSync } from 'node:child_process';

import { buildTargets, cliTags, resolve, yankedVersions, KEEP, ROOT } from '../src/versions.mjs';

let failures = 0;
function check(name, condition, detail = '') {
  if (condition) return;
  failures++;
  console.error(`  ✗ ${name}${detail ? `\n      ${detail}` : ''}`);
}

// The tag list this repository actually has, plus the two layers most likely to confuse it.
const REAL_TAGS = [
  'bootstrap/v0.1.0',
  'cli/v0.2.0',
  'cli/v0.2.1',
  'cli/v0.3.0',
  'cli/v0.4.0',
  'cli/v0.5.0',
  'cli/v0.6.0',
  'cli/v0.7.0',
  'editor/v0.1.0',
  'editor/v0.2.0',
  'sdk/rust/v0.1.0',
  'sdk/rust/v0.1.1',
  'sdk/rust/v0.2.0',
  'v0.1.0',
  'v0.1.1',
  'v0.2.0',
];

console.log('the version list is Layer 4 and nothing else…');
{
  const tags = cliTags(REAL_TAGS).map((t) => t.tag);
  check(
    'every tag kept is a cli tag',
    tags.every((t) => t.startsWith('cli/v')),
    tags.filter((t) => !t.startsWith('cli/v')).join(', '),
  );
  check('and all seven of them are', tags.length === 7, `kept ${tags.length}: ${tags}`);
  check(
    'the unprefixed template tags are not versions of this site',
    !tags.includes('v0.2.0'),
    'v0.1.0/v0.1.1/v0.2.0 are Layer 1 tags that predate the namespacing',
  );
  check(
    'and neither is the newest editor release',
    !tags.some((t) => t.startsWith('editor/')),
    'editor/v0.2.0 is the second-newest release in this repository; a switcher that asked ' +
      'for "the latest release" would offer it as a version of the docs',
  );
  check(
    'newest first',
    cliTags(REAL_TAGS)[0].tag === 'cli/v0.7.0',
    `got ${cliTags(REAL_TAGS)[0].tag}`,
  );
}

console.log('a URL is a series, so a patch release does not orphan a link…');
{
  const withPatch = resolve({ tags: [...REAL_TAGS, 'cli/v0.7.1'] });
  const bases = withPatch.map((v) => v.base);
  check(
    'two patches of one minor are one path',
    new Set(bases).size === bases.length,
    `duplicate base: ${bases}`,
  );
  const latest = withPatch.find((v) => v.latest);
  check('the series is built from its newest patch', latest.ref === 'cli/v0.7.1', latest.ref);
  check('and its URL did not move', latest.base === '/yidam/v0.7', latest.base);
}

console.log('the window keeps three series, and main is not one of them…');
{
  const list = resolve({ tags: REAL_TAGS });
  check(`${KEEP} series plus main`, list.length === KEEP + 1, `got ${list.length}`);
  check('main is first and is not a release', list[0].id === 'main' && list[0].development);
  check(
    'and it is not the latest',
    !list[0].latest && list[1].latest,
    'main must never be marked latest: it documents unreleased tooling',
  );
  check(
    'the three newest series, newest first',
    list.slice(1).map((v) => v.id).join(',') === 'v0.7,v0.6,v0.5',
    list.slice(1).map((v) => v.id).join(','),
  );
}

console.log('a yanked release is dropped, and the window backfills…');
{
  const list = resolve({ tags: REAL_TAGS, yanked: new Set(['0.6.0']) });
  check(
    'the yanked series is gone',
    !list.some((v) => v.id === 'v0.6'),
    list.map((v) => v.id).join(','),
  );
  check(
    'and a fourth series takes its place, so three are still published',
    list.slice(1).map((v) => v.id).join(',') === 'v0.7,v0.5,v0.4',
    list.slice(1).map((v) => v.id).join(','),
  );

  const latestYanked = resolve({ tags: REAL_TAGS, yanked: new Set(['0.7.0']) });
  check(
    'yanking the newest promotes the next one to latest',
    latestYanked.find((v) => v.latest)?.id === 'v0.6',
    latestYanked.find((v) => v.latest)?.id,
  );

  // A series with a live patch is still a series.
  const onePatchYanked = resolve({
    tags: [...REAL_TAGS, 'cli/v0.7.1'],
    yanked: new Set(['0.7.1']),
  });
  check(
    'yanking a patch falls back within the series rather than dropping it',
    onePatchYanked.find((v) => v.id === 'v0.7')?.ref === 'cli/v0.7.0',
    onePatchYanked.find((v) => v.id === 'v0.7')?.ref,
  );
}

console.log('not knowing is not the same as knowing nothing is wrong…');
{
  // The failure that would 404 every version of the docs during someone else's outage.
  check(
    'an unreachable crates.io yields no yanks rather than every yank',
    yankedVersions(null).size === 0 && yankedVersions(undefined).size === 0,
  );
  check('and so does a response shape this does not recognise', yankedVersions({}).size === 0);
  check(
    'a real response is read',
    (() => {
      const y = yankedVersions({
        versions: [
          { num: '0.7.0', yanked: false },
          { num: '0.6.0', yanked: true },
          { num: '0.5.0', yanked: false },
        ],
      });
      return y.size === 1 && y.has('0.6.0');
    })(),
  );
  check(
    'with no yanks known, the list is the full window',
    resolve({ tags: REAL_TAGS, yanked: yankedVersions(null) }).length === KEEP + 1,
    'a failed lookup must degrade to publishing the docs, not to withdrawing them',
  );
}

console.log('a repository with no releases still has a site…');
{
  const list = resolve({ tags: ['editor/v0.1.0', 'v0.1.0'] });
  check('main alone', list.length === 1 && list[0].id === 'main', list.map((v) => v.id).join(','));
  check('and nothing claims to be latest', !list.some((v) => v.latest));
}

console.log('the site root serves a release, and main says which it is…');
{
  const list = resolve({ tags: REAL_TAGS });
  const main = list.find((v) => v.id === 'main');
  check(
    'main is not at the site root any more',
    main.base === `${ROOT}/main`,
    `${main.base} — a reader arriving at the advertised URL used to get unreleased tooling`,
  );

  const targets = buildTargets(list);
  check(
    'one more build than there are versions',
    targets.length === list.length + 1,
    `${targets.length} builds for ${list.length} versions`,
  );

  const alias = targets.filter((t) => t.alias);
  check('exactly one of them is the root alias', alias.length === 1, `${alias.length}`);
  check('and it is served from the root', alias[0]?.base === ROOT, alias[0]?.base);
  check(
    'built from the latest release',
    alias[0]?.ref === 'cli/v0.7.0',
    `${alias[0]?.ref} — the root must serve what a reader can install`,
  );
  check(
    'carrying the latest id, so it renders no "old release" banner',
    alias[0]?.id === 'v0.7' && alias[0]?.latest === true,
    `${alias[0]?.id} latest=${alias[0]?.latest}`,
  );

  const bases = targets.map((t) => t.base);
  check('no two builds write the same path', new Set(bases).size === bases.length, bases.join(','));
  check(
    'every versioned build is under the root',
    targets.filter((t) => !t.alias).every((t) => t.base.startsWith(`${ROOT}/`)),
    bases.join(','),
  );
}

console.log('a repository with no releases publishes no alias…');
{
  const targets = buildTargets(resolve({ tags: ['editor/v0.1.0'] }));
  check('main alone, and nothing aliased to the root', targets.length === 1 && !targets[0].alias);
}

console.log('and against the tags this repository actually has…');
{
  // Discovered, not the fixture above: the fixture proves the function's behaviour, and this
  // proves the behaviour is right about the real tag namespace. #397's defect was that a
  // real-world tag set did not look like the one anybody had in mind.
  let real = [];
  try {
    real = execFileSync('git', ['tag', '-l'], { encoding: 'utf8' })
      .split('\n')
      .map((t) => t.trim())
      .filter(Boolean);
  } catch (e) {
    real = [];
  }
  check(
    'the checkout has tags to check',
    real.length > 0,
    'a shallow clone has none, and every assertion below would be vacuous',
  );

  const kept = cliTags(real).map((t) => t.tag);
  check(
    'no other layer leaks into the version list',
    kept.every((t) => t.startsWith('cli/v')),
    kept.filter((t) => !t.startsWith('cli/v')).join(', '),
  );
  check(
    'and some cli tag was found',
    kept.length > 0,
    `real tags: ${real.slice(0, 8).join(', ')}…`,
  );

  // The trap, against live data: the newest tag in this repository is not necessarily a cli
  // tag, so "the newest release" is not an answer to "which version of the docs".
  const newestOverall = execFileSync(
    'git',
    ['for-each-ref', '--sort=-creatordate', '--format=%(refname:short)', '--count=1', 'refs/tags'],
    { encoding: 'utf8' },
  ).trim();
  const newestCli = kept[0];
  check(
    'the version list names a cli tag whatever the newest tag happens to be',
    newestCli.startsWith('cli/v'),
    `newest tag overall is ${newestOverall}; the list chose ${newestCli}`,
  );
}

if (failures > 0) {
  console.error(`\n${failures} version assertion(s) failed.`);
  process.exit(1);
}
console.log('\nall version assertions hold.');
