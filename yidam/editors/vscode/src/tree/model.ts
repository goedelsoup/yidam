/**
 * The five views, as data.
 *
 * No `vscode` import, and that is the whole design: a tree is a shape, and a shape can be
 * asserted without an editor. What is left in `provider.ts` is the adapter — `TreeNode` to
 * `vscode.TreeItem` — which has no decisions in it.
 *
 * Every builder here is a *transcription* of a report. None of them decides whether a gate
 * passed, whether a node is an open question, or whether the index is stale; those words
 * arrive already answered, from the pinned binary, over the contract in RFC-0016. The rule:
 *
 * > **TypeScript computes affordances. The CLI computes verdicts.**
 *
 * Grouping, ordering, icon choice and phrasing are affordances. Their failure mode is a
 * tree that reads badly. Recomputing a verdict here has a different failure mode — a
 * repository that passes CI and shows red, or the reverse — and that is the drift the
 * report contract exists to prevent.
 */

import type {
  CorpusIndexReport,
  GraphCheckReport,
  IndexStatusReport,
  LintReport,
  OpenQuestionsReport,
  PhasesReport,
  SanghaReport,
  StatusReport,
} from '../reports.ts'

/** A row. Children present means expandable; absent means a leaf. */
export interface TreeNode {
  /** Unique and stable across refreshes, so expansion state survives one. */
  id: string
  label: string
  /** Dimmed text beside the label. */
  description?: string
  tooltip?: string
  /** Codicon id, e.g. `symbol-class`. */
  icon?: string
  /** Repository-relative path this row stands for. Clicking opens it. */
  file?: string
  /** Line to reveal, 1-based, when `file` is set. */
  line?: number
  /** What to run on click, for rows that are actions rather than files. */
  command?: { id: string; args?: unknown[] }
  /** `contextValue`, for menu contributions. */
  context?: string
  children?: TreeNode[]
  /** Start expanded. Used for the groups a reader always wants open. */
  expanded?: boolean
}

/** `concept/tailwater.yml` → `tailwater` */
function stem(path: string): string {
  const base = path.slice(path.lastIndexOf('/') + 1)
  const dot = base.lastIndexOf('.')
  return dot > 0 ? base.slice(0, dot) : base
}

/** The em-dash the CLI prints for an absent field is not a label. */
function named(label: string, fallback: string): string {
  const t = label.trim()
  return t.length > 0 && t !== '—' ? t : fallback
}

function claims(v: number, i: number, o: number): string {
  return `${v}v / ${i}i / ${o}o`
}

/**
 * `corpus-index` rows are relative to the corpus; `open-questions` nodes are relative to
 * the repository root. Matched on suffix rather than by prefixing a hardcoded
 * `.yidam/corpus/`, so a repository that moves its corpus still gets its questions marked.
 */
function openSet(open: OpenQuestionsReport, rows: { node: string }[]): Set<string> {
  const marked = new Set<string>()
  for (const q of open.open_questions) {
    for (const row of rows) {
      if (q.node === row.node || q.node.endsWith(`/${row.node}`)) marked.add(row.node)
    }
  }
  return marked
}

/** Classes, then their instances. Open questions carry a different icon. */
export function corpusTree(
  index: CorpusIndexReport,
  open: OpenQuestionsReport,
  corpusDir = '.yidam/corpus',
): TreeNode[] {
  const marked = openSet(open, index.nodes)
  const byClass = new Map<string, typeof index.nodes>()
  for (const row of index.nodes) {
    const key = named(row.class, 'unclassed')
    const list = byClass.get(key) ?? []
    list.push(row)
    byClass.set(key, list)
  }

  return [...byClass.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([cls, rows]) => ({
      id: `class:${cls}`,
      label: cls,
      description: `${rows.length}`,
      icon: 'symbol-class',
      context: 'yidam.class',
      children: rows
        .slice()
        .sort((a, b) => named(a.label, a.node).localeCompare(named(b.label, b.node)))
        .map((row) => ({
          id: `node:${row.node}`,
          label: named(row.label, stem(row.node)),
          description: marked.has(row.node) ? 'open' : undefined,
          tooltip: `${row.node}\nclaims ${claims(
            row.claims_verified,
            row.claims_inference,
            row.claims_open,
          )} · ${row.links_out} link(s) out · ${row.lines} lines`,
          icon: marked.has(row.node) ? 'question' : 'symbol-field',
          file: `${corpusDir}/${row.node}`,
          context: 'yidam.node',
        })),
    }))
}

