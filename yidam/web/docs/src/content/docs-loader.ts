import { readFile } from 'node:fs/promises';
import { createMarkdownProcessor } from '@astrojs/markdown-remark';
import type { Loader, LoaderContext } from 'astro/loaders';
import { DOCS_BASE, REPO_BASE, docId, findMdFiles, publishedIds, resolveFrom } from './docs-source.ts';
import { SOURCE_ID, rewriteRepoLinks } from './rewrite-repo-links.ts';

/** The repository this site publishes, and the branch it is built from. */
const REPO_URL = 'https://github.com/goedelsoup/yidam';
const BRANCH = 'main';

// Parse a simple frontmatter block (---\nkey: value\n---) from raw markdown.
function parseFrontmatter(raw: string): { data: Record<string, unknown>; body: string } {
  const fm = raw.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)([\s\S]*)$/);
  if (!fm) return { data: {}, body: raw };

  const data: Record<string, unknown> = {};
  for (const line of fm[1].split(/\r?\n/)) {
    const kv = line.match(/^([\w-]+):\s*(.*)/);
    if (kv) data[kv[1]] = kv[2].trim().replace(/^["']|["']$/g, '');
  }
  return { data, body: fm[2] };
}

// Extract the text of the first ATX heading (# …) from markdown body.
function extractH1(body: string): string | null {
  const m = body.match(/^#[ \t]+(.+?)[ \t]*$/m);
  return m ? m[1] : null;
}

// Convert a kebab-case path segment to a readable label as a last resort.
function idToLabel(id: string): string {
  return (id.split('/').pop() ?? id)
    .replace(/-/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/**
 * Load all Markdown files under `base` (relative to the Astro project root),
 * pre-render them to HTML using Astro's own markdown processor, and inject a
 * `title` from the first H1 heading when no frontmatter title is present.
 *
 * filePath is intentionally omitted from store entries: Astro rejects paths
 * outside the project root. Hot-reload in dev will not track changes to files
 * in docs/; restart `npm run dev` to pick up edits.
 */
export function docsFromPath(base: string = DOCS_BASE): Loader {
  return {
    name: 'yidam-docs',

    async load(context: LoaderContext) {
      const { store, parseData, config } = context;

      const absBase = resolveFrom(config.root, base);
      const repoRoot = resolveFrom(config.root, REPO_BASE);
      const published = new Set(publishedIds(absBase));
      const files = findMdFiles(absBase).filter((f) => published.has(docId(f, absBase)));

      // Create a single shared markdown processor for all files. The link
      // rewriter needs to know which file it is rendering, which is why each
      // render below passes the page id through the frontmatter channel: the
      // processor is shared, the context is not.
      const processor = await createMarkdownProcessor({
        gfm: true,
        smartypants: false,
        syntaxHighlight: 'shiki',
        shikiConfig: { theme: 'github-dark' },
        rehypePlugins: [
          rewriteRepoLinks({
            repoRoot,
            docsDir: absBase,
            published,
            base: config.base,
            repoUrl: REPO_URL,
            branch: BRANCH,
          }),
        ],
      });

      await Promise.all(
        files.map(async (filePath) => {
          const id = docId(filePath, absBase);

          const raw = await readFile(filePath, 'utf-8');
          const { data, body } = parseFrontmatter(raw);

          if (!data.title) {
            data.title = extractH1(body) ?? idToLabel(id);
          }

          const { code: html, metadata } = await processor.render(body, {
            frontmatter: { [SOURCE_ID]: id },
          });

          const parsed = await parseData({ id, data, filePath });

          store.set({
            id,
            data: parsed,
            body,
            rendered: {
              html,
              metadata: {
                headings: metadata.headings,
                imagePaths: [],
                frontmatter: data,
              },
            },
          });
        }),
      );
    },
  };
}
