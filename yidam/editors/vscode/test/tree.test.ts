/**
 * The five views, asserted as data.
 *
 * Two halves. The first builds trees from literal reports, so each shape decision is
 * pinned in isolation. The second runs the *real binary* over the golden corpus and feeds
 * its actual JSON to the same builders — which is the half that catches a field the CLI
 * renamed, and the half a hand-written fixture can never catch.
 *
 * No editor is involved in either.
 */

import assert from 'node:assert/strict'
import { test } from 'node:test'

import { capture, contractBinary, SKIP, stageFixture } from './stage.ts'
import type {
  CatalogAuditReport,
  CorpusIndexReport,
  DoctorReport,
  GraphCheckReport,
  IndexStatusReport,
  LintReport,
  OpenQuestionsReport,
  PhasesReport,
  RegenReport,
  SanghaReport,
  StatusReport,
} from '../src/reports.ts'
import {
  corpusTree,
  countLeaves,
  countNodes,
  filterMessage,
  findByFile,
  healthTree,
  matches,
  parentIndex,
  localRef,
  openQuestionsTree,
  parseFilter,
  phasesTree,
  sanghaTree,
  sourcesByNode,
  statusLine,
  type TreeNode,
} from '../src/tree/model.ts'

const ENVELOPE = {
  format_version: '1',
  yidam: { version: '0.1.0', commit: 'abc1234', features: ['reports'] },
  root: '/r',
}

function find(nodes: TreeNode[], id: string): TreeNode | undefined {
  for (const n of nodes) {
    if (n.id === id) return n
    const hit = n.children ? find(n.children, id) : undefined
    if (hit) return hit
  }
  return undefined
}

function ids(nodes: TreeNode[]): string[] {
  return nodes.map((n) => n.id)
}

// ── corpus ──────────────────────────────────────────────────────────────────

const INDEX: CorpusIndexReport = {
  ...ENVELOPE,
  nodes: [
    {
      node: 'concept/tailwater.yml',
      class: 'concept',
      label: 'Tailwater',
      links_out: 1,
      claims_verified: 0,
      claims_inference: 1,
      claims_open: 0,
      lines: 6,
    },
    {
      node: 'concept/low-flow.yml',
      class: 'concept',
      label: 'Low flow',
      links_out: 1,
      claims_verified: 0,
      claims_inference: 0,
      claims_open: 0,
      lines: 6,
    },
    {
      node: 'gauge/ohio-river.yml',
      class: 'gauge',
      label: '—',
      links_out: 0,
      claims_verified: 0,
      claims_inference: 0,
      claims_open: 1,
      lines: 4,
    },
  ],
}

test('the corpus groups by class and sorts both levels', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] })
  assert.deepEqual(ids(tree), ['class:concept', 'class:gauge'])
  assert.deepEqual(ids(tree[0].children!), ['node:concept/low-flow.yml', 'node:concept/tailwater.yml'])
  assert.equal(tree[0].description, '2')
})

test('a node with no label falls back to its filename rather than showing an em dash', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] })
  assert.equal(find(tree, 'node:gauge/ohio-river.yml')!.label, 'ohio-river')
})

/**
 * The two reports index their nodes against different roots — `corpus-index` against the
 * corpus, `open-questions` against the repository. Matching on suffix is what keeps a
 * repository that moves its corpus from silently losing every mark.
 */
test('open questions are marked across the two reports’ different roots', () => {
  const open: OpenQuestionsReport = {
    ...ENVELOPE,
    open_questions: [{ node: '.yidam/corpus/gauge/ohio-river.yml', label: '?Which datum' }],
  }
  const tree = corpusTree(INDEX, open)
  assert.equal(find(tree, 'node:gauge/ohio-river.yml')!.icon, 'question')
  assert.equal(find(tree, 'node:gauge/ohio-river.yml')!.description, 'open')
  assert.equal(find(tree, 'node:concept/tailwater.yml')!.icon, 'symbol-field')
})

test('a corpus node opens the file the corpus dir actually holds', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] })
  assert.equal(find(tree, 'node:concept/tailwater.yml')!.file, '.yidam/corpus/concept/tailwater.yml')
})

test('no open questions is a stated answer, not an empty box', () => {
  const tree = openQuestionsTree({ ...ENVELOPE, open_questions: [] })
  assert.equal(tree.length, 1)
  assert.equal(tree[0].icon, 'check')
})

// ── provenance ──────────────────────────────────────────────────────────────

const CATALOG: CatalogAuditReport = {
  ...ENVELOPE,
  sources: [
    {
      entry: 'stage-discharge.md',
      type: 'paper',
      description: 'A rating-curve derivation',
      obtained: true,
      citations: 2,
      nodes: 2,
      elsewhere: 0,
      // Repository-relative, which is how `catalog-audit` spells it — `corpus-index` spells
      // the same nodes relative to the corpus.
      cited_by: ['.yidam/corpus/concept/tailwater.yml', '.yidam/corpus/concept/low-flow.yml'],
      used_by: [],
      drift: null,
    },
    {
      entry: 'gauge-record.md',
      type: 'dataset',
      description: 'A stream gauge series',
      obtained: false,
      citations: 1,
      nodes: 1,
      elsewhere: 0,
      cited_by: ['.yidam/corpus/concept/tailwater.yml'],
      used_by: ['mixing-zone.yml'],
      drift: { claimed_not_citing: ['mixing-zone.yml'], citing_not_claimed: ['tailwater.yml'] },
    },
  ],
}

/**
 * The two reports index nodes against different roots, the same mismatch `openSet` closes.
 * Matched on suffix rather than by prefixing a hardcoded corpus dir, so a repository that
 * moves its corpus keeps its provenance.
 */
