// Generate every yidam logo asset from one definition.
//
// The state this ends: the mark existed in three copies that had already drifted, two of the
// three shipped assets had no consumer at all, and the editor shipped a second, unrelated
// mark. Nothing decided which was canonical, because nothing could — they were all hand-kept
// files of equal standing. So the geometry lives here, once, and every file in the tree is
// output. `git status` after a run is the whole guarantee.
//
// The mark is three commits and the two routes between the first and the last: straight along
// the trunk, and up through a branch. That is a fork and its merge, drawn as a closed graph —
// the repository's own subject, and the one reading of "a node in a graph" that is about git
// rather than about networks in general.
//
// It is monochrome, which is a change. `design/readme.md` had gold as "the mark in the logo",
// but gold-500 on ink-0 measures 2.97:1, and a mark that thin at 16px cannot spend contrast
// it does not have. Gold remains the UI accent everywhere else.
//
// The wordmark is Spectral 400, outlined. Outlined because an SVG inside an <img> is an
// isolated document that cannot reach the page's webfonts — the old wordmark was live <text>
// and rendered in whatever serif the reader happened to have. Spectral rather than the
// system's Cormorant Garamond because at 22px tall Cormorant holds 37.5% of its ink above
// half opacity and Spectral holds 47.0%; Cormorant's hairlines are a display-size face doing
// a UI-size job. Both are already loaded by `tokens/fonts.css`, so this costs no new request.
//
// To change the wordmark you need the font and `scripts/wordmark-outline.py`; the path below
// is its committed output. To change the mark, change the numbers here.

import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';

// ── geometry ───────────────────────────────────────────────────────────────
const BOX = 24;
const NODE_R = 2.8;
const STROKE = 2.6;
const NODES = { left: [4.5, 18.25], right: [19.5, 18.25], apex: [12, 5.75] };

// Wordmark outline: baseline at y=0, em = 100 units, tracking 0.04em already applied.
const WORDMARK_PATH = 'M-2.3 -45H21.6V-43.4L13.4 -40.3V-39.8L26.7 -10.8H27L38.2 -39.2V-39.8L30.7 -43.4V-45H50.3V-43.4L42.8 -40L20.6 13.9Q18.4 19.2 15.7 21.6Q13 24 8.7 24Q6.2 24 5.45 23.3Q4.7 22.6 4.7 19.7V15.8H14.4Q15.7 14 16.85 11.95Q18 9.9 19.35 6.9Q20.7 3.9 22.8 -0.7L4.4 -40.3L-2.3 -43.4Z M67.4 -58.1Q64.8 -58.1 63.1 -59.85Q61.4 -61.6 61.4 -64.1Q61.4 -66.7 63.1 -68.4Q64.8 -70.1 67.4 -70.1Q70 -70.1 71.7 -68.4Q73.4 -66.7 73.4 -64.1Q73.4 -61.6 71.7 -59.85Q70 -58.1 67.4 -58.1ZM57.4 0V-1.6L63.7 -4.8V-34.6L57.7 -39.8V-40.5L71.1 -46H71.7V-4.8L78 -1.6V0Z M109.1 1Q104.1 1 100.15 -1.5Q96.2 -4 93.95 -8.55Q91.7 -13.1 91.7 -19.4Q91.7 -27.9 95 -33.85Q98.3 -39.8 103.8 -42.9Q109.3 -46 115.8 -46Q118.6 -46 120.7 -45.75Q122.8 -45.5 124.5 -45.1V-63.6L118 -68.8V-69.5L131.9 -75H132.5V-4.5L139.6 -1.6V0H125.5L124.7 -5.1H124Q120.2 -1.9 116.7 -0.45Q113.2 1 109.1 1ZM99.7 -21.7Q99.7 -13.5 104.1 -9.7Q108.5 -5.9 114.6 -5.9Q117 -5.9 119.55 -6.65Q122.1 -7.4 124.5 -8.9V-36.9Q122.1 -39 119.75 -40.05Q117.4 -41.1 114.5 -41.1Q110.5 -41.1 107.15 -39.15Q103.8 -37.2 101.75 -32.95Q99.7 -28.7 99.7 -21.7Z M162.8 1Q158.1 1 155 -1.75Q151.9 -4.5 151.9 -9.4Q151.9 -14.3 155.55 -17.9Q159.2 -21.5 167.8 -23.5L177.3 -25.7V-33Q177.3 -36.5 174.75 -38.95Q172.2 -41.4 168.7 -41.4Q167.3 -41.4 165.9 -41.1Q164.5 -40.8 162.2 -39.7V-30.5H155.8Q154.2 -30.5 153.55 -31.25Q152.9 -32 152.9 -33.9Q152.9 -36.7 155.3 -39.5Q157.7 -42.3 161.85 -44.15Q166 -46 171.2 -46Q178.4 -46 181.85 -43.05Q185.3 -40.1 185.3 -35.1V-6.7Q186 -6 187.35 -5.5Q188.7 -5 190.9 -5H194.7L195 -4.8V-4.5Q193.8 -2.3 191.45 -0.65Q189.1 1 185.7 1Q182.6 1 180.65 -0.65Q178.7 -2.3 177.9 -5H177.3Q174.7 -2.5 170.85 -0.75Q167 1 162.8 1ZM159.7 -11.4Q159.7 -5.2 167.2 -5.2Q169.6 -5.2 171.95 -5.7Q174.3 -6.2 177.3 -7.5V-22.3Q169.3 -20.8 165.6 -19.15Q161.9 -17.5 160.8 -15.6Q159.7 -13.7 159.7 -11.4Z M204.9 0V-1.6L211.2 -4.8V-34.6L205.2 -39.8V-40.5L217.6 -46H218.2L219.1 -38.6H219.6Q223.4 -42.4 226.8 -44.2Q230.2 -46 234.3 -46Q239.1 -46 242.5 -43.85Q245.9 -41.7 246.9 -37.4H247.2Q251.4 -41.8 255.05 -43.9Q258.7 -46 263.1 -46Q268.7 -46 272.35 -43.05Q276 -40.1 276 -34.3V-4.8L282.3 -1.6V0H260.9V-1.6L268 -4.8V-29.6Q268 -34 265.35 -36.3Q262.7 -38.6 258.6 -38.6Q255.2 -38.6 252.2 -37.5Q249.2 -36.4 247.2 -34.2V-4.8L254.1 -1.6V0H232.5V-1.6L239.2 -4.8V-29.6Q239.2 -34 236.55 -36.3Q233.9 -38.6 229.8 -38.6Q227.3 -38.6 224.2 -37.7Q221.1 -36.8 219.2 -35.1V-4.8L225.7 -1.6V0Z';
const WORDMARK_INK = { x0: -2.3, x1: 282.3, top: -75.0, bottom: 24.0 };

