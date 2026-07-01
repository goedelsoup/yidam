import { defineCollection } from 'astro:content';
import { docsSchema } from '@astrojs/starlight/schema';
import { docsFromPath } from './docs-loader';

export const collections = {
  docs: defineCollection({
    // Load all Markdown files from docs/ at the repo root.
    // base is relative to this project root (yidam/web/docs/).
    loader: docsFromPath('../../../docs'),
    schema: docsSchema(),
  }),
};