test('sources are inverted onto their citing nodes across the two reports’ roots', () => {
  const by = sourcesByNode(CATALOG, INDEX.nodes)
  assert.deepEqual(
    by.get('concept/tailwater.yml')!.map((s) => s.entry),
    ['gauge-record.md', 'stage-discharge.md'],
    'sorted, so the rows do not reorder between refreshes',
  )
  assert.deepEqual(by.get('concept/low-flow.yml')!.map((s) => s.entry), ['stage-discharge.md'])
  assert.equal(by.get('gauge/ohio-river.yml'), undefined, 'a node citing nothing has no entry')
})

test('no catalog report means no sources rather than an empty map of them', () => {
  assert.equal(sourcesByNode(null, INDEX.nodes).size, 0)
})

test('a node carries the sources it draws on, and clicking one opens the entry', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] }, undefined, null, CATALOG)
  const node = find(tree, 'node:concept/tailwater.yml')!
  assert.deepEqual(ids(node.children!), [
    'source:concept/tailwater.yml:gauge-record.md',
    'source:concept/tailwater.yml:stage-discharge.md',
  ])
  assert.equal(
    find(tree, 'source:concept/tailwater.yml:gauge-record.md')!.file,
    '.yidam/catalog/gauge-record.md',
  )
})

/**
 * Most nodes cite nothing. An arrow beside every one of them is an arrow that is usually a
 * lie about there being something behind it.
 */
test('a node with no sources gets no children rather than an empty group', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] }, undefined, null, CATALOG)
  assert.deepEqual(find(tree, 'node:gauge/ohio-river.yml')!.children, [])
})

/**
 * A CLI that predates the provenance fields.
 *
 * There is no version the extension could refuse here. Adding a field is not a contract
 * break, so such a CLI still reports `format_version` 1 and the handshake passes — the one
 * check built for exactly this question cannot see the difference. And the pairing is
 * ordinary rather than exotic: the extension updates itself from Open VSX, while a
 * repository builds its binary from the commit pinned in `.yidam.toml`.
 *
 * So it must degrade to "no sources known", which is true, and never to a broken view.
 */
test('a CLI older than the provenance fields yields no sources rather than throwing', () => {
  const old: CatalogAuditReport = {
    ...ENVELOPE,
    sources: [
      {
        entry: 'stage-discharge.md',
        type: 'paper',
        description: 'A rating-curve derivation',
        obtained: true,
        citations: 2,
        nodes: 2,
        elsewhere: 0,
      },
    ],
  }
  assert.equal(sourcesByNode(old, INDEX.nodes).size, 0, 'no citation is resolvable')
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] }, undefined, null, old)
  assert.deepEqual(find(tree, 'node:concept/tailwater.yml')!.children, [])
})

/**
 * `drift` is guarded separately from `cited_by` because it is read on a different path —
 * the row's tooltip rather than the inversion. Unreachable from any released CLI, which
 * shipped all three fields together; covered because the type now permits it, and an
 * untested guard is the one that regresses.
 */
test('a source whose drift is absent renders without claiming its used-by disagrees', () => {
  const partial: CatalogAuditReport = {
    ...ENVELOPE,
    sources: [
      {
        entry: 'stage-discharge.md',
        type: 'paper',
        description: 'A rating-curve derivation',
        obtained: true,
        citations: 1,
        nodes: 1,
        elsewhere: 0,
        cited_by: ['.yidam/corpus/concept/tailwater.yml'],
      },
    ],
  }
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] }, undefined, null, partial)
  const row = find(tree, 'source:concept/tailwater.yml:stage-discharge.md')!
  assert.ok(!row.tooltip!.includes('disagrees'), 'no list was declared, so nothing drifted')
})

/**
 * A node citing an unretrieved source is `catalog-unobtained-but-cited`, an Error that
 * reaches the editor as a diagnostic and a Health row. A second red mark here would be the
 * tree rendering a verdict it did not compute.
 */
test('an unobtained source says so and is not dressed as a failure', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] }, undefined, null, CATALOG)
  const row = find(tree, 'source:concept/tailwater.yml:gauge-record.md')!
  assert.match(row.description!, /not obtained/)
  assert.notEqual(row.icon, 'error')
  assert.notEqual(row.icon, 'warning')
})

test('a source cited by two nodes appears under both', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] }, undefined, null, CATALOG)
  assert.ok(find(tree, 'source:concept/tailwater.yml:stage-discharge.md'))
  assert.ok(find(tree, 'source:concept/low-flow.yml:stage-discharge.md'))
})

/**
 * The regression sources introduced: a leaf in the Corpus view is a *source* now, so a
 * filter counting leaves would report `4 of 3` — more shown than the corpus holds.
 */
test('the corpus counts nodes, not the sources hanging off them', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] }, undefined, null, CATALOG)
  assert.equal(countNodes(tree), 3, 'three nodes')
  assert.equal(countLeaves(tree), 4, 'and four leaves, which is why the two are different')
})

// ── narrowing ───────────────────────────────────────────────────────────────

const OPEN_ONE: OpenQuestionsReport = {
  ...ENVELOPE,
  open_questions: [
    { node: '.yidam/corpus/gauge/ohio-river.yml', label: '?Which datum' },
    { node: '.yidam/corpus/concept/tailwater.yml', label: 'Tailwater' },
  ],
}

/** An empty query is no filter at all, which is not the same as one matching everything. */
test('an empty query parses to no filter, and no filter keeps every row', () => {
  assert.equal(parseFilter(''), null)
  assert.equal(parseFilter('   '), null)
  assert.equal(matches(null, { label: 'x', node: 'y/x.yml', open: false }), true)
})