// ── colours ────────────────────────────────────────────────────────────────
const INK = '#181614';    // --ink-900, for a light ground
const PAPER = '#f5f2eb';  // --ink-50,  for a dark ground
const GROUND = '#181614'; // the plate the Marketplace tile carries

// ── builders ───────────────────────────────────────────────────────────────
const { left: L, right: R, apex: A } = NODES;

function markBody(colour, indent = '  ') {
  const paint = colour === 'currentColor' ? 'currentColor' : colour;
  const edges = `M${L[0]} ${L[1]}H${R[0]}M${L[0]} ${L[1]} ${A[0]} ${A[1]}M${A[0]} ${A[1]} ${R[0]} ${R[1]}`;
  const disc = ([cx, cy]) => `${indent}<circle cx="${cx}" cy="${cy}" r="${NODE_R}" fill="${paint}"/>`;
  return [
    `${indent}<path d="${edges}" fill="none" stroke="${paint}" stroke-width="${STROKE}" stroke-linecap="round"/>`,
    disc(L), disc(R), disc(A),
  ].join('\n');
}

// Scale the em so the wordmark's x-height band sits on the mark's optical centre.
const EM = 20;
const BASELINE = 17.0;
const GAP = 7;
const s = EM / 100;
const wmX = (L[0] - NODE_R) + (R[0] - L[0]) + NODE_R * 2 + GAP - WORDMARK_INK.x0 * s;
const LOCKUP_W = Math.round((wmX + WORDMARK_INK.x1 * s + (L[0] - NODE_R)) * 100) / 100;

function wordmarkBody(colour, tx, ty, indent = '  ') {
  return `${indent}<path transform="translate(${tx} ${ty}) scale(${s})" d="${WORDMARK_PATH}" fill="${colour}"/>`;
}

const svg = (w, h, body, extra = '') =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"` +
  `${extra}>\n${body}\n</svg>\n`;

// ── assets ─────────────────────────────────────────────────────────────────
const wmW = Math.round((WORDMARK_INK.x1 - WORDMARK_INK.x0) * s * 100) / 100;
const wmH = Math.round((WORDMARK_INK.bottom - WORDMARK_INK.top) * s * 100) / 100;

