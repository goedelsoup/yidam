// Score a logo SVG against the thresholds the mark redraw is held to.
//
// The failure this answers: the mark was judged at 400% in a design tool and shipped to a
// browser tab, where 90.5% of its inked pixels land under half opacity and the tab shows a
// grey blur. Nobody was wrong about how it looked; they were looking at the wrong size. So
// this renders a candidate at the size its consumer actually requests and reports numbers,
// and every candidate gets judged small before anyone looks at it large.
//
// Legibility is the share of non-transparent pixels at or above 50% alpha. A mark drawn in
// strokes too thin for the pixel grid does not vanish — it turns into a wash of 20%-alpha
// pixels, which is the same amount of "ink" and none of the legibility. Counting inked
// pixels alone would call that a pass.
//
// Rendering emulates a browser fitting an SVG into a square slot: the longer axis is fitted
// to the box and the shorter one letterboxes. A 48x32 mark in a 16px tab is drawn 16x11,
// not 16x16, and its strokes scale by 16/48 rather than 16/32.
//
// `--fit=height` is for the assets that are not square. A lockup is sized by its height in a
// header, never by fitting its long axis into a box — asked for "24px" of an 88x24 lockup a
// square slot gives you 24 wide and 6 tall, which is not a size anybody renders it at, and
// scoring that answers a question nobody asked.
//
// Legibility alone is not enough, and the branch-and-merge candidate proved it: it scored 75%
// at 16px by closing up into a solid blob. High ink coverage and an unreadable mark are the
// same number. So counters are counted too — the enclosed background regions inside the mark.
// A form whose counters survive from 128px down to 16px is still the shape that was drawn; one
// whose counters fill in has become a silhouette of itself, whatever its alpha says.
//
// A full-canvas background rect is removed before scoring. Left in, it makes every pixel in
// the box opaque and the score reads 96.8% no matter how thin the strokes on top of it are —
// the metric would be measuring the plate. The plate is still reported, because whether an
// asset carries its own ground is a real fact about it: the Marketplace tile must, and the
// activity-bar icon must not.
//
// The SVG is read with regexes rather than a parser. These are hand-authored files of a few
// dozen lines that this repository controls; a dependency to read them would cost more than
// it returns. The limit is real though: a stroke width set from CSS, or geometry reached
// through <use>, is not counted here.

import { execFileSync } from 'node:child_process';
import { inflateSync } from 'node:zlib';
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, basename } from 'node:path';

// ── PNG ────────────────────────────────────────────────────────────────────
//
// rsvg-convert writes 8-bit RGBA, non-interlaced, which is the only shape decoded here; a
// file that is anything else is a changed toolchain and should say so rather than be
// guessed at.
function alphaChannel(png) {
  if (png.subarray(0, 8).toString('hex') !== '89504e470d0a1a0a') {
    throw new Error('not a PNG');
  }
  const width = png.readUInt32BE(16);
  const height = png.readUInt32BE(20);
  const [depth, colorType, , , interlace] = [png[24], png[25], png[26], png[27], png[28]];
  if (depth !== 8 || colorType !== 6 || interlace !== 0) {
    throw new Error(`expected 8-bit RGBA non-interlaced, got depth=${depth} colorType=${colorType} interlace=${interlace}`);
  }

  const idat = [];
  for (let off = 8; off < png.length; ) {
    const len = png.readUInt32BE(off);
    const type = png.subarray(off + 4, off + 8).toString('latin1');
    if (type === 'IDAT') idat.push(png.subarray(off + 8, off + 8 + len));
    off += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));

  // Un-filter. Each scanline is prefixed with its filter type and is decoded against the
  // reconstructed line above it, so this has to run in order and cannot be vectorised away.
  const bpp = 4;
  const stride = width * bpp;
  const out = Buffer.alloc(height * stride);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? out[y * stride + x - bpp] : 0;
      const b = y > 0 ? out[(y - 1) * stride + x] : 0;
      const c = x >= bpp && y > 0 ? out[(y - 1) * stride + x - bpp] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      } else if (filter !== 0) throw new Error(`unknown PNG filter ${filter}`);
      out[y * stride + x] = v & 0xff;
    }
  }

  const alpha = [];
  for (let i = 3; i < out.length; i += 4) alpha.push(out[i]);
  return { width, height, alpha };
}

// Enclosed background regions: flood the background inward from the border, then every
// unvisited non-ink pixel belongs to a counter. Four-connected on purpose — an eight-connected
// flood leaks through the diagonal gaps that anti-aliasing leaves at these sizes.
function counters(width, height, alpha) {
  const ink = alpha.map((a) => a >= 128);
  const seen = new Uint8Array(width * height);
  const stack = [];
  for (let x = 0; x < width; x++) { stack.push(x, x + (height - 1) * width); }
  for (let y = 0; y < height; y++) { stack.push(y * width, width - 1 + y * width); }
  while (stack.length) {
    const i = stack.pop();
    if (seen[i] || ink[i]) continue;
    seen[i] = 1;
    const x = i % width, y = (i - x) / width;
    if (x > 0) stack.push(i - 1);
    if (x < width - 1) stack.push(i + 1);
    if (y > 0) stack.push(i - width);
    if (y < height - 1) stack.push(i + width);
  }
  let regions = 0;
  const visited = new Uint8Array(width * height);
  for (let i = 0; i < ink.length; i++) {
    if (ink[i] || seen[i] || visited[i]) continue;
    regions++;
    const q = [i];
    while (q.length) {
      const j = q.pop();
      if (visited[j] || ink[j] || seen[j]) continue;
      visited[j] = 1;
      const x = j % width, y = (j - x) / width;
      if (x > 0) q.push(j - 1);
      if (x < width - 1) q.push(j + 1);
      if (y > 0) q.push(j - width);
      if (y < height - 1) q.push(j + width);
    }
  }
  return regions;
}