test('free text matches the label or the node path, either case', () => {
  const f = parseFilter('TAIL')!
  assert.equal(matches(f, { label: 'Tailwater', node: 'concept/other.yml', open: false }), true)
  assert.equal(matches(f, { label: 'Other', node: 'concept/tailwater.yml', open: false }), true)
  assert.equal(matches(f, { label: 'Other', node: 'concept/low-flow.yml', open: false }), false)
})

/** Two words are one narrower question, not two alternatives. */
test('every free term must match', () => {
  const f = parseFilter('low flow')!
  assert.equal(matches(f, { label: 'Low flow', node: 'concept/low-flow.yml', open: false }), true)
  assert.equal(matches(f, { label: 'Low water', node: 'concept/low.yml', open: false }), false)
})

/** Two classes are alternatives: narrowing to a pair of them is still one question. */
test('classes are alternatives to each other and conjunctive with the text', () => {
  const f = parseFilter('class:gauge class:concept river')!
  assert.equal(matches(f, { label: 'Ohio river', node: 'a.yml', class: 'gauge', open: false }), true)
  assert.equal(matches(f, { label: 'Ohio river', node: 'a.yml', class: 'reach', open: false }), false)
  assert.equal(matches(f, { label: 'Tailwater', node: 'a.yml', class: 'concept', open: false }), false)
})

/**
 * `open-questions` does not carry a class, and the `<class>/<name>.yml` layout is a
 * convention rather than a fact. A row whose class nothing in hand states matches no
 * `class:` term rather than being guessed at from its path.
 */
test('a row with no class stated matches no class term', () => {
  const f = parseFilter('class:gauge')!
  assert.equal(matches(f, { label: 'x', node: 'gauge/x.yml', open: false }), false)
})

test('is:open keeps only the rows a report marked open', () => {
  const f = parseFilter('is:open')!
  assert.equal(matches(f, { label: 'x', node: 'a.yml', open: true }), true)
  assert.equal(matches(f, { label: 'x', node: 'a.yml', open: false }), false)
})

test('the corpus narrows, and a class the filter emptied is dropped rather than shown empty', () => {
  const tree = corpusTree(INDEX, OPEN_ONE, undefined, parseFilter('class:concept')!)
  assert.deepEqual(ids(tree), ['class:concept'])
  assert.deepEqual(ids(tree[0].children!), [
    'node:concept/low-flow.yml',
    'node:concept/tailwater.yml',
  ])
})

/**
 * A bare `1` under a class of two reads as a class of one, which is the filter lying about
 * the corpus rather than about what it is showing of it.
 */
test('a narrowed class states both numbers', () => {
  const tree = corpusTree(INDEX, OPEN_ONE, undefined, parseFilter('tailwater')!)
  assert.equal(find(tree, 'class:concept')!.description, '1 of 2')
  assert.equal(corpusTree(INDEX, OPEN_ONE)[0].description, '2', 'unfiltered says just the count')
})

/** The two questions VS Code's own type-to-filter cannot ask, both answered from the reports. */
test('is:open narrows the corpus to the nodes the open-questions report marks', () => {
  const tree = corpusTree(INDEX, OPEN_ONE, undefined, parseFilter('is:open')!)
  assert.deepEqual(ids(tree), ['class:concept', 'class:gauge'])
  assert.deepEqual(ids(tree[0].children!), ['node:concept/tailwater.yml'])
  assert.equal(countLeaves(tree), 2)
})

/** The rendered label is what is on the screen, so typing it has to match. */
test('a node with no label is matched by the filename the view shows for it', () => {
  const tree = corpusTree(INDEX, OPEN_ONE, undefined, parseFilter('ohio')!)
  assert.deepEqual(ids(tree), ['class:gauge'])
})

test('open questions narrow on their own labels and paths', () => {
  const tree = openQuestionsTree(OPEN_ONE, parseFilter('datum')!)
  assert.deepEqual(ids(tree), ['open:.yidam/corpus/gauge/ohio-river.yml'])
})

/**
 * `class:` on the Open questions view is answered by `corpus-index`, which is the authority
 * on what class a node is in — not by reading the path, which would be this file forming a
 * second opinion about it.
 */
test('class: narrows open questions only when the index is in hand', () => {
  const f = parseFilter('class:gauge')!
  assert.deepEqual(ids(openQuestionsTree(OPEN_ONE, f, INDEX)), [
    'open:.yidam/corpus/gauge/ohio-river.yml',
  ])
  assert.deepEqual(openQuestionsTree(OPEN_ONE, f), [], 'no index, so nothing states a class')
})

/**
 * A tick reading `No open questions` under an active filter would be the view
 * congratulating a repository on a state the filter invented.
 */
test('a filter that matches nothing is empty, not a clean bill of health', () => {
  assert.deepEqual(openQuestionsTree(OPEN_ONE, parseFilter('zzz')!), [])
  assert.deepEqual(corpusTree(INDEX, OPEN_ONE, undefined, parseFilter('zzz')!), [])
  assert.deepEqual(ids(openQuestionsTree({ ...ENVELOPE, open_questions: [] })), ['open:none'])
})

/** Rows, not groups: what a reader counts when asking how much a filter left. */
test('leaves are counted through the groups', () => {
  assert.equal(countLeaves(corpusTree(INDEX, OPEN_ONE)), 3)
  assert.equal(countLeaves([]), 0)
})

/**
 * A view that hides rows without saying it is hiding them is how a reader concludes a node
 * was deleted.
 */
