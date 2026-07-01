import { readFile, readdir } from 'node:fs/promises';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createMarkdownProcessor } from '@astrojs/markdown-remark';
import type { Loader, LoaderContext } from 'astro/loaders';

// Find all .md files under dir, recursively.
async function findMdFiles(dir: string): Promise<string[]> {
  const results: string[] = [];
  async function walk(current: string) {
    const entries = await readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const full = join(current, entry.name);
      if (entry.isDirectory()) {
        await walk(full);
      } else if (entry.isFile() && entry.name.endsWith('.md')) {
        results.push(full);
      }
    }
  }
  await walk(dir);
  return results;
}

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
export function docsFromPath(base: string): Loader {
  return {
    name: 'yidam-docs',

    async load(context: LoaderContext) {
      const { store, parseData, config } = context;

      const absBase = fileURLToPath(new URL(base, config.root));
      const files = await findMdFiles(absBase);

      // Create a single shared markdown processor for all files.
      const processor = await createMarkdownProcessor({
        gfm: true,
        smartypants: false,
        syntaxHighlight: 'shiki',
        shikiConfig: { theme: 'github-dark' },
      });

      await Promise.all(
        files.map(async (filePath) => {
          const id = relative(absBase, filePath).replace(/\.md$/, '');

          const raw = await readFile(filePath, 'utf-8');
          const { data, body } = parseFrontmatter(raw);

          if (!data.title) {
            data.title = extractH1(body) ?? idToLabel(id);
          }

          const { code: html, metadata } = await processor.render(body);

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
