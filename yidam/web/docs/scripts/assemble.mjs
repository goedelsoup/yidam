#!/usr/bin/env node
// One Pages artifact out of one build per version (#466).
//
//   node scripts/assemble.mjs --staging <dir> --out <dir> --versions <json-file>
//
// `--staging` holds one directory per build target, named by the id the matrix used, each
// the `dist/` that build produced. This lays them out under one root and then decorates
// them.
//
// # Why the decoration happens here and not in each build
//
// Two reasons, and the second is the one that decided it.
//
// A version is built from *its own tag*, so its `astro.config.mjs` is whatever that release
// shipped — and every tag that exists today predates this feature. A switcher added as a
// Starlight component override would appear on versions built after it and on no version
// built before, which includes the current release, which is what the site root serves. The
// most-visited page on the site would be the one page with no way to leave it.
//
// And the useful switcher — the one that keeps you on the page you were reading — needs to
// know which pages every *other* version has. Only the assembled tree knows that. A build
// knows one version.
//
// # What it fixes as well as adds
//
// Astro applies `base` to a redirect's key and not to its value, so every version's
// `redirects: { '/': '/yidam/what-yidam-is/' }` resolves to the *root* copy. Typing
// `/yidam/v0.6/` sent a reader to the latest release's first page instead of that version's.
// The old tags carry that in their configs and cannot be fixed at their source; it is fixed
// here, for every version at once.

import { existsSync, mkdirSync, cpSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';

import { ROOT, ROOT_SLOT } from '../src/versions.mjs';

function arg(name, required = true) {
  const i = process.argv.indexOf(`--${name}`);
  if (i < 0 || !process.argv[i + 1]) {
    if (required) throw new Error(`--${name} is required`);
    return undefined;
  }
  return process.argv[i + 1];
}

/**
 * Every file under `dir`, as paths relative to it, using `/` separators.
 *
 * `skip` names top-level directories to leave alone. The root alias is laid down *at* the
 * output root, so every other version sits inside its walk — without this it decorates them
 * a second time and each page carries two switchers, the second one claiming to be a
 * different version. Found by assembling the tree and reading it.
 */
export function walk(dir, prefix = '', skip = new Set()) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (!prefix && skip.has(entry.name)) continue;
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) out.push(...walk(join(dir, entry.name), rel, skip));
    else out.push(rel);
  }
  return out;
}

/**
 * The page a file is, as a version-relative path: `what-yidam-is/index.html` → `what-yidam-is/`.
 *
 * Anything that is not a page keeps its own path, so an asset is never mistaken for one.
 */
export function pagePath(rel) {
  return rel.endsWith('/index.html') ? rel.slice(0, -'index.html'.length) : rel;
}

/**
 * Where the switcher should send a reader who is on `page` and picks `target`.
 *
 * The same page in the other version when that version has it, and the other version's front
 * door when it does not — which is the common case going backwards, since `docs/` grows.
 * Never a bare `${base}/`: that is the redirect this script is here to fix, and following it
 * before it is rewritten would bounce across versions.
 */
export function switchTo(target, page, pagesOf) {
  const pages = pagesOf(target.id) ?? new Set();
  if (page && pages.has(page)) return `${target.base}/${page}`;
  return `${target.base}/${target.home}`;
}

/** The markup and styles injected into every page. */
export function decoration({ self, latest, versions, page, pagesOf }) {
  const options = versions
    .map((v) => {
      const href = switchTo(v, page, pagesOf);
      const label = v.development ? `${v.label} (unreleased)` : v.label;
      const suffix = v.latest ? ' · latest' : '';
      const selected = v.id === self.id ? ' selected' : '';
      return `<option value="${href}"${selected}>${label}${suffix}</option>`;
    })
    .join('');

  const notice = self.latest
    ? ''
    : self.development
      ? `<p class="yidam-vb__note"><strong>This documents main</strong> — unreleased tooling that no published version contains yet. The current release is <a href="${switchTo(latest, page, pagesOf)}">${latest.label}</a>.</p>`
      : `<p class="yidam-vb__note"><strong>This documents ${self.label}</strong>, an older release. The current release is <a href="${switchTo(latest, page, pagesOf)}">${latest.label}</a>.</p>`;

  // A plain <select> with an inline handler. This site ships no client framework — its React
  // integration is a build-time renderer with no `client:*` directive anywhere — and a
  // version menu is not a reason to start. With scripting off it still shows which version
  // is being read, which is the half that matters.
  return `<div class="yidam-vb" data-yidam-versions${self.latest ? '' : ' data-old'}>
<div class="yidam-vb__bar"><label class="yidam-vb__label" for="yidam-version">Version</label><select id="yidam-version" onchange="location.href=this.value">${options}</select></div>
${notice}</div>
<style>
.yidam-vb{font-family:var(--sl-font-system,system-ui,sans-serif);border-bottom:1px solid var(--sl-color-gray-5,#3f3f46);background:var(--sl-color-bg-nav,var(--sl-color-bg,#17181c))}
.yidam-vb__bar{display:flex;align-items:center;gap:.5rem;padding:.35rem .75rem;font-size:.8125rem}
.yidam-vb__label{color:var(--sl-color-gray-3,#a1a1aa);text-transform:uppercase;letter-spacing:.06em;font-size:.6875rem}
.yidam-vb select{font:inherit;font-family:var(--sl-font-system-mono,ui-monospace,monospace);color:var(--sl-color-white,#fff);background:var(--sl-color-black,#000);border:1px solid var(--sl-color-gray-5,#3f3f46);border-radius:.25rem;padding:.1rem .35rem;cursor:pointer}
.yidam-vb__note{margin:0;padding:.5rem .75rem;font-size:.8125rem;text-align:center;text-wrap:balance;background:var(--sl-color-orange-low,#3a2a12);color:var(--sl-color-white,#fff);border-top:1px solid var(--sl-color-gray-5,#3f3f46)}
.yidam-vb__note a{color:inherit}
</style>`;
}