test('a narrowed view says so, and says how much it is not showing', () => {
  assert.equal(filterMessage(null, 3, 3), undefined)
  assert.equal(filterMessage(parseFilter('gauge')!, 1, 90), 'filter: gauge — 1 of 90')
  assert.equal(
    filterMessage(parseFilter('zzz')!, 0, 90),
    'filter: zzz — nothing matches, 90 hidden',
  )
})

// ── phases ──────────────────────────────────────────────────────────────────

const PHASES: PhasesReport = {
  ...ENVELOPE,
  phases: [
    { name: 'Auditor', ref_name: 'ma/auditor', owner: 'Tester', started: '2026-01-01', commits: 3 },
    {
      name: 'Advocate',
      ref_name: 'origin/ma/advocate',
      owner: 'Tester',
      started: '2026-01-02',
      commits: 1,
    },
    {
      name: 'Schema reach',
      ref_name: 'rigpa/schema-reach',
      owner: 'Tester',
      started: '2026-01-03',
      commits: 0,
    },
  ],
}

test('phases split by namespace, remote refs included', () => {
  const tree = phasesTree(PHASES)
  assert.deepEqual(ids(tree), ['phases:ma', 'phases:rigpa'])
  assert.deepEqual(ids(tree[0].children!), ['phase:ma/auditor', 'phase:origin/ma/advocate'])
  assert.equal(tree[0].expanded, true, 'positions are the half a reader is looking for')
})

/**
 * A ref neither namespace claims is shown rather than dropped.
 *
 * The CLI is the authority on what a phase is. A grouping that silently swallows a row it
 * did not expect is how a view starts disagreeing with the command behind it — and it
 * disagrees invisibly, which is the worst version.
 */
test('a phase in neither namespace still appears', () => {
  const tree = phasesTree({
    ...ENVELOPE,
    phases: [{ name: 'Odd', ref_name: 'wip/odd', owner: 'x', started: '—', commits: 0 }],
  })
  assert.deepEqual(ids(tree), ['phases:other'])
})

/**
 * `phase/*` is the namespace PHASES.md defines a phase in, and it had no group — every one
 * fell into `Other` alongside genuinely unclassifiable refs. The CLI carried the same defect
 * in `active_phase_count`, so both readers of the model looked past the same namespace.
 */
test('the phase namespace has its own group, not Other', () => {
  const tree = phasesTree({
    ...ENVELOPE,
    phases: [
      { name: 'Outcome axis', ref_name: 'phase/outcome-axis', owner: 'x', started: '—', commits: 3 },
    ],
  })
  assert.deepEqual(ids(tree), ['phases:phase'])
  assert.deepEqual(ids(tree[0].children!), ['phase:phase/outcome-axis'])
})

/**
 * A settled ref says so. Active is the expectation and `position` is already the group's
 * name, so neither is stated — but a ref that outlived its settlement is the one thing in
 * this view a reader can act on, and the count that hid it is what this whole change is about.
 */
test('settledness is the only state the row states', () => {
  const tree = phasesTree({
    ...ENVELOPE,
    phases: [
      { name: 'Done', ref_name: 'phase/done', owner: 'x', started: '—', commits: 2, state: 'settled' },
      { name: 'Live', ref_name: 'phase/live', owner: 'x', started: '—', commits: 1, state: 'active' },
    ],
  })
  const rows = tree[0].children!
  assert.equal(rows.find((r) => r.id === 'phase:phase/done')!.description, '2 commit(s) · settled')
  assert.equal(rows.find((r) => r.id === 'phase:phase/live')!.description, '1 commit(s)')
})

/** A binary older than the `state` field omits it; the view must not render `undefined`. */
test('a report without state still renders', () => {
  const tree = phasesTree({
    ...ENVELOPE,
    phases: [{ name: 'Old', ref_name: 'phase/old', owner: 'x', started: '—', commits: 4 }],
  })
  assert.equal(tree[0].children![0].description, '4 commit(s)')
})

test('no phases says so', () => {
  assert.equal(phasesTree({ ...ENVELOPE, phases: [] })[0].id, 'phases:none')
})

/**
 * `git switch origin/ma/auditor` detaches HEAD — the one outcome a click on a branch row
 * must not produce.
 */
test('a remote-only phase switches to the local name git would create', () => {
  assert.equal(localRef('origin/ma/advocate'), 'ma/advocate')
  assert.equal(localRef('upstream/rigpa/schema-reach'), 'rigpa/schema-reach')
  assert.equal(localRef('ma/auditor'), 'ma/auditor')
  // Not a phase ref at all: left alone rather than mangled.
  assert.equal(localRef('feature/thing'), 'feature/thing')
})

// ── health ──────────────────────────────────────────────────────────────────

function lintReport(over: Partial<LintReport['gate']> = {}): LintReport {
  return {
    ...ENVELOPE,
    gate: {
      passed: true,
      new_violations: 0,
      baselined_violations: 0,
      stale_baseline_entries: [],
      ...over,
    },
    checks: [],
  }
}

const GRAPH: GraphCheckReport = {
  ...ENVELOPE,
  passed: true,
  corpus_empty: false,
  total_instances: 3,
  clean_instances: 3,
  classes_defined: 2,
  nodes_with_issues: [],
  classes_without_instances: [],
}

const REGEN: RegenReport = { ...ENVELOPE, passed: true, stale: [] }

const INDEX_STATUS: IndexStatusReport = {
  ...ENVELOPE,
  index_present: true,
  meta_present: true,
  built_at: 1767225600,
  built: '2026-01-01',
  model: 'Xenova/all-MiniLM-L6-v2',
  embedding_dim: 384,
  node_count: 3,
  stale_nodes: 0,
}

