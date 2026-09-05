#!/usr/bin/env node
// The style guide, enforced. `docs/style-guide.md` states a sentence ceiling per tier and a
// heading rule; this is what makes them cost something.
//
//   node scripts/check-prose.mjs [--update]
//
// A written standard nobody measures decays in the direction nobody looks — the same failure
// `cli_reference.rs` exists for. Two rules are mechanical enough to check:
//
//   1. Sentence length, per tier. Tier 1 (task pages) 20 words, Tier 2 (reference) 25,
//      Tier 3 (argument, walkthroughs, RFCs) no ceiling — the register is the point there,
//      and `aesthetic-direction.md` commits to it.
//   2. Every page's first subheading is `##`, not `###`. Not a rendering rule: Starlight
//      measures its contents panel from the shallowest heading a page has, so an all-`###`
//      page renders a panel that looks right. It is the document outline that is wrong.
//
// ── why a baseline and not the target ────────────────────────────────────────
// Tier 1 sits at 18% over 25 words against a target of 10%, so a gate at the target would be
// red on the day it lands and would teach everyone to skip it. This ratchets instead, the way
// `.yidam/lint-baseline.yml` does for the corpus: the committed baseline is what each page
// scores today, a page may not get worse, and beating it prints the number to lower.
//
// ── the two ways a guard like this rots ──────────────────────────────────────
// A hardcoded roster stops covering new pages without ever going red, so the page set is
// *discovered* and a page missing from the baseline is a failure, not a skip. And a tier
// assignment is a judgement, so an unassigned page fails rather than defaulting to the
// loosest tier — adding a page forces someone to say what it is.

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
// `--root` and `--baseline` exist so the tests can drive this over a fixture tree. Without
// them the empty-scan guard below is unreachable, and an unreachable guard is one nobody has
// ever seen fire.
const rootArg = process.argv.indexOf('--root');
const baseArg = process.argv.indexOf('--baseline');
const DOCS = rootArg > 0 ? process.argv[rootArg + 1] : join(here, '../../../../docs');
const BASELINE = baseArg > 0 ? process.argv[baseArg + 1] : join(here, 'prose-baseline.json');

/** Pages the site does not publish, so the standard does not reach them. */
const UNPUBLISHED = new Set(['README.md']);

/** Tier 1 — task pages. The reader is executing something; ceiling 20. */
export const TIER1 = new Set([
  'quickstart.md', 'installation.md', 'configuration.md', 'troubleshooting.md',
  'editor-setup.md', 'upgrading.md', 'mcp-server.md', 'artifact-vaults.md',
  'sharing-derivations.md', 'cli-reference.md',
]);

/** Tier 2 — reference. Descriptive rather than procedural; ceiling 25. */
export const TIER2 = new Set([
  'vocabulary.md', 'information-architecture.md', 'git-branch-model.md', 'bootstrap-flow.md',
  'domain-computer.md', 'web-interface.md', 'sangha-resolution-flow.md',
  'constitutional-governance.md', 'conduct-norms.md', 'quality-rubric.md', 'test-harness.md',
  'post-genesis-measurement.md', 'style-guide.md',
  'ontology/what-an-ontology-is.md', 'ontology/choosing-an-alignment.md',
  'ontology/alignment-in-practice.md',
]);

/** Tier 3 — argument and narrative. No ceiling; the register is deliberate. */
export function isTier3(rel) {
  return rel.startsWith('rfcs/') || rel.startsWith('research/') || rel.startsWith('walkthroughs/')
    || ['what-yidam-is.md', 'aesthetic-direction.md', 'contributing.md', 'versioning.md'].includes(rel);
}

export function tierOf(rel) {
  if (TIER1.has(rel)) return 1;
  if (TIER2.has(rel)) return 2;
  if (isTier3(rel)) return 3;
  return null; // unassigned — a failure, never a default
}

const CEILING = { 1: 20, 2: 25 };

/** Every markdown page under docs/, discovered rather than listed. */
export function pages(root) {
  const out = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const full = join(root, entry.name);
    if (entry.isDirectory()) out.push(...pages(full).map((p) => join(entry.name, p)));
    else if (entry.name.endsWith('.md')) out.push(entry.name);
  }
  return out;
}

/**
 * Prose paragraphs, and each list item as its own paragraph.
 *
 * Code fences, tables, headings and block quotes are excluded. **List items are not**, and the
 * reason the first version excluded them does not survive contact: joining *adjacent* bullets
 * into one block is what reported 155 run-ons that did not exist, and the fix for that is to
 * start a new block at every marker — not to stop reading bullets. A bullet on a task page is
 * an instruction, and "one instruction per sentence" is the rule it is most likely to break.
 *
 * Excluding them also only ever half-worked. The marker line was skipped and a wrapped item's
 * continuation lines were not, so a two-line bullet was measured from its second line: one
 * finding on `cli-reference` read `record says \`redistributable: true\`.** A default of…`,
 * which is not a sentence anybody wrote. Reading the whole item costs 13 findings across
 * `docs/` and adds 189 sentences, and closes the dodge of bulleting past the ceiling.
 */
