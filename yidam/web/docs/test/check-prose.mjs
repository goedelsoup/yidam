// The prose checker, and the ways a guard like it fails open.
//
// A fixture tree rather than the real `docs/`: the shapes that matter are a page that
// regresses, a page nobody assigned a tier, a page that skips a heading level, and a tree with
// nothing in it — and the real corpus produces those only by accident, if at all.
//
// Every assertion here was seen to fail before it was seen to pass. A scanning guard that
// looks at nothing passes, which is the failure this file exists to rule out.

import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { measure, proseBlocks, sentences, tierOf, wordCount } from '../scripts/check-prose.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const script = join(here, '../scripts/check-prose.mjs');

let failures = 0;
function check(name, condition, detail = '') {
  if (condition) return;
  failures++;
  console.error(`  ✗ ${name}${detail ? `\n      ${detail}` : ''}`);
}

/** Run the checker over a fixture tree. Findings go to stderr, so both streams are kept. */
function run(root, baseline, ...args) {
  const r = spawnSync(process.execPath, [script, '--root', root, '--baseline', baseline, ...args],
    { encoding: 'utf8' });
  return { code: r.status, out: `${r.stdout}${r.stderr}` };
}

function tree(files) {
  const dir = mkdtempSync(join(tmpdir(), 'prose-'));
  for (const [rel, body] of Object.entries(files)) {
    const full = join(dir, rel);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, body);
  }
  return dir;
}

const SHORT = '# Page\n\n## A section\n\nThis is short. So is this one.\n';
const LONG = '# Page\n\n## A section\n\n'
  + 'This particular sentence is deliberately built to run well past the tier one ceiling of '
  + 'twenty words so that the checker has something it must object to.\n';

// ── the units ────────────────────────────────────────────────────────────────
console.error('a table row and a bullet are not sentences…');
check('a table is not prose', proseBlocks('| a | b |\n|---|---|\n| 1 | 2 |').length === 0);
check('adjacent bullets are separate blocks',
  proseBlocks('- one thing\n- another thing').length === 2,
  'joining them is what reported 155 run-ons that did not exist');
check('a wrapped bullet is one block',
  proseBlocks('- one thing that runs\n  onto a second line').length === 1,
  'the marker line was skipped and the continuation was not, so items were measured from their tails');
check('a bullet block drops its marker',
  proseBlocks('- one thing')[0] === 'one thing');
check('a fence is not prose', proseBlocks('```\nlet x = 1;\n```').join('') === '');
check('a paragraph is prose', proseBlocks('Hello there.\nSecond line.').length === 1);

console.error('a sentence that opens with a link is still a sentence…');
check('splits before a markdown link',
  sentences('One thing. [Two](x.md) is another.').length === 2,
  'the lookahead must admit "[", or pairs glue together and every count inflates');
check('splits before inline code', sentences('One thing. `code` follows.').length === 2);

console.error('a bold claim that ends in a full stop is a sentence, not a lead-in…');
check('splits after a bold sentence',
  sentences('**One instruction per sentence.** A step that does two is two sentences.').length === 2,
  'the house style opens a paragraph this way; gluing them charges the page for a run-on it does not contain');
check('splits after an italic sentence',
  sentences('It gives the reason: *the artifact outlives the access.* A directory is intent.').length === 2);
check('a full stop inside emphasis mid-sentence does not split',
  sentences('The flag is **required.**').length === 1);

console.error('a link counts as the words a reader reads, not its target…');
check('link target is not counted', wordCount('[a b](https://example.com/very/long/path) c') === 3);
check('inline code is one word', wordCount('`a --flag --and --another` b') === 2);

console.error('the tier is a judgement, and an unknown page has none…');
check('a task page is tier 1', tierOf('quickstart.md') === 1);
check('a reference page is tier 2', tierOf('vocabulary.md') === 2);
check('an RFC is tier 3', tierOf('rfcs/0001-report-contract.md') === 3);
check('an unknown page has no tier', tierOf('brand-new-page.md') === null,
  'defaulting an unassigned page to a tier is how a guard stops covering new files');

console.error('a page whose first subheading is ### skips a level…');
check('h1 then h3 is a skip', measure('# T\n\n### S\n\nWords here now.\n', 20).skipsLevel === true);
check('h1 then h2 is not', measure('# T\n\n## S\n\nWords here now.\n', 20).skipsLevel === false);
check('a page with no subheading is not', measure('# T\n\nWords here now.\n', 20).skipsLevel === false);

// ── the gate ─────────────────────────────────────────────────────────────────
console.error('a clean tree passes, and a regressed one does not…');
{
  const root = tree({ 'quickstart.md': SHORT });
  const base = join(root, 'b.json');
  writeFileSync(base, JSON.stringify({ 'quickstart.md': 0 }));
  check('clean passes', run(root, base).code === 0);

  writeFileSync(join(root, 'quickstart.md'), LONG);
  const bad = run(root, base);
  check('a new long sentence fails', bad.code === 1);
  check('and names the page', bad.out.includes('quickstart.md'), bad.out);
  rmSync(root, { recursive: true, force: true });
}

console.error('a page nobody tiered is a failure, not a skip…');
{
  const root = tree({ 'brand-new-page.md': SHORT });
  const base = join(root, 'b.json');
  writeFileSync(base, '{}');
  const r = run(root, base);
  check('untiered fails', r.code === 1);
  check('and says so', /no tier/.test(r.out), r.out);
  rmSync(root, { recursive: true, force: true });
}

console.error('a page absent from the baseline is a failure, not a skip…');
{
  const root = tree({ 'quickstart.md': SHORT });
  const base = join(root, 'b.json');
  writeFileSync(base, '{}');
  const r = run(root, base);
  check('unlisted fails', r.code === 1);
  check('and points at --update', /--update/.test(r.out), r.out);
  rmSync(root, { recursive: true, force: true });
}

console.error('a skipped heading level fails even on a tier 3 page…');
{
  const root = tree({ 'rfcs/0001-x.md': '# T\n\n### S\n\nSome prose lives here.\n' });
  const base = join(root, 'b.json');
  writeFileSync(base, '{}');
  const r = run(root, base);
  check('tier 3 still owes the heading rule', r.code === 1);
  check('and names the skip', /skip/.test(r.out), r.out);
  rmSync(root, { recursive: true, force: true });
}

console.error('an empty tree is a failure, not a pass…');
{
  const root = tree({});
  const base = join(root, 'b.json');
  writeFileSync(base, '{}');
  const r = run(root, base);
  check('nothing scanned is a failure', r.code === 1, r.out);
  rmSync(root, { recursive: true, force: true });
}

console.error('--update writes what it measured…');
{
  const root = tree({ 'quickstart.md': LONG });
  const base = join(root, 'b.json');
  writeFileSync(base, '{}');
  check('update exits zero', run(root, base, '--update').code === 0);
  check('and then the gate is green', run(root, base).code === 0);
  rmSync(root, { recursive: true, force: true });
}

if (failures) {
  console.error(`\n${failures} prose-checker assertion(s) failed.`);
  process.exit(1);
}
console.error('\nall prose-checker assertions hold.');