const DOCTOR: DoctorReport = {
  ...ENVELOPE,
  passed: true,
  strict: false,
  failed: 0,
  warned: 0,
  checks: [
    { id: 'repository', question: 'Am I in a derived repository?', verdict: 'ok', detail: '/r', remedy: null },
  ],
}

/** The four gates, the one act, and the precondition that comes before all of them. */
test('health carries all six rows, gates and the one act alike', () => {
  const tree = healthTree({
    lint: lintReport(),
    graph: GRAPH,
    index: INDEX_STATUS,
    regen: REGEN,
    doctor: DOCTOR,
  })
  assert.deepEqual(ids(tree), [
    'health:doctor',
    'health:graph',
    'health:lint',
    'health:index',
    'health:regen',
    'health:vendor',
  ])
})

/**
 * Setup is a precondition, not a gate — and the distinction is the icon.
 *
 * A light `reports` install legitimately has no vector index, so `doctor` warns on a normal
 * state. Rendering that red is how a reader learns to ignore the row; only `fail` is wrong
 * now, which is the same split `doctor`'s own exit code makes.
 */
test('a warning setup is not rendered as a failure, and a failing one is', () => {
  const warned = healthTree({
    lint: lintReport(),
    graph: GRAPH,
    index: INDEX_STATUS,
    regen: REGEN,
    doctor: {
      ...DOCTOR,
      warned: 1,
      checks: [
        ...DOCTOR.checks,
        { id: 'index', question: 'Is the index built?', verdict: 'warn', detail: 'no index', remedy: 'yidam index-build' },
      ],
    },
  })
  const row = find(warned, 'health:doctor')!
  assert.equal(row.icon, 'warning')
  assert.equal(row.description, '1 warning(s)')

  const failed = healthTree({
    lint: lintReport(),
    graph: GRAPH,
    index: INDEX_STATUS,
    regen: REGEN,
    doctor: {
      ...DOCTOR,
      passed: false,
      failed: 1,
      checks: [
        ...DOCTOR.checks,
        { id: 'provenance', question: 'Does this repository record where it came from?', verdict: 'fail', detail: 'no .yidam.toml', remedy: 'mise run yidam-vendor-update' },
      ],
    },
  })
  const bad = find(failed, 'health:doctor')!
  assert.equal(bad.icon, 'error')
  assert.equal(bad.expanded, true, 'a failing precondition opens itself')
  assert.deepEqual(ids(bad.children!), ['health:doctor:provenance'])
})

/**
 * `doctor` asks nine questions and eight of them are usually yes. Listing those is a wall
 * of ticks that buries the one that is not.
 */
test('only the setup checks a reader can act on get rows', () => {
  const tree = healthTree({
    lint: lintReport(),
    graph: GRAPH,
    index: INDEX_STATUS,
    regen: REGEN,
    doctor: {
      ...DOCTOR,
      checks: [
        ...DOCTOR.checks,
        { id: 'skipped', question: 'Unanswerable', verdict: 'skipped', detail: '—', remedy: null },
      ],
    },
  })
  assert.deepEqual(find(tree, 'health:doctor')!.children, [])
})

/**
 * A remedy is a shell command the report chose. Stating one is rendering; running one is a
 * capability, and this row does not have it.
 */
test('a setup remedy is stated and never offered as a click', () => {
  const tree = healthTree({
    lint: lintReport(),
    graph: GRAPH,
    index: INDEX_STATUS,
    regen: REGEN,
    doctor: {
      ...DOCTOR,
      passed: false,
      failed: 1,
      checks: [
        { id: 'provenance', question: 'Where from?', verdict: 'fail', detail: 'no .yidam.toml', remedy: 'mise run yidam-vendor-update' },
      ],
    },
  })
  const row = find(tree, 'health:doctor:provenance')!
  assert.match(row.tooltip!, /Remedy: mise run yidam-vendor-update/)
  assert.equal(row.command, undefined, 'the extension runs only the tasks its own manifest names')
})

/**
 * Vendor drift needs the network, so it stays an act — and must not render as passing.
 *
 * A row claiming the prelude is current without asking the origin would be this extension
 * asserting something no command answered, which is the drift the contract exists to close.
 * REGEN was in this test until `yidam regen --check` gave it a verdict; the next test is
 * what replaced it.
 */
test('vendor drift is offered as an act, never as a verdict', () => {
  const row = find(
    healthTree({ lint: lintReport(), graph: GRAPH, index: INDEX_STATUS, regen: REGEN, doctor: DOCTOR }),
    'health:vendor',
  )!
  assert.notEqual(row.icon, 'pass')
  assert.notEqual(row.icon, 'error')
  assert.ok(row.command, 'it is a thing to run')
})

/** REGEN freshness became a gate the moment a command could answer it without writing. */
test('a stale REGEN block is a verdict, and names which generator', () => {
  const stale: RegenReport = {
    ...ENVELOPE,
    passed: false,
    stale: [{ file: '.yidam/corpus/README.md', generator: 'corpus-index' }],
  }
  const row = find(
    healthTree({ lint: lintReport(), graph: GRAPH, index: INDEX_STATUS, regen: stale, doctor: DOCTOR }),
    'health:regen',
  )!
  assert.equal(row.icon, 'warning')
  assert.equal(row.description, '1 stale')
  assert.equal(row.expanded, true)
  assert.equal(row.children![0].file, '.yidam/corpus/README.md')
  assert.equal(row.children![0].description, 'corpus-index')

  const current = find(
    healthTree({ lint: lintReport(), graph: GRAPH, index: INDEX_STATUS, regen: REGEN, doctor: DOCTOR }),
    'health:regen',
  )!
  assert.equal(current.icon, 'pass')
  assert.equal(current.description, 'current')
  // Regenerating a current block is a no-op, so unlike blessing a baseline the action is
  // never the wrong thing to click.
  assert.equal(current.command?.id, 'yidam.regen')
})

