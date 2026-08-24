import { existsSync, statSync } from 'node:fs';
import { dirname, relative, resolve, sep } from 'node:path';

/**
 * Relative links in `docs/` point at repository paths, because `docs/` is read on
 * GitHub as well as here. `[the constitution](../../yidam/prelude/CONSTITUTION.md)`
 * and `[RFC-0005](0005-mcp-tool-contract.md)` are correct there and 404 on a
 * rendered site, which is 147 dead links across this corpus — most of the outbound
 * navigation the docs have.
 *
 * Rewriting them at render time keeps one spelling in the source. Each link is
 * resolved against the file it appears in and sent to whichever surface actually
 * holds the target:
 *
 *   - another published page  → its route on this site
 *   - anything else in the repo → the file on GitHub
 *   - nothing at all           → a build failure, because it is dead on GitHub too
 *
 * The last one is why this is a plugin and not a find-and-replace: a link that
 * resolves to no file is a defect wherever it is read, and the docs build is the
 * only thing in CI that looks at these links at all.
 */

/** Where the plugin reads the id of the page being rendered. */
export const SOURCE_ID = '__yidamDocId';

/**
 * GitHub's line anchors (`#L27`, `#L107-L120`) address lines of a source file.
 * They mean nothing on a rendered page, so a link carrying one to a published
 * page keeps the page and drops the anchor: the reader lands on the right
 * document, on this site, instead of being sent out to the repository.
 */
const LINE_ANCHOR = /^#L\d+(?:-L\d+)?$/;

const EXTERNAL = /^(?:[a-z][a-z0-9+.-]*:|\/\/|\/|#)/i;

interface HastNode {
  type: string;
  tagName?: string;
  properties?: Record<string, unknown>;
  children?: HastNode[];
}

export interface RewriteOptions {
  /** Absolute path to the repository root. */
  repoRoot: string;
  /** Absolute path to `docs/`. */
  docsDir: string;
  /** Page ids this site builds — see docs-source.ts. */
  published: Set<string>;
  /** Astro's `base`, e.g. `/yidam`. */
  base: string;
  /** e.g. `https://github.com/goedelsoup/yidam` */
  repoUrl: string;
  /** Branch the site is built from. */
  branch: string;
}

function walk(node: HastNode, visit: (n: HastNode) => void) {
  visit(node);
  for (const child of node.children ?? []) walk(child, visit);
}

/** Markdown percent-encodes a link target containing a space; the filesystem does not. */
function decodePath(path: string): string {
  if (!path.includes('%')) return path;
  try {
    return decodeURIComponent(path);
  } catch {
    return path;
  }
}

function isInside(parent: string, child: string): boolean {
  const rel = relative(parent, child);
  return rel !== '' && !rel.startsWith('..') && !rel.startsWith(sep);
}

export function rewriteRepoLinks(opts: RewriteOptions) {
  const prefix = opts.base.replace(/\/+$/, '');

  return function attach() {
    return function transform(tree: HastNode, file: { data?: any; path?: string }) {
      const sourceId: string | undefined = file?.data?.astro?.frontmatter?.[SOURCE_ID];
      if (!sourceId) return;

      const sourceDir = dirname(resolve(opts.docsDir, `${sourceId}.md`));
      const dead: string[] = [];

      walk(tree, (node) => {
        if (node.type !== 'element' || node.tagName !== 'a') return;
        const href = node.properties?.href;
        if (typeof href !== 'string' || href === '' || EXTERNAL.test(href)) return;

        const hash = href.indexOf('#');
        const path = hash === -1 ? href : href.slice(0, hash);
        const fragment = hash === -1 ? '' : href.slice(hash);
        if (path === '') return;

        const target = resolve(sourceDir, decodePath(path));
        if (!isInside(opts.repoRoot, target) || !existsSync(target)) {
          dead.push(href);
          return;
        }

        if (isInside(opts.docsDir, target) && target.endsWith('.md')) {
          const id = relative(opts.docsDir, target).replace(/\.md$/, '').split(sep).join('/');
          if (opts.published.has(id)) {
            const anchor = LINE_ANCHOR.test(fragment) ? '' : fragment;
            node.properties!.href = `${prefix}/${id}/${anchor}`;
            return;
          }
        }

        const kind = statSync(target).isDirectory() ? 'tree' : 'blob';
        const repoPath = relative(opts.repoRoot, target).split(sep).join('/');
        node.properties!.href = `${opts.repoUrl}/${kind}/${opts.branch}/${repoPath}${fragment}`;
      });

      if (dead.length > 0) {
        throw new Error(
          `docs/${sourceId}.md links to ${dead.length} path(s) that do not exist:\n` +
            dead.map((d) => `  ${d}`).join('\n') +
            `\nThese are broken on GitHub too. Fix the link or delete it.`,
        );
      }
    };
  };
}