/**
 * Flat, because the question is flat.
 *
 * The single most-asked question of a research repository, and until now the only answer
 * was a REGEN table in a README — which is to say, correct as of the last time somebody
 * ran the generator.
 */
export function openQuestionsTree(open: OpenQuestionsReport): TreeNode[] {
  if (open.open_questions.length === 0) {
    return [{ id: 'open:none', label: 'No open questions', icon: 'check' }]
  }
  return open.open_questions.map((q) => ({
    id: `open:${q.node}`,
    label: named(q.label, stem(q.node)),
    description: q.node,
    icon: 'question',
    file: q.node,
    context: 'yidam.openQuestion',
  }))
}

/** Local or remote-tracking, `ma/auditor` and `origin/ma/auditor` alike. */
function isNamespace(ref: string, ns: string): boolean {
  return ref === ns || ref.startsWith(`${ns}/`) || ref.includes(`/${ns}/`)
}

/**
 * `ma/*` and `rigpa/*` in separate groups.
 *
 * The two namespaces are different acts — a held position and a settled synthesis — and a
 * repository running a real sangha has an order of magnitude more of the second. One flat
 * list buries the first, which is the half a reader is usually looking for.
 */
export function phasesTree(report: PhasesReport): TreeNode[] {
  const row = (p: PhasesReport['phases'][number]): TreeNode => ({
    id: `phase:${p.ref_name}`,
    label: p.name,
    description: `${p.commits} commit(s)`,
    tooltip: `${p.ref_name}\n${p.owner} · started ${p.started}`,
    icon: 'git-branch',
    command: { id: 'yidam.checkoutPhase', args: [p.ref_name] },
    context: 'yidam.phase',
  })

  const positions = report.phases.filter((p) => isNamespace(p.ref_name, 'ma'))
  const evolutions = report.phases.filter((p) => isNamespace(p.ref_name, 'rigpa'))
  const other = report.phases.filter(
    (p) => !isNamespace(p.ref_name, 'ma') && !isNamespace(p.ref_name, 'rigpa'),
  )

  const groups: TreeNode[] = []
  if (positions.length > 0) {
    groups.push({
      id: 'phases:ma',
      label: 'Positions',
      description: `ma/* · ${positions.length}`,
      icon: 'account',
      expanded: true,
      children: positions.map(row),
    })
  }
  if (evolutions.length > 0) {
    groups.push({
      id: 'phases:rigpa',
      label: 'Evolutions',
      description: `rigpa/* · ${evolutions.length}`,
      icon: 'merge',
      children: evolutions.map(row),
    })
  }
  // A ref the CLI reported and neither namespace claims. Shown rather than dropped: the
  // report is the authority on what a phase is, and a group that silently swallows rows
  // is how a view starts disagreeing with the command behind it.
  if (other.length > 0) {
    groups.push({
      id: 'phases:other',
      label: 'Other',
      description: `${other.length}`,
      icon: 'git-branch',
      children: other.map(row),
    })
  }
  if (groups.length === 0) {
    return [{ id: 'phases:none', label: 'No active phases', icon: 'circle-slash' }]
  }
  return groups
}

export interface HealthInput {
  lint: LintReport | null
  graph: GraphCheckReport | null
  index: IndexStatusReport | null
}

/**
 * A gate whose report did not arrive.
 *
 * Rendered as its own state rather than folded into "failing": those are different facts,
 * and a view that shows a red X because a subprocess died is lying about the corpus.
 */
function unavailable(id: string, label: string): TreeNode {
  return {
    id,
    label,
    description: 'unavailable',
    tooltip: 'The binary did not produce a readable report. This is not a verdict.',
    icon: 'question',
  }
}