test('a failing graph gate expands and lists its offending nodes', () => {
  const graph: GraphCheckReport = {
    ...GRAPH,
    passed: false,
    clean_instances: 2,
    nodes_with_issues: [{ node: '.yidam/corpus/concept/tailwater.yml', issues: ['dangling-edge'] }],
  }
  const row = find(healthTree({ lint: lintReport(), graph, index: INDEX_STATUS, regen: REGEN, doctor: DOCTOR }), 'health:graph')!
  assert.equal(row.icon, 'error')
  assert.equal(row.expanded, true)
  assert.equal(row.description, '2/3 clean')
  assert.equal(row.children![0].file, '.yidam/corpus/concept/tailwater.yml')
})

/**
 * Blessing is the remedy for stale debt, not for a regression.
 *
 * Offering one click that turns new violations into inherited debt would make laundering
 * a regression the easiest thing on the screen — and the baseline is a two-way ratchet
 * precisely so that cannot happen quietly.
 */
test('the lint row offers blessing only when the debt is stale and nothing is new', () => {
  const stale = [{ check: 'node-missing-label', node: 'concept/tailwater.yml' }]

  const staleOnly = find(
    healthTree({
      lint: lintReport({ passed: false, stale_baseline_entries: stale }),
      graph: GRAPH,
      index: INDEX_STATUS,
      regen: REGEN,
      doctor: DOCTOR,
    }),
    'health:lint',
  )!
  assert.equal(staleOnly.command?.id, 'yidam.blessBaseline')
  assert.equal(staleOnly.children!.length, 1)

  const regressed = find(
    healthTree({
      lint: lintReport({ passed: false, new_violations: 2, stale_baseline_entries: stale }),
      graph: GRAPH,
      index: INDEX_STATUS,
      regen: REGEN,
      doctor: DOCTOR,
    }),
    'health:lint',
  )!
  assert.equal(regressed.command, undefined, 'a regression is not blessed away in one click')
})

/**
 * A report that failed to arrive is its own state.
 *
 * Folding it into "failing" would show a red X because a subprocess died, which is a claim
 * about the corpus that nothing checked.
 */
test('a missing report renders as unavailable rather than as a failure', () => {
  const tree = healthTree({ lint: null, graph: null, index: null, regen: null, doctor: null })
  for (const id of ['health:doctor', 'health:graph', 'health:lint', 'health:index', 'health:regen']) {
    const row = find(tree, id)!
    assert.equal(row.description, 'unavailable')
    assert.notEqual(row.icon, 'error')
  }
})

// ── sangha ──────────────────────────────────────────────────────────────────

const SANGHA: SanghaReport = {
  ...ENVELOPE,
  collective: true,
  electors: [
    { name: 'auditor', branch: 'ma/auditor', role: 'Verification.', branch_present: true },
    { name: 'ghost', branch: 'ma/ghost', role: 'Registered early.', branch_present: false },
  ],
  positions: [
    { file: '.yidam/sangha/positions/auditor-scope.md', elector: 'auditor', question: 'scope' },
    { file: '.yidam/sangha/positions/stray.md', elector: '', question: 'stray' },
  ],
  resolutions: [
    {
      file: '.yidam/sangha/resolutions/scope.md',
      evolution: 'scope',
      date: '2026-01-01',
      tips: ['ma/auditor@abc1234'],
      branch_present: false,
    },
  ],
}

test('electors carry their own positions, and a missing branch is said out loud', () => {
  const tree = sanghaTree(SANGHA)
  assert.deepEqual(ids(tree), [
    'elector:auditor',
    'elector:ghost',
    'sangha:unattributed',
    'sangha:resolutions',
  ])
  assert.equal(find(tree, 'elector:ghost')!.description, 'branch missing')
  assert.equal(find(tree, 'elector:ghost')!.icon, 'warning')
  assert.equal(find(tree, 'elector:auditor')!.children!.length, 1)
})

/**
 * A position matching no registered elector is the one nobody would otherwise find.
 */
test('unattributed positions get their own group', () => {
  const group = find(sanghaTree(SANGHA), 'sangha:unattributed')!
  assert.equal(group.expanded, true)
  assert.equal(group.children![0].file, '.yidam/sangha/positions/stray.md')
})

/** A deleted `rigpa/*` branch is routine once the resolution lands — a note, not an alarm. */
test('a resolution whose branch is gone still reads as settled', () => {
  const row = find(sanghaTree(SANGHA), 'resolution:.yidam/sangha/resolutions/scope.md')!
  assert.equal(row.icon, 'law')
  assert.match(row.tooltip!, /branch gone/)
  assert.match(row.tooltip!, /ma\/auditor@abc1234/)
})

test('a repository with no electors says what would make it collective', () => {
  const tree = sanghaTree({ ...SANGHA, collective: false, electors: [] })
  assert.equal(tree.length, 1)
  assert.match(tree[0].tooltip!, /ma\/<name>/)
})

// ── status line ─────────────────────────────────────────────────────────────

const STATUS: StatusReport = {
  ...ENVELOPE,
  nodes: 90,
  open_questions: 3,
  catalog_entries: 12,
  claims_verified: 40,
  claims_inference: 8,
  claims_open: 3,
  index_present: true,
  active_phases: 2,
  genesis: '2026-01-01',
}