// ── render ─────────────────────────────────────────────────────────────────
function renderAt(svgPath, box, aspect, fit) {
  // Fit the longer axis to the box, the way a browser fits an SVG into a square slot;
  // or fit the height, the way a header sizes a lockup.
  const args = fit === 'height' || aspect < 1 ? ['-h', String(box)] : ['-w', String(box)];
  return execFileSync('rsvg-convert', [...args, svgPath], { maxBuffer: 1 << 26 });
}

// ── SVG shape ──────────────────────────────────────────────────────────────
const DRAWN = 'circle|ellipse|rect|line|path|polygon|polyline|text|use|image';

function inspect(source) {
  const stripped = source
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<defs[\s\S]*?<\/defs>/g, '');

  const viewBox = source.match(/viewBox="([\d.\s-]+)"/);
  let aspect = 1, vb = null;
  if (viewBox) {
    const [, , w, h] = viewBox[1].trim().split(/\s+/).map(Number);
    vb = { w, h };
    aspect = w / h;
  }

  const elements = [...stripped.matchAll(new RegExp(`<(${DRAWN})[\\s/>]`, 'g'))].map((m) => m[1]);
  const strokeWidths = [
    ...new Set([...stripped.matchAll(/stroke-width="([\d.]+)"/g)].map((m) => Number(m[1]))),
  ].sort((a, b) => a - b);

  // Both halves of an arrowhead: the marker that draws it and the attribute that places it.
  // A <marker> in <defs> with nothing pointing at it draws nothing.
  const markers = [...source.matchAll(/marker-(?:start|mid|end)="url\(/g)].length;

  // A rect that covers the whole viewBox is a ground, not a drawn element.
  let plate = null;
  for (const m of stripped.matchAll(/<rect\b[^>]*>/g)) {
    const w = m[0].match(/\bwidth="([\d.%]+)"/);
    const h = m[0].match(/\bheight="([\d.%]+)"/);
    if (!w || !h) continue;
    const covers = (v, extent) => v === '100%' || (vb && Number(v) === extent);
    if (covers(w[1], vb?.w) && covers(h[1], vb?.h)) plate = m[0];
  }

  return {
    plate: plate !== null,
    viewBox: vb,
    aspect,
    elements: elements.length,
    elementKinds: elements,
    strokeWidths,
    distinctStrokeWidths: strokeWidths.length,
    markers,
    textNodes: elements.filter((e) => e === 'text').length,
  };
}

function score(svgPath, sizes, fit = 'box') {
  const source = readFileSync(svgPath, 'utf8');
  const shape = inspect(source);

  // Score the mark, not the plate it may sit on.
  let scored = svgPath;
  if (shape.plate) {
    scored = join(tmpdir(), `mark-score-${process.pid}-${basename(svgPath)}`);
    writeFileSync(scored, source.replace(/<rect\b[^>]*\/?>/, ''));
  }

  const renders = sizes.map((box) => {
    const { width, height, alpha } = alphaChannel(renderAt(scored, box, shape.aspect, fit));
    const inked = alpha.filter((a) => a > 0);
    const solid = inked.filter((a) => a >= 128).length;
    return {
      box,
      drawn: `${width}x${height}`,
      inked: inked.length,
      legibility: inked.length ? solid / inked.length : 0,
      meanAlpha: inked.length ? inked.reduce((s, a) => s + a, 0) / inked.length : 0,
      counters: counters(width, height, alpha),
    };
  });
  return { path: svgPath, ...shape, renders };
}

// ── report ─────────────────────────────────────────────────────────────────
const pct = (n) => `${(n * 100).toFixed(1)}%`;

const args = process.argv.slice(2);
const json = args.includes('--json');
const paths = args.filter((a) => !a.startsWith('--'));
const sizeArg = args.find((a) => a.startsWith('--sizes='));
const sizes = sizeArg ? sizeArg.slice(8).split(',').map(Number) : [16, 24, 32, 128];
const fitArg = args.find((a) => a.startsWith('--fit='));
const fit = fitArg ? fitArg.slice(6) : 'box';

if (paths.length === 0) {
  console.error('usage: node scripts/mark-score.mjs <svg...> [--sizes=16,24] [--fit=box|height] [--json]');
  process.exit(2);
}

const results = paths.map((p) => score(p, sizes, fit));

if (json) {
  console.log(JSON.stringify(results, null, 2));
} else {
  for (const r of results) {
    const ratio = r.viewBox ? `${r.viewBox.w}x${r.viewBox.h}` : 'none';
    const square = r.viewBox && r.viewBox.w === r.viewBox.h ? 'square' : `${r.aspect.toFixed(2)}:1`;
    console.log(`\n${r.path}`);
    console.log(`  viewBox ${ratio} (${square})   elements ${r.elements}` +
      `   stroke widths ${r.strokeWidths.join('/') || 'none'}` +
      `   markers ${r.markers}   <text> ${r.textNodes}`);
    for (const s of r.renders) {
      console.log(`  ${String(s.box).padStart(4)}px  drawn ${s.drawn.padEnd(8)}` +
        ` legible ${pct(s.legibility).padStart(6)}` +
        `  mean alpha ${s.meanAlpha.toFixed(0).padStart(3)}/255` +
        `  counters ${s.counters}` +
        `  inked ${s.inked}px`);
    }
  }
  console.log('');
}