// Each asset declares what it is, because the gate that applies to a 16px favicon is not the
// gate that applies to an 88x24 lockup. A mark is square and must survive a browser tab; a
// lockup is sized by its height in a header and carries type, whose counters legitimately
// fill in below its minimum size. Judging them alike either passes a bad favicon or fails a
// good lockup.
const assets = {
  // canonical, in the design system
  'yidam/design/assets/logo-mark.svg': { kind: 'mark',
    svg: svg(BOX, BOX, markBody(INK), ' aria-label="yidam mark"') },
  'yidam/design/assets/logo-mark-dark.svg': { kind: 'mark',
    svg: svg(BOX, BOX, markBody(PAPER), ' aria-label="yidam mark"') },
  'yidam/design/assets/wordmark.svg': { kind: 'wordmark',
    svg: svg(wmW, wmH, wordmarkBody(INK, -WORDMARK_INK.x0 * s, -WORDMARK_INK.top * s), ' aria-label="yidam"') },
  'yidam/design/assets/wordmark-dark.svg': { kind: 'wordmark',
    svg: svg(wmW, wmH, wordmarkBody(PAPER, -WORDMARK_INK.x0 * s, -WORDMARK_INK.top * s), ' aria-label="yidam"') },
  'yidam/design/assets/logo.svg': { kind: 'lockup',
    svg: svg(LOCKUP_W, BOX, [markBody(INK), wordmarkBody(INK, wmX, BASELINE)].join('\n'), ' aria-label="yidam"') },
  'yidam/design/assets/logo-dark.svg': { kind: 'lockup',
    svg: svg(LOCKUP_W, BOX, [markBody(PAPER), wordmarkBody(PAPER, wmX, BASELINE)].join('\n'), ' aria-label="yidam"') },

  // docs site: Starlight takes a light and a dark source and picks by theme
  'yidam/web/docs/src/assets/logo-mark.svg': { kind: 'mark',
    svg: svg(BOX, BOX, markBody(INK), ' aria-label="yidam mark"') },
  'yidam/web/docs/src/assets/logo-mark-dark.svg': { kind: 'mark',
    svg: svg(BOX, BOX, markBody(PAPER), ' aria-label="yidam mark"') },

  // A tab has no theme to inherit, so the favicon carries its own query. It switches `color`
  // and paints with `currentColor` rather than setting `fill` in the rule: a CSS `fill`
  // outranks the `fill="none"` presentation attribute on the edge path, which would fill the
  // mark the day those three open segments became one closed outline. The `color` attribute
  // on the root is the fallback for anything that does not run the media query.
  'yidam/web/docs/public/favicon.svg': { kind: 'mark',
    svg: svg(BOX, BOX, `  <style>\n` +
      `    @media (prefers-color-scheme: dark) { svg { color: ${PAPER}; } }\n` +
      `  </style>\n` +
      markBody('currentColor'),
      ` color="${INK}" aria-label="yidam mark"`) },

  // VS Code recolours activity-bar icons to the theme foreground, so this one must be
  // monochrome and must not carry a colour of its own.
  'yidam/editors/vscode/resources/yidam.svg': { kind: 'mark',
    svg: svg(BOX, BOX, markBody('currentColor'), '') },

  // The Marketplace card renders the icon as-is against a card that is light for some
  // readers and dark for others, so this one brings its own ground.
  'yidam/editors/vscode/resources/icon.svg': { kind: 'mark',
    svg: svg(BOX, BOX, `  <rect width="${BOX}" height="${BOX}" rx="5" fill="${GROUND}"/>\n` + markBody(PAPER), '') },
};

// The guard asks for the list rather than keeping its own copy of it, so a file added here
// is covered the moment it is added.
if (process.argv.includes('--manifest')) {
  console.log(JSON.stringify(
    Object.entries(assets).map(([path, a]) => ({ path, kind: a.kind })), null, 2));
  process.exit(0);
}

const outRoot = process.argv.includes('--out')
  ? process.argv[process.argv.indexOf('--out') + 1]
  : process.cwd();

for (const [rel, asset] of Object.entries(assets)) {
  const path = join(outRoot, rel);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, asset.svg);
  console.log(`  ${rel}  (${asset.kind})`);
}
console.log(`\n${Object.keys(assets).length} files. icon.png is rendered from icon.svg by the mise task.`);
