#!/usr/bin/env node
// The I/O half of the version list: read the tags, ask crates.io, print the answer.
//
// Separate from `src/versions.mjs` on purpose. Everything that *decides* is a pure function
// over arguments there, where `test/versions.mjs` reaches it without a network or a checkout;
// this file only gathers, and its one piece of judgement is what to do when the gathering
// fails.
//
//   node scripts/versions.mjs            the version list, as JSON
//   node scripts/versions.mjs --matrix   the same, shaped for a GitHub Actions matrix
//
// Both write to stdout. Diagnostics go to stderr so a caller can pipe this into `jq`.

import { execFileSync } from 'node:child_process';
import { ROOT, buildTargets, resolve as resolveVersions, yankedVersions } from '../src/versions.mjs';

/** The crate the CLI publishes as. `cli/v0.7.0` is its `0.7.0`. */
const CRATE = 'yidam';

/**
 * Every tag in the repository.
 *
 * `git tag -l` and not `gh api /releases`: a release list is ordered by *publication* and
 * spans all four layers, which is the shape of #397 — `releases/latest` answering for a
 * layer the caller did not mean. Tags are a set, and `cliTags` filters it.
 *
 * A shallow checkout has no tags and yields an empty list, which would silently publish only
 * `main`. The workflow fetches them; this refuses rather than pretends.
 */
function tags() {
  const out = execFileSync('git', ['tag', '-l'], { encoding: 'utf8' });
  const all = out.split('\n').map((t) => t.trim()).filter(Boolean);
  if (all.length === 0) {
    throw new Error(
      'no tags in this checkout. A shallow clone has none, and a version list built from ' +
        'none would publish `main` alone while reporting success — fetch tags first ' +
        '(actions/checkout with `fetch-tags: true`, or `git fetch --tags`).',
    );
  }
  return all;
}

/**
 * Versions crates.io reports as yanked, or an empty set if it could not be asked.
 *
 * The direction of this failure is the whole point. #466 drops a yanked version's docs
 * entirely — its URLs 404 — so an error that read as "everything is yanked" would withdraw
 * the documentation during somebody else's outage. Unknown therefore means "nothing known to
 * be yanked", and the docs stay up.
 *
 * The opposite risk is real and smaller: a yanked version keeps its docs until the next
 * successful build.
 */
async function yanked() {
  const url = `https://crates.io/api/v1/crates/${CRATE}`;
  try {
    const response = await fetch(url, {
      headers: { 'user-agent': 'yidam-docs-build (github.com/goedelsoup/yidam)' },
      signal: AbortSignal.timeout(20_000),
    });
    if (!response.ok) {
      throw new Error(`${response.status} ${response.statusText}`);
    }
    const set = yankedVersions(await response.json());
    console.error(`crates.io: ${set.size} yanked version(s)${set.size ? `: ${[...set]}` : ''}`);
    return set;
  } catch (e) {
    console.error(
      `::warning::could not reach crates.io (${e}); building every tag in the window. A ` +
        'yanked release keeps its documentation until the next successful build.',
    );
    return new Set();
  }
}

// On a pull request there is one thing worth building: the change itself. The released
// versions cannot have changed — they are tags — and building them would spend five jobs to
// re-prove that four immutable refs still compile. The PR keeps the gate it has always had,
// which is the dead-link and sidebar-completeness check over its own tree, and the tags are
// built when something is actually published.
const pullRequest = process.argv.includes('--pull-request');
const versions = pullRequest
  ? [
      {
        id: 'main',
        label: 'main',
        // Empty: `actions/checkout` with no `ref` takes the event's own, which on a pull
        // request is the merge commit — the thing being reviewed.
        ref: '',
        base: ROOT,
        // Not `latest`: the alias exists so the site root can serve a release, and there is
        // no release here and nothing to assemble. Marking it latest would emit the same
        // build twice under two names.
        latest: false,
        development: true,
      },
    ]
  : resolveVersions({ tags: tags(), yanked: await yanked() });
console.error(
  `versions: ${versions.map((v) => `${v.id}${v.latest ? ' (latest)' : ''}`).join(', ')}`,
);

if (process.argv.includes('--matrix')) {
  // One job per *build target*, which is one more than there are versions: the latest
  // release is built again at the site root. `include` rather than a bare list so each job
  // carries the ref it checks out and the base it builds for, instead of recomputing them
  // in YAML where nothing tests them.
  const targets = buildTargets(versions);
  console.error(`builds: ${targets.map((t) => t.base).join(', ')}`);
  process.stdout.write(JSON.stringify({ include: targets }));
} else if (process.argv.includes('--builds')) {
  process.stdout.write(JSON.stringify(buildTargets(versions), null, 2));
} else {
  // The switcher's list: what a reader can choose between. The root alias is not among
  // them — it is a second copy of a version already listed, and offering it would put one
  // release in the menu twice.
  process.stdout.write(JSON.stringify(versions, null, 2));
}
process.stdout.write('\n');
