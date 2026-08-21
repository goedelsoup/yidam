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
  CorpusIndexReport,
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
  healthTree,
  localRef,
  openQuestionsTree,
  phasesTree,
  sanghaTree,
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

test('health carries all five rows, gates and the one act alike', () => {
  const tree = healthTree({ lint: lintReport(), graph: GRAPH, index: INDEX_STATUS, regen: REGEN })
  assert.deepEqual(ids(tree), [
    'health:graph',
    'health:lint',
    'health:index',
    'health:regen',
    'health:vendor',
  ])
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
    healthTree({ lint: lintReport(), graph: GRAPH, index: INDEX_STATUS, regen: REGEN }),
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
    healthTree({ lint: lintReport(), graph: GRAPH, index: INDEX_STATUS, regen: stale }),
    'health:regen',
  )!
  assert.equal(row.icon, 'warning')
  assert.equal(row.description, '1 stale')
  assert.equal(row.expanded, true)
  assert.equal(row.children![0].file, '.yidam/corpus/README.md')
  assert.equal(row.children![0].description, 'corpus-index')

  const current = find(
    healthTree({ lint: lintReport(), graph: GRAPH, index: INDEX_STATUS, regen: REGEN }),
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
  const row = find(healthTree({ lint: lintReport(), graph, index: INDEX_STATUS, regen: REGEN }), 'health:graph')!
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
  const tree = healthTree({ lint: null, graph: null, index: null, regen: null })
  for (const id of ['health:graph', 'health:lint', 'health:index', 'health:regen']) {
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

  // Both arms of the open-question predicate, from the real report. A corpus using only
  // one of them cannot tell an implementation that reads both from one that reads either
  // — which is why the MCP cases were split into two nodes, and the same was true here.
  const labels = open.open_questions.map((q) => q.label)
  assert.equal(labels.length, 2)
  assert.ok(
    labels.some((l) => l.startsWith('?')),
    'the arm stated in the label',
  )
  assert.ok(
    labels.some((l) => !l.startsWith('?')),
    'the arm stated in a declared claim field',
  )

  const phases = phasesTree(read<PhasesReport>(['phases']))
  assert.deepEqual(ids(phases), ['phases:ma', 'phases:rigpa'])
  assert.equal(find(phases, 'phase:ma/hydrologist')!.description, '0 commit(s)')

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
  })
  assert.deepEqual(ids(health), [
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
