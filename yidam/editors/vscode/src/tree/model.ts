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
  CatalogAuditReport,
  CorpusIndexReport,
  DoctorReport,
  GraphCheckReport,
  IndexStatusReport,
  LintReport,
  OpenQuestionsReport,
  RegenReport,
  PhasesReport,
  SanghaReport,
  SourceRow,
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
  /**
   * What this row is *about*, when that is not a file — a class name, a ref name.
   *
   * A context-menu command is handed the `TreeNode` itself, so this is how it learns what
   * it was invoked on without parsing `id`. An id is an identity for the tree's own
   * bookkeeping; reading a subject out of it would make the format load-bearing.
   */
  subject?: string
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

/**
 * Child → parent, over a whole forest.
 *
 * `TreeView.reveal` cannot select a nested row without `getParent`, and the provider walks
 * downward only — it holds roots and hands out `children`. Building the reverse here rather
 * than in the provider keeps the walk where it can be tested against plain data; the
 * provider stores the result and answers from it.
 *
 * Keyed by object identity, which is sound because the provider hands out the very nodes it
 * was given. Ids would work too and would invite the id format to become load-bearing.
 */
export function parentIndex(roots: TreeNode[]): Map<TreeNode, TreeNode> {
  const parents = new Map<TreeNode, TreeNode>()
  const walk = (node: TreeNode): void => {
    for (const child of node.children ?? []) {
      parents.set(child, node)
      walk(child)
    }
  }
  for (const root of roots) walk(root)
  return parents
}

/**
 * The row standing for a repo-relative path, anywhere in the forest.
 *
 * Depth-first and first-match. A corpus node appears once; a catalog entry appears under
 * every node that cites it, and `reveal` on one of those wants *a* row rather than all of
 * them — a caller that wanted the set would be asking a different question.
 */