/**
 * Five rows: three gates and two acts.
 *
 * The split is not cosmetic. A gate has a verdict a command computed — `graph-check`,
 * `lint`, `index-status` all answer one. REGEN freshness and vendor drift do **not**:
 * nothing reports whether a REGEN block is stale without rewriting it, and prelude drift
 * is not knowable without the network. Rendering either as a green tick would be this
 * extension inventing a verdict, which is exactly the failure RFC-0016 exists to close.
 * So they are offered as things to run, and they say so.
 */
export function healthTree(input: HealthInput): TreeNode[] {
  const { lint, graph, index } = input
  const rows: TreeNode[] = []

  if (!graph) rows.push(unavailable('health:graph', 'Graph'))
  else rows.push({
    id: 'health:graph',
    label: 'Graph',
    description: graph.corpus_empty
      ? 'empty corpus'
      : `${graph.clean_instances}/${graph.total_instances} clean`,
    tooltip: graph.passed ? 'graph-check passes.' : 'graph-check fails.',
    icon: graph.passed ? 'pass' : 'error',
    expanded: !graph.passed,
    children: graph.nodes_with_issues.map((n) => ({
      id: `health:graph:${n.node}`,
      label: stem(n.node),
      description: n.issues.join('; '),
      tooltip: n.node,
      icon: 'error',
      file: n.node,
    })),
  })

  if (!lint) {
    rows.push(unavailable('health:lint', 'Lint'))
  } else {
  const stale = lint.gate.stale_baseline_entries
  const lintRow: TreeNode = {
    id: 'health:lint',
    label: 'Lint',
    description: `${lint.gate.new_violations} new · ${lint.gate.baselined_violations} inherited${
      stale.length > 0 ? ` · ${stale.length} stale` : ''
    }`,
    tooltip: lint.gate.passed
      ? 'The lint gate agrees with the committed baseline.'
      : 'The lint gate disagrees with the committed baseline.',
    icon: lint.gate.passed ? 'pass' : 'error',
    expanded: stale.length > 0,
    children: stale.map((e) => ({
      id: `health:stale:${e.check}:${e.node}`,
      label: e.node,
      description: `${e.check} — no longer violated`,
      tooltip:
        'The baseline records this violation and the corpus no longer has it. A baseline ' +
        'permitted to be wrong drifts, so this fails CI too. Bless to clear it.',
      icon: 'issue-closed',
      command: { id: 'yidam.blessBaseline' },
    })),
  }
  // Blessing is offered as the row's action only when the debt is *stale*. A failing gate
  // with new violations is a regression, and one-click blessing would make laundering it
  // into inherited debt the easiest thing on the screen.
  if (stale.length > 0 && lint.gate.new_violations === 0) {
    lintRow.command = { id: 'yidam.blessBaseline' }
  }
  rows.push(lintRow)
  }

  if (!index) rows.push(unavailable('health:index', 'Semantic index'))
  else rows.push({
    id: 'health:index',
    label: 'Semantic index',
    description: describeIndex(index),
    tooltip: index.meta_present
      ? `built ${index.built} · ${index.model} (${index.embedding_dim} dims) · ${index.node_count} rows`
      : 'No index metadata.',
    icon: !index.index_present ? 'circle-slash' : index.stale_nodes > 0 ? 'warning' : 'pass',
    command: { id: 'yidam.buildIndex' },
  })

  rows.push({
    id: 'health:regen',
    label: 'REGEN blocks',
    description: 'run to refresh',
    tooltip:
      'Freshness is not reported: nothing answers whether a block is stale without ' +
      'rewriting it, so this is an action rather than a gate. CI still checks it, by ' +
      'running the generators and diffing.',
    icon: 'sync',
    command: { id: 'yidam.regen' },
  })

  rows.push({
    id: 'health:vendor',
    label: 'Vendored prelude',
    description: 'check for updates',
    tooltip:
      'Drift against the pin in `.yidam.toml` needs the network to answer, so it is not ' +
      'checked on activation. Runs `mise run yidam-vendor-status`.',
    icon: 'cloud-download',
    command: { id: 'yidam.vendorStatus' },
  })

  return rows
}