/** Insert the decoration directly after the opening `<body>` tag. */
export function inject(html, markup) {
  const m = /<body[^>]*>/i.exec(html);
  if (!m) return null;
  const at = m.index + m[0].length;
  return html.slice(0, at) + markup + html.slice(at);
}

/**
 * A version's root redirect, pointed back inside that version.
 *
 * The generated file is small and its shape is stable across the Astro versions these tags
 * were built with: a `<title>`, a refresh `<meta>`, a canonical `<link>` and one `<a>`, each
 * carrying the same absolute URL. Rewriting the URL wherever it appears is therefore enough,
 * and doing it by replacement rather than by regenerating the file keeps whatever else a
 * future Astro puts in there.
 */
export function retargetRedirect(html, from, to) {
  return html.split(from).join(to);
}

// ── the assembly ─────────────────────────────────────────────────────────────

if (import.meta.url === `file://${process.argv[1]}`) {
  const staging = arg('staging');
  const out = arg('out');
  const versions = JSON.parse(readFileSync(arg('versions'), 'utf8'));

  const latest = versions.find((v) => v.latest);
  if (!latest) {
    throw new Error(
      'no version is marked latest, so nothing can be served from the site root. A ' +
        'repository with no `cli/v*` releases publishes `main` alone and does not reach here.',
    );
  }

  // Lay each build down at the path its base names. `main` and the versions go in
  // subdirectories; the alias — the latest, built a second time — is the root itself.
  const targets = [];
  for (const dir of readdirSync(staging, { withFileTypes: true })) {
    if (!dir.isDirectory()) continue;
    const isAlias = dir.name === ROOT_SLOT;
    const version = isAlias ? latest : versions.find((v) => v.id === dir.name);
    if (!version) {
      throw new Error(
        `staging holds \`${dir.name}\`, which is not a version in the list. A build whose ` +
          'artifact nobody claims would be published at a path nothing links to.',
      );
    }
    const suffix = isAlias ? '' : version.base.slice(ROOT.length);
    const dest = join(out, suffix);
    mkdirSync(dest, { recursive: true });
    cpSync(join(staging, dir.name), dest, { recursive: true });
    targets.push({ version, isAlias, dest, suffix });
  }

  const expected = versions.length + 1;
  if (targets.length !== expected) {
    throw new Error(
      `assembled ${targets.length} build(s) and the version list needs ${expected} ` +
        `(${versions.map((v) => v.id).join(', ')}, plus the root alias). A missing build is a ` +
        'path that 404s while every other page keeps linking to it.',
    );
  }

  // The subdirectories every version occupies. The alias owns the root and must not treat
  // them as its own pages.
  const versionDirs = new Set(
    versions.map((v) => v.base.slice(ROOT.length).replace(/^\//, '')).filter(Boolean),
  );

  // Which pages each version has, so the switcher can keep a reader on the page they are on.
  const pages = new Map();
  for (const t of targets) {
    if (t.isAlias) continue;
    pages.set(
      t.version.id,
      new Set(walk(t.dest, '', versionDirs).filter((f) => f.endsWith('.html')).map(pagePath)),
    );
  }
  // Each version's front door, for the case where the other version has no such page.
  for (const v of versions) {
    const has = pages.get(v.id) ?? new Set();
    v.home = has.has('what-yidam-is/') ? 'what-yidam-is/' : [...has][0] || '';
  }
  const pagesOf = (id) => pages.get(id);

  let decorated = 0;
  let skipped = 0;
  for (const t of targets) {
    for (const rel of walk(t.dest, '', t.isAlias ? versionDirs : new Set())) {
      if (!rel.endsWith('.html')) continue;
      const file = join(t.dest, rel);
      let html = readFileSync(file, 'utf8');

      // The version's own root redirect, which Astro pointed at the site root.
      if (rel === 'index.html' && html.includes('http-equiv="refresh"')) {
        const target = `${t.version.base}/${t.version.home}`;
        const wrong = `${ROOT}/${t.version.home}`;
        if (!t.isAlias && html.includes(wrong)) {
          html = retargetRedirect(html, wrong, target);
        }
        writeFileSync(file, html);
        continue;
      }

      const markup = decoration({
        self: t.version,
        latest,
        versions,
        page: pagePath(rel),
        pagesOf,
      });
      const next = inject(html, markup);
      if (next === null) {
        skipped++;
        continue;
      }
      writeFileSync(file, next);
      decorated++;
    }
  }

  console.error(
    `assembled ${targets.length} build(s) into ${out}; decorated ${decorated} page(s)` +
      (skipped ? `, skipped ${skipped} with no <body>` : ''),
  );
  if (decorated === 0) {
    throw new Error(
      'no page was decorated. Every version would publish without a switcher or a banner, ' +
        'which is the state #466 exists to end — and it would publish green.',
    );
  }
}
