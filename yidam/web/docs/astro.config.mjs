import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  // Redirect the bare root to the first doc page.
  redirects: {
    '/': '/what-yidam-is',
  },
  integrations: [
    starlight({
      title: 'yidam',
      description: 'A git-native knowledge graph system for structured domain research.',
      logo: {
        src: './src/assets/logo-mark.svg',
        replacesTitle: false,
      },
      customCss: ['./src/styles/custom.css'],
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/goedelsoup/yidam',
        },
      ],
      // Suppress the auto-rendered page title so the markdown H1 is the sole heading.
      components: {
        PageTitle: './src/components/PageTitle.astro',
      },
      sidebar: [
        {
          label: 'Start here',
          items: [{ slug: 'quickstart', label: 'Quickstart' }],
        },
        {
          label: 'yidam',
          items: [
            { slug: 'what-yidam-is', label: 'What yidam is' },
            { slug: 'vocabulary', label: 'Vocabulary' },
            { slug: 'aesthetic-direction', label: 'Aesthetic direction' },
          ],
        },
        {
          label: 'Architecture',
          items: [
            { slug: 'information-architecture', label: 'Information architecture' },
            { slug: 'git-branch-model', label: 'Git branch model' },
            { slug: 'domain-computer', label: 'Domain computer' },
            { slug: 'web-interface', label: 'Web interface' },
          ],
        },
        {
          label: 'Processes',
          items: [
            { slug: 'bootstrap-flow', label: 'Bootstrap flow' },
            { slug: 'sangha-resolution-flow', label: 'Sangha resolution' },
            { slug: 'constitutional-governance', label: 'Constitutional governance' },
            { slug: 'sharing-derivations', label: 'Sharing a derivation' },
          ],
        },
        {
          label: 'Quality & conduct',
          items: [
            { slug: 'quality-rubric', label: 'Quality rubric' },
            { slug: 'conduct-norms', label: 'Conduct norms' },
            { slug: 'test-harness', label: 'Test harness' },
          ],
        },
        {
          label: 'Research',
          items: [
            {
              label: 'Ontology-anchored path resolution',
              items: [
                { slug: 'research/system/README', label: 'Overview' },
                { slug: 'research/system/outline', label: 'Outline' },
                {
                  label: 'Notes',
                  collapsed: true,
                  items: [
                    { slug: 'research/system/notes/yidam-case', label: 'yidam case study' },
                    { slug: 'research/system/notes/traversal-cost', label: 'Traversal cost' },
                    { slug: 'research/system/notes/ontology-maps', label: 'Ontology maps' },
                    { slug: 'research/system/notes/focused-scan', label: 'Focused scan' },
                    { slug: 'research/system/notes/efficiency', label: 'Efficiency analysis' },
                  ],
                },
              ],
            },
          ],
        },
      ],
    }),
  ],
});