function describeIndex(index: IndexStatusReport): string {
  if (!index.index_present) return 'not initialized'
  if (!index.meta_present) return 'present, no metadata'
  if (index.stale_nodes > 0) return `${index.stale_nodes} node(s) stale`
  return `up to date${index.built ? ` · ${index.built}` : ''}`
}

/**
 * Electors, their positions beneath them, then the settled record.
 *
 * Read-only, and that is constitutional rather than a scoping decision: Article V confines
 * synthesis to resolution events, so a surface that wrote a position or drafted a
 * resolution would be performing one outside the protocol that routes them.
 */
export function sanghaTree(s: SanghaReport): TreeNode[] {
  if (!s.collective) {
    return [
      {
        id: 'sangha:none',
        label: 'Not in collective mode',
        description: 'no registered electors',
        tooltip:
          'An elector is someone maintaining a `ma/<name>` branch, registered in ' +
          '`.yidam/sangha/electors.md`. Until one is, there is no sangha to show.',
        icon: 'circle-slash',
      },
    ]
  }

  const positionRow = (p: SanghaReport['positions'][number]): TreeNode => ({
    id: `position:${p.file}`,
    label: p.question,
    icon: 'comment',
    file: p.file,
    context: 'yidam.position',
  })

  const out: TreeNode[] = s.electors.map((e) => {
    const held = s.positions.filter((p) => p.elector === e.name)
    return {
      id: `elector:${e.name}`,
      label: e.name,
      description: e.branch_present ? `${held.length} position(s)` : 'branch missing',
      tooltip: `${e.branch}\n\n${e.role}`,
      icon: e.branch_present ? 'account' : 'warning',
      children: held.map(positionRow),
    }
  })

  // Positions matching no registered elector. Surfaced rather than dropped — a position
  // filed under a name the table does not carry is the one nobody will find otherwise.
  const orphans = s.positions.filter((p) => p.elector === '')
  if (orphans.length > 0) {
    out.push({
      id: 'sangha:unattributed',
      label: 'Unattributed',
      description: `${orphans.length}`,
      tooltip:
        'Filed under a name `electors.md` does not carry. Either the file is misnamed or ' +
        'the elector was never registered.',
      icon: 'question',
      expanded: true,
      children: orphans.map((p) => ({ ...positionRow(p), label: p.question })),
    })
  }

  out.push({
    id: 'sangha:resolutions',
    label: 'Resolutions',
    description: `${s.resolutions.length}`,
    icon: 'law',
    children: s.resolutions.map((r) => ({
      id: `resolution:${r.file}`,
      label: r.evolution,
      description: r.date || undefined,
      tooltip:
        `rigpa/${r.evolution}${r.branch_present ? '' : ' (branch gone)'}\n\n` +
        (r.tips.length > 0 ? `Read:\n  ${r.tips.join('\n  ')}` : 'No tips recorded.'),
      icon: 'law',
      file: r.file,
      context: 'yidam.resolution',
    })),
  })

  return out
}

/**
 * The status bar line.
 *
 * `index N stale` rather than the RFC's "index 3 commits stale": staleness is counted in
 * corpus files modified since the build, which is what `index-status` measures and what a
 * rebuild would actually re-embed. Commits are the wrong unit — one commit can touch fifty
 * nodes or none.
 */
export function statusLine(
  status: StatusReport | null,
  index: IndexStatusReport | null,
): string | null {
  if (!status) return null
  const parts = [`${status.nodes} nodes`, `${status.open_questions} open`]
  if (index && !index.index_present) parts.push('no index')
  else if (index && index.stale_nodes > 0) parts.push(`index ${index.stale_nodes} stale`)
  return parts.join(' · ')
}

/**
 * The ref to hand `git switch`.
 *
 * A phase living only on a remote is reported as `origin/ma/auditor`, and switching to that
 * literally lands you on a detached HEAD — the one outcome a click on a branch row must not
 * produce. Dropping the remote prefix lets git's own DWIM create the tracking branch, which
 * is what somebody clicking a remote-only phase means.
 */
export function localRef(ref: string): string {
  const m = /^[^/]+\/((?:ma|rigpa)\/.+)$/.exec(ref)
  return m ? m[1] : ref
}