test('the status line reports staleness in the unit index-status measures', () => {
  assert.equal(statusLine(STATUS, { ...INDEX_STATUS, stale_nodes: 12 }), '90 nodes · 3 open · index 12 stale')
  assert.equal(statusLine(STATUS, INDEX_STATUS), '90 nodes · 3 open')
  assert.equal(
    statusLine(STATUS, { ...INDEX_STATUS, index_present: false }),
    '90 nodes · 3 open · no index',
  )
  assert.equal(statusLine(null, INDEX_STATUS), null)
})

// ── against the real binary ──────────────────────────────────────────────────





/**
 * The builders, run over what the binary actually emits.
 *
 * The literal fixtures above pin the shapes; this pins the *field names*. A CLI that
 * renamed `ref_name` would leave every test above green and every view empty.
 */
/**
 * A context-menu command is handed the row, not the row's click arguments, so a row whose
 * subject lives only in its `id` cannot be acted on without parsing that id — which would
 * make the format load-bearing for something other than the tree's own bookkeeping.
 */
/**
 * `TreeView.reveal` cannot select a nested row without `getParent`, and the provider walks
 * downward only. This is the reverse walk, where it can be checked against plain data.
 */
test('every row knows its parent, and the roots know they have none', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] })
  const parents = parentIndex(tree)

  for (const cls of tree) {
    assert.equal(parents.get(cls), undefined, 'a class row is a root')
    for (const node of cls.children!) {
      assert.equal(parents.get(node), cls, `${node.id} should point back at ${cls.id}`)
    }
  }
  // Every non-root is in the index: a row reveal cannot reach is a row reveal silently
  // fails on.
  const count = tree.reduce((n, c) => n + (c.children?.length ?? 0), 0)
  assert.equal(parents.size, count)
})

test('the parent walk recurses, on a tree deeper than any view builds', () => {
  // Every builder here nests exactly two deep, so a walk that handled one level of children
  // and never recursed would pass against all five of them. `parentIndex` is a general
  // function over `TreeNode`, and the depth it does not currently meet is the depth the next
  // view will have.
  const leaf: TreeNode = { id: 'c', label: 'c' }
  const mid: TreeNode = { id: 'b', label: 'b', children: [leaf] }
  const root: TreeNode = { id: 'a', label: 'a', children: [mid] }

  const parents = parentIndex([root])
  assert.equal(parents.get(mid), root)
  assert.equal(parents.get(leaf), mid, 'the grandchild must know its parent')
  assert.equal(parents.get(root), undefined)
  assert.equal(parents.size, 2)
})

test('a two-level view maps every child to its group', () => {
  const tree = sanghaTree(SANGHA)
  const parents = parentIndex(tree)
  const elector = tree.find((n) => n.id === 'elector:auditor')!
  assert.equal(parents.get(elector.children![0]), elector)
})

test('a row is findable by the file it stands for', () => {
  const tree = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] })
  const hit = findByFile(tree, '.yidam/corpus/concept/tailwater.yml')
  assert.equal(hit?.id, 'node:concept/tailwater.yml')
  assert.equal(findByFile(tree, '.yidam/corpus/nope.yml'), undefined)
  // Class rows stand for no file and must not be returned for one.
  assert.equal(findByFile(tree, ''), undefined)
})

test('rows that stand for something other than a file name it', () => {
  const corpus = corpusTree(INDEX, { ...ENVELOPE, open_questions: [] })
  const cls = corpus[0]
  assert.equal(cls.subject, cls.label, 'a class row names its class')
  assert.ok(
    cls.children!.every((c) => c.file && !c.subject),
    'an instance row stands for a file and needs no subject',
  )

  const phases = phasesTree(PHASES)
  const phase = find(phases, 'phase:ma/auditor')!
  assert.equal(phase.subject, 'ma/auditor', 'a phase row names its ref')
  // The same string its click command passes, so both routes act on one thing.
  assert.deepEqual(phase.command!.args, [phase.subject])
})

