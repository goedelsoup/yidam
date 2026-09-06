import { defineCollection } from 'astro:content';
import { docsSchema } from '@astrojs/starlight/schema';
// `src/content.config.ts` rather than `src/content/config.ts`: Astro 6 removed the legacy
// location and refuses to build while a file sits there, so the collection definition now
// lives one directory up from the loader it names. Everything under `src/content/` stays
// put — the loader reads `docs/` at the repository root and resolves it against the Astro
// project root, not against this file, so the move costs one import specifier and nothing
// else.
import { docsFromPath } from './content/docs-loader.ts';

export const collections = {
  docs: defineCollection({
    // Load all Markdown files from docs/ at the repo root.
    // base is relative to this project root (yidam/web/docs/).
    loader: docsFromPath('../../../docs'),
    schema: docsSchema(),
  }),
};