export function findByFile(roots: TreeNode[], file: string): TreeNode | undefined {
  for (const node of roots) {
    if (node.file === file) return node
    const hit = findByFile(node.children ?? [], file)
    if (hit) return hit
  }
  return undefined
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

// ── narrowing ───────────────────────────────────────────────────────────────

/**
 * A filter, parsed once and applied to the two views that grow with the corpus.
 *
 * **In memory, never a setting.** A `.vscode/settings.json` carrying a filter is a filter
 * somebody else committed, applied to a window whose reader did not narrow anything — and
 * the symptom, a node that is not in the tree, reads as a deleted node rather than a hidden
 * one. A filter that dies with the window cannot mislead a second person.
 *
 * Three rules and no more, each of them a field the reports already returned. This narrows
 * what `corpus-index` and `open-questions` answered; it never decides membership, which is
 * the line the affordance/verdict rule draws. VS Code's own type-to-filter covers the free
 * text alone — `class:` and `is:open` are here because that one matches the rendered label
 * and nothing else, so it cannot answer either of the two questions a reader at scale has.
 */
export interface Filter {
  /** As typed, so a narrowed view can say what it is hiding rows for. */
  query: string
  /** Free text. Every term must match the label or the node path. */
  terms: string[]
  /** `class:<name>`, substring. Any one may match. */
  classes: string[]
  /** `is:open` — the node carries an open question. */
  openOnly: boolean
}

/**
 * `null` for an empty query.
 *
 * Which is what keeps "no filter" distinguishable from "a filter that happens to match
 * everything": the second still narrows, and a view under one still has to say so.
 */
export function parseFilter(query: string): Filter | null {
  const raw = query.trim()
  if (raw.length === 0) return null
  const terms: string[] = []
  const classes: string[] = []
  let openOnly = false
  for (const token of raw.toLowerCase().split(/\s+/)) {
    if (token === 'is:open') openOnly = true
    else if (token.startsWith('class:') && token.length > 'class:'.length) {
      classes.push(token.slice('class:'.length))
    } else terms.push(token)
  }
  return { query: raw, terms, classes, openOnly }
}

/** One row's filterable facts, as the reports gave them. */
export interface Filterable {
  /** The label as rendered, so typing what is on the screen matches it. */
  label: string
  /** The path, as its own report spells it — matched as free text, so either root works. */
  node: string
  /** Absent when no report in hand says. `class:` then matches nothing rather than guessing. */
  class?: string
  open: boolean
}

/** Terms are conjunctive, classes disjunctive: narrowing to two classes is one question. */
export function matches(filter: Filter | null, row: Filterable): boolean {
  if (!filter) return true
  if (filter.openOnly && !row.open) return false
  if (filter.classes.length > 0) {
    const cls = (row.class ?? '').toLowerCase()
    if (cls.length === 0 || !filter.classes.some((c) => cls.includes(c))) return false
  }
  if (filter.terms.length === 0) return true
  const hay = `${row.label}\n${row.node}`.toLowerCase()
  return filter.terms.every((t) => hay.includes(t))
}

/**
 * Corpus rows that stand for a node.
 *
 * Not [`countLeaves`], and the distinction became load-bearing the moment a node could
 * carry sources: a leaf in the Corpus view is now a *source*, so counting leaves would
 * report a filter as `17 of 90` because seventeen sources survived under three nodes.
 * The filter narrows nodes, so the message counts nodes.
 */
export function countNodes(roots: TreeNode[]): number {
  let n = 0
  for (const node of roots) {
    if (node.context === 'yidam.node') n += 1
    n += countNodes(node.children ?? [])
  }
  return n
}

/** Rows rather than groups — what a reader is counting when they ask how much is left. */
export function countLeaves(roots: TreeNode[]): number {
  let n = 0
  for (const node of roots) {
    const kids = node.children ?? []
    n += kids.length === 0 ? 1 : countLeaves(kids)
  }
  return n
}

/**
 * What a narrowed view says about itself, or nothing when it is not narrowed.
 *
 * A view that hides rows without saying it is hiding them is how a reader concludes a node
 * was deleted. It belongs in the view's `message` rather than in a row: the tree holds
 * nodes, and "12 of 90" is not one.
 */
export function filterMessage(
  filter: Filter | null,
  shown: number,
  total: number,
): string | undefined {
  if (!filter) return undefined
  return shown === 0
    ? `filter: ${filter.query} — nothing matches, ${total} hidden`
    : `filter: ${filter.query} — ${shown} of ${total}`
}

/**
 * The class `corpus-index` gives this node, under the name the Corpus view renders it by.
 *
 * `open-questions` does not carry one, and the `<class>/<name>.yml` layout that would let
 * one be read off the path is a convention rather than a fact — deriving it here would be
 * this file forming its own opinion about what class a node is in. So `class:` narrows the
 * Open questions view only when the index is also in hand, and matches nothing when it is
 * not, which is the honest answer to a question nothing available can answer.
 */
function classOf(index: CorpusIndexReport | null, node: string): string | undefined {
  if (!index) return undefined
  for (const row of index.nodes) {
    if (node === row.node || node.endsWith(`/${row.node}`)) return named(row.class, 'unclassed')
  }
  return undefined
}

/**
 * Node → the sources it draws on, inverted from `catalog-audit`.
 *
 * The report is indexed by source because that is the question the audit asks — *who draws
 * on this*. A reader in a corpus view asks the other one, *what does this rest on*, and the
 * inversion is a re-index of what the report already said rather than a second opinion
 * about what a citation is. `cited_by` is the CLI's answer to that, resolved by the same
 * function the gate reads; deriving it here from links would be the re-implementation the
 * whole contract exists to prevent.
 *
 * Keyed on the node path as `catalog-audit` spells it — repository-relative — while
 * `corpus-index` spells its rows relative to the corpus. Matched on suffix, the same way
 * `openSet` reconciles the same two roots, so a repository that moves its corpus keeps its
 * provenance.
 */
export function sourcesByNode(
  catalog: CatalogAuditReport | null,
  rows: { node: string }[],
): Map<string, SourceRow[]> {
  const out = new Map<string, SourceRow[]>()
  if (!catalog) return out
  for (const source of catalog.sources) {
    for (const cited of source.cited_by ?? []) {
      for (const row of rows) {
        if (cited !== row.node && !cited.endsWith(`/${row.node}`)) continue
        const list = out.get(row.node) ?? []
        list.push(source)
        out.set(row.node, list)
      }
    }
  }
  for (const list of out.values()) list.sort((a, b) => a.entry.localeCompare(b.entry))
  return out
}

/**
 * One source, as a row beneath the node that cites it.
 *
 * Clicking opens the catalog entry, which is the whole point: the question this answers is
 * "what is this node sourced from", and until now answering it meant leaving the view.
 *
 * `obtained: false` is stated and is **not** dressed as a failure. A node citing an
 * unretrieved source is `catalog-unobtained-but-cited`, which is Error severity and reaches
 * the editor as a diagnostic and a Health row. A second red mark here would be this file
 * rendering a verdict it did not compute.
 */
function sourceRow(node: string, source: SourceRow, catalogDir: string): TreeNode {
  // Reached through `?.` rather than a null test, because there are three states and only
  // two of them are about drift: a declared list that holds, a declared list that does not,
  // and — against a CLI older than 0.6.0 — no field at all. The first and third both mean
  // "nothing to report here", so neither arm needs to distinguish them.
  const drifting =
    (source.drift?.claimed_not_citing.length ?? 0) > 0 ||
    (source.drift?.citing_not_claimed.length ?? 0) > 0
  return {
    id: `source:${node}:${source.entry}`,
    label: named(source.description, stem(source.entry)),
    description: [source.type === '—' ? null : source.type, source.obtained ? null : 'not obtained']
      .filter((p) => p !== null)
      .join(' · '),
    tooltip:
      `${source.entry}\n${source.nodes} node(s) draw on this` +
      (source.elsewhere > 0 ? `, ${source.elsewhere} other file(s) link to it` : '') +
      (drifting ? '\n\nIts `used-by` list disagrees with the citations.' : ''),
    icon: 'book',
    file: `${catalogDir}/${source.entry}`,
  }
}

/**
 * Classes, then their instances, then what each instance rests on.
 *
 * A class whose every instance the filter hid is dropped rather than shown empty: an
 * expandable row with nothing under it is a worse answer than no row. A node with no
 * sources gets no children for the same reason — most nodes cite nothing, and making every
 * one of them expandable would put an arrow beside a row that has nothing behind it.
 *
 * The filter narrows *nodes*. A kept node keeps its sources, and a term matching a source
 * name keeps nothing: the two views this filter was built for are indexed by node, and a
 * filter that sometimes meant "source" would be answering a question the reader did not ask.
 */
export function corpusTree(
  index: CorpusIndexReport,
  open: OpenQuestionsReport,
  corpusDir = '.yidam/corpus',
  filter: Filter | null = null,
  catalog: CatalogAuditReport | null = null,
  catalogDir = '.yidam/catalog',
): TreeNode[] {
  const marked = openSet(open, index.nodes)
  const sources = sourcesByNode(catalog, index.nodes)
  const byClass = new Map<string, { kept: typeof index.nodes; total: number }>()
  for (const row of index.nodes) {
    const key = named(row.class, 'unclassed')
    const group = byClass.get(key) ?? { kept: [], total: 0 }
    group.total += 1
    const keep = matches(filter, {
      label: named(row.label, stem(row.node)),
      node: row.node,
      class: key,
      open: marked.has(row.node),
    })
    if (keep) group.kept.push(row)
    byClass.set(key, group)
  }

  return [...byClass.entries()]
    .filter(([, group]) => group.kept.length > 0)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([cls, group]) => ({
      id: `class:${cls}`,
      label: cls,
      // Narrowed, the count says both numbers. A bare `3` under a class of twelve reads as
      // a class of three, which is the filter lying about the corpus rather than about
      // what it is showing of it.
      description:
        group.kept.length === group.total
          ? `${group.total}`
          : `${group.kept.length} of ${group.total}`,
      icon: 'symbol-class',
      subject: cls,
      context: 'yidam.class',
      children: group.kept
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
          children: (sources.get(row.node) ?? []).map((source) =>
            sourceRow(row.node, source, catalogDir),
          ),
        })),
    }))
}

