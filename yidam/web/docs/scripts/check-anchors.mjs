#!/usr/bin/env node
// Every internal link resolves — to a page, and to the heading it names.
//
//   node scripts/check-anchors.mjs <site-dir>
//
// The Astro build already fails on a dead relative link in `docs/` and on a sidebar entry
// naming a page that does not exist. What it has never checked is the part after the `#`.
// A wrong fragment is the quietest kind of broken link: the browser finds the page, finds no
// such heading, and leaves the reader at the top of a document that looks right. Nothing goes
// red, and the reader is simply somewhere else than the author sent them.
//
// This runs over the *assembled* site rather than one build, so it also covers the links the
// assembly itself writes — the switcher's, and the version root redirects it retargets.

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

/** Every file under `dir`, relative, `/`-separated. */
function walk(dir, prefix = '') {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) out.push(...walk(join(dir, entry.name), rel));
    else out.push(rel);
  }
  return out;
}

/** `id="…"` and `name="…"`, which is what a fragment can land on. */
export function anchorsIn(html) {
  const out = new Set();
  for (const m of html.matchAll(/\sid="([^"]+)"/g)) out.add(m[1]);
  for (const m of html.matchAll(/<a[^>]+\sname="([^"]+)"/g)) out.add(m[1]);
  return out;
}

/** Every `href` in a document, in source order. */
export function hrefsIn(html) {
  return [...html.matchAll(/\shref="([^"]*)"/g)].map((m) => m[1]);
}

/**
 * The site path a file serves.
 *
 * `what-yidam-is/index.html` is served at `what-yidam-is/`, and a fragment on that page is
 * checked against that file. Anything else is served at its own path.
 */
export function servedAt(rel) {
  return rel.endsWith('/index.html') ? rel.slice(0, -'index.html'.length) : rel;
}

/**
 * Resolve an href against the page it appears on, or `null` when it is not ours to check.
 *
 * Skipped: absolute URLs, `mailto:`, and a bare `#` — an empty fragment is "this page".
 */
export function resolveHref(href, fromPage, root) {
  if (!href || href === '#') return null;
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('//')) return null;

  const [pathPart, fragment] = href.split('#');
  let path;
  if (pathPart === '') {
    path = fromPage;
  } else if (pathPart.startsWith('/')) {
    if (!pathPart.startsWith(root)) return null; // another site on the same host
    path = pathPart.slice(root.length).replace(/^\//, '');
  } else {
    const base = fromPage.replace(/[^/]*$/, '');
    path = new URL(pathPart, `http://x/${base}`).pathname.replace(/^\//, '');
  }
  return { path, fragment: fragment || null };
}

const root = process.argv[2];
if (!root) {
  console.error('usage: check-anchors.mjs <site-dir>');
  process.exit(2);
}
const ROOT_PREFIX = '/yidam';

/**
 * Fragments the framework emits and owns, listed rather than pattern-matched.
 *
 * `#_top` is Starlight's table-of-contents "Overview" entry. It has no matching `id` on any
 * page — 195 of them in this site — and a browser scrolls to the top for an unresolvable
 * fragment, which is what the link means. It is Starlight's markup, not this repository's,
 * and nothing here can fix it.
 *
 * An exclusion roster, so it says what it excludes and why. Anything not named here is
 * graded, which is the direction that matters: a new framework fragment shows up as a
 * failure and gets a decision, rather than being swallowed by a pattern nobody revisits.
 */
const FRAMEWORK_FRAGMENTS = new Set(['_top']);

const files = walk(root);
const pages = new Map(); // served path -> anchors
const present = new Set(); // every served path, pages and assets alike
for (const rel of files) {
  present.add(servedAt(rel));
  present.add(rel);
  if (rel.endsWith('.html')) pages.set(servedAt(rel), anchorsIn(readFileSync(join(root, rel), 'utf8')));
}

const deadPages = [];
const deadAnchors = [];
for (const rel of files) {
  if (!rel.endsWith('.html')) continue;
  const from = servedAt(rel);
  const html = readFileSync(join(root, rel), 'utf8');
  for (const href of hrefsIn(html)) {
    const target = resolveHref(href, from, ROOT_PREFIX);
    if (!target) continue;
    if (!present.has(target.path) && !present.has(`${target.path}index.html`)) {
      deadPages.push(`  ${from} → ${href}`);
      continue;
    }
    if (!target.fragment || FRAMEWORK_FRAGMENTS.has(target.fragment)) continue;
    const anchors = pages.get(target.path);
    if (!anchors) continue; // a fragment on a non-HTML target; not ours to grade
    if (!anchors.has(target.fragment) && !anchors.has(decodeURIComponent(target.fragment))) {
      deadAnchors.push(`  ${from} → ${href}`);
    }
  }
}

console.error(
  `checked ${pages.size} page(s): ${deadPages.length} dead link(s), ` +
    `${deadAnchors.length} dead anchor(s)`,
);

if (pages.size === 0) {
  console.error('no pages were checked, so this asserted nothing');
  process.exit(1);
}

if (deadPages.length || deadAnchors.length) {
  if (deadPages.length) {
    console.error(`\n${deadPages.length} link(s) to a page this site does not publish:`);
    console.error(deadPages.slice(0, 40).join('\n'));
  }
  if (deadAnchors.length) {
    console.error(
      `\n${deadAnchors.length} link(s) to a heading the target page does not have. A reader ` +
        'following one lands at the top of the right document and is told nothing:',
    );
    console.error(deadAnchors.slice(0, 40).join('\n'));
  }
  process.exit(1);
}
