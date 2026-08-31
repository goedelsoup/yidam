import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { DOCS_BASE, publishedIds, resolveFrom } from './src/content/docs-source.ts';

// GitHub Pages serves this repository's site from a subpath, so `base` is part of
// every URL Starlight generates and part of every link the loader rewrites. `site`
// is what makes the sitemap and the canonical URLs absolute; without it Astro
// skips the sitemap entirely, which it did for as long as this site had no host.
const SITE = 'https://goedelsoup.github.io';
const BASE = '/yidam';

const sidebar = [
  // The site's root redirects to `what-yidam-is`, so it leads here: a reader who
  // typed the bare URL is already on the first page of this group.
  {
    label: 'Start here',
    items: [
      { slug: 'what-yidam-is', label: 'What yidam is' },
      { slug: 'quickstart', label: 'Quickstart' },
      { slug: 'installation', label: 'Installation' },
    ],
  },
  // Before the reference, and after the install: a reader who has not decided yet
  // is answering "what would mine look like", and until #447 the only corpus this
  // repository shipped was hydrology. The sidebar ran model → reference → model
  // with no tier for somebody still deciding.
  {
    label: 'Walkthroughs',
    items: [
      { slug: 'walkthroughs/property-research', label: 'Property research' },
      { slug: 'walkthroughs/investigative-journalism', label: 'Investigative journalism' },
      { slug: 'walkthroughs/incident-retrospectives', label: 'Incident retrospectives' },
      { slug: 'walkthroughs/genealogy', label: 'Genealogy (sketch)' },
      { slug: 'walkthroughs/museum-provenance', label: 'Museum provenance (sketch)' },
      { slug: 'walkthroughs/language-documentation', label: 'Language documentation (sketch)' },
    ],
  },
  // Reference before concept, deliberately. Someone who has installed the binary
  // has a question about the binary; the model can be read after, and is one
  // group down.
  {
    label: 'Using yidam',
    items: [
      { slug: 'cli-reference', label: 'CLI reference' },
      { slug: 'configuration', label: 'Configuration' },
      { slug: 'editor-setup', label: 'Editor setup' },
      { slug: 'mcp-server', label: 'Connecting an agent (MCP)' },
      { slug: 'artifact-vaults', label: 'Artifact vaults' },
      { slug: 'sharing-derivations', label: 'Sharing a derivation' },
      { slug: 'troubleshooting', label: 'Troubleshooting' },
    ],
  },
  {
    label: 'The model',
    items: [
      { slug: 'vocabulary', label: 'Vocabulary' },
      { slug: 'information-architecture', label: 'Information architecture' },
      { slug: 'git-branch-model', label: 'Git branch model' },
      { slug: 'bootstrap-flow', label: 'Bootstrap flow' },
      { slug: 'domain-computer', label: 'Domain computer' },
      { slug: 'web-interface', label: 'Web interface' },
    ],
  },
  {
    label: 'Governance',
    items: [
      { slug: 'sangha-resolution-flow', label: 'Sangha resolution' },
      { slug: 'constitutional-governance', label: 'Constitutional governance' },
      { slug: 'conduct-norms', label: 'Conduct norms' },
    ],
  },
  {
    label: 'Quality',
    items: [
      { slug: 'quality-rubric', label: 'Quality rubric' },
      { slug: 'test-harness', label: 'Test harness' },
      { slug: 'post-genesis-measurement', label: 'Post-genesis measurement' },
    ],
  },
  {
    label: 'The project',
    items: [
      { slug: 'contributing', label: 'Contributing' },
      { slug: 'versioning', label: 'Versioning and releases' },
      { slug: 'aesthetic-direction', label: 'Aesthetic direction' },
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
  {
    // Working documents, not user documentation: eighteen files, and by volume
    // nearly two thirds of everything under docs/. They get their own collapsed
    // section, last, rather than a place in the main flow — a reader arriving to
    // find out what yidam is should not have to scroll past a design backlog to
    // reach the answer. Labels are the index table's short titles, not the H1s,
    // which run to a full sentence each.
    label: 'RFCs',
    collapsed: true,
    items: [
      { slug: 'rfcs/README', label: 'Overview' },
      { slug: 'rfcs/0001-report-contract', label: '0001 · The report contract' },
      { slug: 'rfcs/0002-node-model-unification', label: '0002 · Node-model unification' },
      { slug: 'rfcs/0003-feature-gated-reports-binary', label: '0003 · Feature-gated builds' },
      { slug: 'rfcs/0004-drift-detection', label: '0004 · Drift detection' },
      { slug: 'rfcs/0005-mcp-tool-contract', label: '0005 · One MCP tool contract' },
      { slug: 'rfcs/0006-correctness-reconciliation', label: '0006 · Correctness reconciliation' },
      { slug: 'rfcs/0007-python-index-layer', label: '0007 · The Python index layer' },
      { slug: 'rfcs/0008-emergent-claims', label: '0008 · Emergent claims' },
      { slug: 'rfcs/0009-resolution-executor', label: '0009 · Resolution execution' },
      { slug: 'rfcs/0010-evolution-lineage', label: '0010 · Evolution lineage' },
      { slug: 'rfcs/0011-partial-sangha', label: '0011 · Partial-sangha resolutions' },
      { slug: 'rfcs/0012-elector-attestation', label: '0012 · Elector attestation' },
      { slug: 'rfcs/0013-node-model-close', label: '0013 · Closing the node model' },
      { slug: 'rfcs/0014-node-rename', label: '0014 · Node rename' },
      { slug: 'rfcs/0015-epistemic-log', label: '0015 · An epistemic log' },
      { slug: 'rfcs/0016-editor-surface', label: '0016 · The editor surface' },
      { slug: 'rfcs/0017-assertion-surface', label: '0017 · The assertion surface' },
      { slug: 'rfcs/0018-query-surface', label: '0018 · The query surface' },
      { slug: 'rfcs/0019-external-citation', label: '0019 · External citation' },
      { slug: 'rfcs/0020-proposal-surface', label: '0020 · The proposal surface' },
      { slug: 'rfcs/0021-diff-alignment', label: '0021 · Diff-to-ontology alignment' },
      { slug: 'rfcs/0022-semantic-alignment', label: '0022 · Semantic alignment' },
      { slug: 'rfcs/0023-remote-vaults', label: '0023 · Remote vaults' },
      { slug: 'rfcs/0024-policy-as-code', label: '0024 · Policy as code' },
      { slug: 'rfcs/0025-quality-surface', label: '0025 · The instrument, turned around' },
      { slug: 'rfcs/0026-orchestrator-layer', label: '0026 · The orchestrator layer' },
    ],
  },
];

// ── the sidebar is the only way in ───────────────────────────────────────────
// Starlight builds a page for every entry the loader returns, whether or not the
// sidebar names it. This site shipped seventeen RFCs that way: rendered, routed,
// and reachable by nobody who did not already know the URL — the same failure as
// the site itself having no host. An unlisted page is not published, so a page
// with no sidebar entry fails the build rather than becoming quietly invisible.
//
// It runs in both directions: an entry naming a page that no longer exists is a
// dead sidebar link, and Starlight's own error for that arrives later and says
// less.
function sidebarSlugs(items, found = new Set()) {
  for (const item of items) {
    if (item.slug) found.add(item.slug);
    if (item.items) sidebarSlugs(item.items, found);
  }
  return found;
}

const listed = sidebarSlugs(sidebar);
const built = publishedIds(resolveFrom(import.meta.url, DOCS_BASE));

const unlisted = built.filter((id) => !listed.has(id));
const stale = [...listed].filter((slug) => !built.includes(slug));

if (unlisted.length > 0 || stale.length > 0) {
  throw new Error(
    [
      unlisted.length > 0 &&
        `docs/ has ${unlisted.length} page(s) missing from the sidebar in astro.config.mjs:\n` +
          unlisted.map((id) => `  ${id}`).join('\n') +
          `\nAdd a sidebar entry, or add the id to UNPUBLISHED in src/content/docs-source.ts.`,
      stale.length > 0 &&
        `the sidebar names ${stale.length} page(s) that docs/ does not have:\n` +
          [...stale].map((s) => `  ${s}`).join('\n'),
    ]
      .filter(Boolean)
      .join('\n\n'),
  );
}

export default defineConfig({
  site: SITE,
  base: BASE,
  // Redirect the bare root to the first doc page. `base` is spelled out in the
  // destination because Astro applies it to the key and not to the value: the
  // generated page lands at /yidam/ and a bare '/what-yidam-is' would send every
  // arrival at the site's front door to goedelsoup.github.io/what-yidam-is, which
  // is a different site's 404.
  redirects: {
    '/': `${BASE}/what-yidam-is/`,
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
      sidebar,
    }),
  ],
});