/**
 * Flat, because the question is flat.
 *
 * The single most-asked question of a research repository, and until now the only answer
 * was a REGEN table in a README — which is to say, correct as of the last time somebody
 * ran the generator.
 *
 * Flatness is right for two and awkward for sixty-four, and the filter is the answer to
 * that rather than grouping: a group is a claim about how the questions divide, and the
 * one division available here — the class — is a fact about the *node*, not about the
 * question it carries. So it narrows on request and stays flat.
 */
export function openQuestionsTree(
  open: OpenQuestionsReport,
  filter: Filter | null = null,
  index: CorpusIndexReport | null = null,
): TreeNode[] {
  // Asked before the filter, so "there are none" and "none of them match" stay separate
  // answers. A tick reading `No open questions` under an active filter would be the view
  // congratulating a repository on a state the filter invented.
  if (open.open_questions.length === 0) {
    return [{ id: 'open:none', label: 'No open questions', icon: 'check' }]
  }
  return open.open_questions
    .filter((q) =>
      matches(filter, {
        label: named(q.label, stem(q.node)),
        node: q.node,
        class: classOf(index, q.node),
        open: true,
      }),
    )
    .map((q) => ({
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
 * `ma/*`, `phase/*` and `rigpa/*` in separate groups.
 *
 * The three namespaces are different acts — a held position, a bounded investigation, and a
 * settled synthesis — and a repository running a real sangha has an order of magnitude more
 * of the last. One flat list buries the first, which is the half a reader is usually looking
 * for.
 *
 * `phase/*` had no group and fell into `Other`, which is the same defect the CLI carried:
 * PHASES.md defines a phase in that namespace and both readers of it looked elsewhere. The
 * fixture gained a `phase/*` branch and this test failed, which is the whole reason to put
 * one there.
 */
export function phasesTree(report: PhasesReport): TreeNode[] {
  const row = (p: PhasesReport['phases'][number]): TreeNode => ({
    id: `phase:${p.ref_name}`,
    label: p.name,
    // Settledness is called out and nothing else is: `active` is the expectation and
    // `position` is already the group's name, so stating either would be noise. A ref that
    // outlived its settlement is the one thing here a reader can act on.
    description:
      p.state === 'settled' ? `${p.commits} commit(s) · settled` : `${p.commits} commit(s)`,
    tooltip: `${p.ref_name}\n${p.owner} · started ${p.started}`,
    icon: 'git-branch',
    command: { id: 'yidam.checkoutPhase', args: [p.ref_name] },
    subject: p.ref_name,
    context: 'yidam.phase',
  })

  const positions = report.phases.filter((p) => isNamespace(p.ref_name, 'ma'))
  const bounded = report.phases.filter((p) => isNamespace(p.ref_name, 'phase'))
  const evolutions = report.phases.filter((p) => isNamespace(p.ref_name, 'rigpa'))
  const other = report.phases.filter(
    (p) =>
      !isNamespace(p.ref_name, 'ma') &&
      !isNamespace(p.ref_name, 'phase') &&
      !isNamespace(p.ref_name, 'rigpa'),
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
  if (bounded.length > 0) {
    groups.push({
      id: 'phases:phase',
      label: 'Phases',
      description: `phase/* · ${bounded.length}`,
      icon: 'beaker',
      children: bounded.map(row),
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
  regen: RegenReport | null
  doctor: DoctorReport | null
}

/**
 * Whether the setup is sound, which is the question that comes before the corpus's.
 *
 * First in the view for that reason: a repository whose binary is not the one it pins, or
 * whose provenance file is missing, is one where every row beneath this may be answering
 * about something other than what the reader thinks. It is a precondition, not a gate.
 *
 * Only the checks that are not `ok` get rows. `doctor` asks nine questions and eight of them
 * are usually yes; listing those is a wall of ticks that buries the one that is not.
 *
 * **Remedies are stated and never run.** Each is a shell command the report chose, and a
 * one-click row that executed a string a subprocess handed us is a different capability from
 * rendering one — the two mise tasks this extension does run are named in its own manifest.
 */
function doctorRow(doctor: DoctorReport): TreeNode {
  const actionable = doctor.checks.filter((c) => c.verdict !== 'ok' && c.verdict !== 'skipped')
  return {
    id: 'health:doctor',
    label: 'Setup',
    description: doctor.passed
      ? doctor.warned > 0
        ? `${doctor.warned} warning(s)`
        : 'sound'
      : `${doctor.failed} failing`,
    tooltip: doctor.passed
      ? 'Nothing here is wrong now. `yidam doctor` for the full list.'
      : 'Something about this setup is wrong now, not merely worth knowing.',
    // A failing check is wrong now; a warning is routinely lived with — a light `reports`
    // install legitimately has no vector index. Rendering both red would teach a reader to
    // ignore the row.
    icon: !doctor.passed ? 'error' : doctor.warned > 0 ? 'warning' : 'pass',
    expanded: !doctor.passed,
    children: actionable.map((c) => ({
      id: `health:doctor:${c.id}`,
      label: c.question,
      description: c.detail,
      tooltip: c.remedy ? `${c.detail}\n\nRemedy: ${c.remedy}` : c.detail,
      icon: c.verdict === 'fail' ? 'error' : 'warning',
    })),
  }
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
 * Five rows: four gates and one act.
 *
 * REGEN freshness was an act until `yidam regen --check` existed. It could not be a gate
 * while the only way to answer the question was to rewrite the blocks and see what moved —
 * an extension is not going to edit your files to render a tick.
 *
 * Vendor drift is still an act, and for a reason that will not go away: it needs the
 * network. A row that claimed the prelude was current without asking the origin would be
 * this extension inventing a verdict, which is the failure RFC-0016 exists to close.
 */
export function healthTree(input: HealthInput): TreeNode[] {
  const { lint, graph, index, regen, doctor } = input
  const rows: TreeNode[] = []

  if (!doctor) rows.push(unavailable('health:doctor', 'Setup'))
  else rows.push(doctorRow(doctor))

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

  if (!regen) rows.push(unavailable('health:regen', 'REGEN blocks'))
  else
    rows.push({
      id: 'health:regen',
      label: 'REGEN blocks',
      description: regen.passed ? 'current' : `${regen.stale.length} stale`,
      tooltip: regen.passed
        ? 'Every generated block holds what its generator produces.'
        : 'A stale block is a README telling its readers something the corpus no longer says.',
      icon: regen.passed ? 'pass' : 'warning',
      expanded: !regen.passed,
      // The row's action is the remedy, and unlike blessing a baseline it is never the
      // wrong thing to do: regenerating a current block is a no-op.
      command: { id: 'yidam.regen' },
      children: regen.stale.map((s) => ({
        id: `health:regen:${s.file}`,
        label: s.file,
        description: s.generator,
        tooltip: `\`yidam ${s.generator}\` produces something this file does not hold.`,
        icon: 'sync',
        file: s.file,
      })),
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