test('every view builds from the real binary’s own JSON', async (t) => {
  const dir = stageFixture('yidam-tree-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  const read = <T>(args: string[]): T =>
    JSON.parse(capture(bin, [...args, '--format', 'json'], dir)) as T

  const index = read<CorpusIndexReport>(['corpus-index'])
  const open = read<OpenQuestionsReport>(['open-questions'])
  const corpus = corpusTree(index, open)
  // More than one class, so grouping is exercised above the arity where any grouping
  // implementation looks correct. The fixture carried one class until it carried two.
  assert.ok(corpus.length > 1, 'the fixture has several classes')
  assert.ok(
    corpus.every((c) => c.children!.length > 0),
    'and instances under each of them',
  )
  assert.ok(
    corpus.flatMap((c) => c.children!).every((c) => c.label !== '—'),
    'no row renders the CLI’s absent-field dash as a label',
  )

  // The filter against the binary's own JSON, and specifically the two questions VS Code's
  // built-in type-to-filter cannot ask. `is:open` reads the join between two reports and
  // `class:` reads a field only one of them carries — both are exactly where a renamed
  // field would go unnoticed against a hand-written report.
  const markedIds = corpus
    .flatMap((c) => c.children!)
    .filter((n) => n.icon === 'question')
    .map((n) => n.id)
  assert.ok(markedIds.length > 0, 'the fixture has open questions to narrow to')
  assert.ok(markedIds.length < index.nodes.length, 'and a node without one, so this narrows')
  const openOnly = corpusTree(index, open, undefined, parseFilter('is:open')!)
  assert.deepEqual(
    openOnly.flatMap((c) => c.children!).map((n) => n.id).sort(),
    markedIds.slice().sort(),
  )

  // The provenance layer, from the binary's own JSON. `cited_by` is a field only the real
  // report can prove is spelled and rooted the way this inversion assumes.
  const catalog = read<CatalogAuditReport>(['catalog-audit'])
  const cited = catalog.sources.find((s) => (s.cited_by?.length ?? 0) > 0)
  assert.ok(cited, 'the fixture has a source some node draws on')
  // The type permits absence, because a CLI older than 0.6.0 omits the field. This test
  // runs *this* binary, so here it is a claim about the current contract rather than a
  // guard: the producing half of what the extension is now written to tolerate missing.
  assert.ok(cited.cited_by, 'the binary emits `cited_by`')
  assert.equal(cited.cited_by.length, cited.nodes, 'the count is the list’s length')
  const uncited = catalog.sources.find((s) => s.cited_by?.length === 0)
  assert.ok(uncited, 'and one no node draws on, which is the other arm')
  assert.equal(uncited.drift, null, 'an entry declaring no `used-by` list has not drifted')

  // Both drift arms, from the report rather than from a literal. One arm alone cannot tell
  // an implementation reading both from one reading either.
  assert.ok(cited.drift, 'the cited entry declares a `used-by` list')
  assert.equal(cited.drift.claimed_not_citing.length, 1, 'it claims a node that does not cite it')
  assert.equal(cited.drift.citing_not_claimed.length, 1, 'and omits one that does')

  const sourced = corpusTree(index, open, undefined, null, catalog)
  const placed = sourced
    .flatMap((c) => c.children!)
    .filter((n) => (n.children ?? []).length > 0)
  assert.equal(
    placed.length,
    cited.nodes,
    'every node the report names carries the source, and no other node does',
  )
  assert.ok(
    placed.every((n) => n.children!.some((src) => src.file?.endsWith(cited.entry))),
    'and the row opens the entry it stands for',
  )
  assert.equal(
    countNodes(sourced),
    index.nodes.length,
    'the node count is unmoved by the sources hanging off it',
  )

  const oneClass = corpusTree(index, open, undefined, parseFilter(`class:${corpus[0].label}`)!)
  assert.deepEqual(ids(oneClass), [corpus[0].id], 'class: leaves exactly the one group')
  assert.equal(countLeaves(oneClass), corpus[0].children!.length, 'and all of its instances')
  assert.deepEqual(
    corpusTree(index, open, undefined, parseFilter('zzzz-matches-nothing')!),
    [],
    'a filter matching nothing shows nothing, and the view message says so',
  )
  assert.deepEqual(
    openQuestionsTree(open, parseFilter(`class:${corpus[0].label}`)!, index).length +
      openQuestionsTree(open, parseFilter(`class:${corpus[1].label}`)!, index).length,
    open.open_questions.length,
    'every open question is placed by the index into one of the corpus’s classes',
  )

  // Both arms of the open-question predicate, from the real report. A corpus using only
  // one of them cannot tell an implementation that reads both from one that reads either
  // — which is why the MCP cases were split into two nodes, and the same was true here.
  //
  // Three, not two, since the fixture gained a claim written in backticks. That node is the
  // one that distinguishes the current rule from the typographic one it replaced: under
  // "a backticked tag is a mention" it would not be an open question at all.
  const labels = open.open_questions.map((q) => q.label)
  assert.equal(labels.length, 3)
  assert.ok(
    labels.some((l) => l.startsWith('?')),
    'the arm stated in the label',
  )
  assert.ok(
    labels.some((l) => !l.startsWith('?')),
    'the arm stated in a declared claim field',
  )

  const phases = phasesTree(read<PhasesReport>(['phases']))
  // The fixture stages one ref of each kind. `phase/*` was in no fixture at all, which is
  // why nothing caught either reader ignoring the namespace.
  assert.deepEqual(ids(phases), ['phases:ma', 'phases:phase', 'phases:rigpa'])
  assert.equal(find(phases, 'phase:ma/hydrologist')!.description, '0 commit(s)')
  // Branched at HEAD with nothing ahead of the baseline, so the binary reports them settled.
  assert.equal(find(phases, 'phase:phase/low-flow-survey')!.description, '0 commit(s) · settled')

  const sangha = read<SanghaReport>(['sangha'])
  assert.equal(sangha.collective, true)
  const tree = sanghaTree(sangha)
  // The fixture registers `gauge-reader` with no branch and files one position under a
  // name the table does not carry. Both arms are exercised by the real report.
  assert.equal(find(tree, 'elector:gauge-reader')!.description, 'branch missing')
  assert.ok(find(tree, 'sangha:unattributed'), 'the fixture carries one unattributed position')
  assert.equal(find(tree, 'elector:hydrologist')!.children!.length, 1)

  const health = healthTree({
    lint: read<LintReport>(['lint']),
    graph: read<GraphCheckReport>(['graph-check']),
    index: read<IndexStatusReport>(['index-status']),
    regen: read<RegenReport>(['regen', '--check']),
    doctor: read<DoctorReport>(['doctor']),
  })
  assert.deepEqual(ids(health), [
    'health:doctor',
    'health:graph',
    'health:lint',
    'health:index',
    'health:regen',
    'health:vendor',
  ])
  // The fixture carries a deliberate broken edge, so the lint gate fails here.
  assert.equal(find(health, 'health:lint')!.icon, 'error')
  assert.equal(find(health, 'health:index')!.description, 'not initialized')
  // The fixture ships a placeholder in its `yidam status` REGEN block, so a fresh copy is
  // stale — and `--check` says so without rewriting the fixture the rest of this test reads.
  assert.equal(find(health, 'health:regen')!.icon, 'warning')

  const line = statusLine(read<StatusReport>(['status']), read<IndexStatusReport>(['index-status']))
  assert.match(line!, /^\d+ nodes · \d+ open · no index$/)
})