export function proseBlocks(md) {
  const noFence = md.replace(/```[\s\S]*?```/g, '\n\n').replace(/<!--[\s\S]*?-->/g, '\n\n');
  const blocks = [];
  let cur = [];
  const flush = () => { if (cur.length) { blocks.push(cur.join(' ')); cur = []; } };
  for (const line of noFence.split('\n')) {
    const s = line.trim();
    const marker = line.match(/^\s*([-*+]\s|\d+\.\s)/);
    if (marker) { flush(); cur.push(s.slice(marker[1].length)); continue; }
    if (!s || /^[|>#]/.test(s) || /^[-*_]{3,}$/.test(s) || /^ {4}/.test(line)) { flush(); continue; }
    cur.push(s);
  }
  flush();
  return blocks;
}

/**
 * Split a paragraph into sentences.
 *
 * The lookahead admits `[`, `` ` `` and `*` as well as a capital: a sentence beginning with a
 * markdown link or inline code is still a sentence, and omitting them silently glued pairs
 * together and inflated every count on the page.
 *
 * The lookbehind lets closing emphasis sit between the full stop and the space, because the
 * house style opens a paragraph with a bold claim — `**One instruction per sentence.** A step
 * that does two things is two sentences.` Without it those are one 27-word sentence, and the
 * page is charged for a run-on it does not contain: 22 of the 146 findings on `cli-reference`
 * and `mcp-server` were this, and every one would have been "fixed" by rewriting prose that
 * was already inside the ceiling.
 */
export function sentences(block) {
  return block.split(/(?<=[.!?][*`_"')\]]*)\s+(?=[A-Z"'*`[(—])/).map((s) => s.trim()).filter(Boolean);
}

/** Words, with inline code and link targets reduced to the token a reader actually reads. */
export function wordCount(s) {
  return s.replace(/`[^`]*`/g, 'C')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/[*_]+/g, '')
    .split(/\s+/).filter((w) => /\w/.test(w)).length;
}

/** Over-ceiling sentence count, and whether the page skips from `#` to `###`. */
export function measure(md, ceiling) {
  let over = 0, total = 0;
  for (const b of proseBlocks(md)) {
    for (const s of sentences(b)) {
      const n = wordCount(s);
      if (n < 3) continue;
      total++;
      if (ceiling && n > ceiling) over++;
    }
  }
  const headings = [...md.matchAll(/^(#{1,6}) /gm)].map((m) => m[1].length);
  const subs = headings.filter((h) => h >= 2);
  return { over, total, skipsLevel: subs.length > 0 && !subs.includes(2) };
}

// ── run ──────────────────────────────────────────────────────────────────────
// Only when this file *is* the command. The tests import the functions above, and a module
// that gates the real `docs/` as a side effect of being imported would take the whole suite
// down with it the first time a page regressed.
const isEntry = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (!isEntry) { /* imported for its exports */ } else {

const update = process.argv.includes('--update');
const found = pages(DOCS).filter((p) => !UNPUBLISHED.has(p)).sort();

if (found.length === 0) {
  console.error(`check-prose: no pages under ${relative(process.cwd(), DOCS)} — the scan found nothing to check.`);
  process.exit(1);
}

let baseline = {};
try { baseline = JSON.parse(readFileSync(BASELINE, 'utf8')); } catch { /* first run */ }

const untiered = [], regressed = [], beat = [], skipped = [], unlisted = [];
const next = {};

for (const rel of found) {
  const tier = tierOf(rel);
  if (tier === null) { untiered.push(rel); continue; }
  const md = readFileSync(join(DOCS, rel), 'utf8');
  const { over, total, skipsLevel } = measure(md, CEILING[tier]);
  if (skipsLevel) skipped.push(rel);
  if (tier === 3) continue; // no ceiling; the heading rule still applied above
  next[rel] = over;
  if (!(rel in baseline)) { unlisted.push(`${rel} (${over} over, ${total} sentences)`); continue; }
  if (over > baseline[rel]) regressed.push(`${rel}: ${over} over ceiling, baseline allows ${baseline[rel]}`);
  else if (over < baseline[rel]) beat.push(`${rel}: ${over} — lower its baseline from ${baseline[rel]}`);
}

if (update) {
  writeFileSync(BASELINE, `${JSON.stringify(next, null, 2)}\n`);
  console.error(`check-prose: baseline written for ${Object.keys(next).length} page(s).`);
  process.exit(0);
}

const problems = [];
if (untiered.length) {
  problems.push(`${untiered.length} page(s) have no tier in check-prose.mjs:\n` +
    untiered.map((p) => `  ${p}`).join('\n') +
    `\nAdd each to TIER1, TIER2 or isTier3 — which tier a page is in is a judgement, not a default.`);
}
if (unlisted.length) {
  problems.push(`${unlisted.length} page(s) are not in prose-baseline.json:\n` +
    unlisted.map((p) => `  ${p}`).join('\n') +
    `\nRun \`node scripts/check-prose.mjs --update\` and commit the baseline.`);
}
if (regressed.length) {
  problems.push(`${regressed.length} page(s) got longer-winded:\n` +
    regressed.map((p) => `  ${p}`).join('\n') +
    `\nSplit the new long sentences, or say why the ceiling does not serve this page.`);
}
if (skipped.length) {
  problems.push(`${skipped.length} page(s) skip from '#' to '###':\n` +
    skipped.map((p) => `  ${p}`).join('\n') +
    `\nA page's first subheading is '##'. See docs/style-guide.md.`);
}

if (problems.length) {
  console.error(`check-prose: ${problems.length} problem(s).\n\n${problems.join('\n\n')}`);
  process.exit(1);
}

const t1 = found.filter((p) => tierOf(p) === 1);
const overT1 = t1.reduce((a, p) => a + next[p], 0);
console.error(`check-prose: ${found.length} page(s) checked, no regressions. ` +
  `Tier 1 carries ${overT1} sentence(s) over its ${CEILING[1]}-word ceiling.` +
  (beat.length ? `\n${beat.length} page(s) beat their baseline:\n${beat.map((b) => `  ${b}`).join('\n')}` : ''));

}
