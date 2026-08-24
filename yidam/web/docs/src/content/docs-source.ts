import { readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * `docs/` at the repository root, relative to this Astro project (yidam/web/docs/).
 *
 * Both the content loader and astro.config.mjs resolve the same constant, so the
 * sidebar and the pages it points at are read from one directory by construction.
 */
export const DOCS_BASE = '../../../docs';

/**
 * The repository root, same relative walk one level shorter.
 */
export const REPO_BASE = '../../../';

/**
 * Markdown under `docs/` that the site deliberately does not publish.
 *
 * `README.md` is the directory's index for someone browsing the repository: a
 * contents table, plus a "what belongs here" note addressed to whoever edits
 * `docs/`. The sidebar is the site's version of that table, so publishing it
 * would ship a second copy that goes stale the first time the two disagree.
 * Links to it from other pages are rewritten to GitHub, which is where the page
 * is doing its job.
 */
export const UNPUBLISHED = new Set(['README']);

/** Absolute path to a directory named by one of the constants above. */
export function resolveFrom(base: URL, relativePath: string): string {
  return fileURLToPath(new URL(relativePath, base));
}

/** Every .md file under `dir`, recursively, as absolute paths. */
export function findMdFiles(dir: string): string[] {
  const results: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) results.push(...findMdFiles(full));
    else if (entry.isFile() && entry.name.endsWith('.md')) results.push(full);
  }
  return results;
}

/** The page id (= Starlight slug) a docs/ file is published under. */
export function docId(absPath: string, docsDir: string): string {
  return relative(docsDir, absPath).replace(/\.md$/, '').split(/[\\/]/).join('/');
}

/**
 * Every page id the site builds, sorted — the single answer to "what is on this
 * site", used by the loader to build pages and by astro.config.mjs to check that
 * the sidebar reaches all of them.
 */
export function publishedIds(docsDir: string): string[] {
  return findMdFiles(docsDir)
    .map((f) => docId(f, docsDir))
    .filter((id) => !UNPUBLISHED.has(id))
    .sort();
}
