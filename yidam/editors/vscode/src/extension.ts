/**
 * The activation shell — deliberately thin.
 *
 * Everything worth testing lives in `./binary` and `./handshake`, neither of which imports
 * `vscode`. What is left here is wiring: read a setting, spawn a process, put a string in
 * the status bar.
 *
 * The rule that governs this file and every later one:
 *
 * > **TypeScript computes affordances. The CLI computes verdicts.**
 *
 * An affordance is a navigation or authoring convenience whose failure mode is *not
 * helping*. A verdict is a statement about whether the corpus is sound. Verdicts cross the
 * process boundary as JSON from the pinned binary; this extension renders them and never
 * derives them. A TypeScript re-implementation of the checks is the failure the whole RFC
 * set exists to close — one downstream project already wrote ~1,600 lines of it, in Python,
 * and it has drifted.
 */

import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import * as vscode from 'vscode'

import { pinPath, resolveBinary, type Resolution } from './binary.ts'
import { DARK, findClaims, LIGHT, shouldDecorate, TAGS, type Tag } from './claims.ts'
import { DEFAULT_OPTIONS, type Finding, type RepoCondition } from './diagnostics.ts'
import {
  hoverFor,
  lineOfTarget,
  nodeById,
  referencesTo,
  relationshipCandidates,
  resolveFrom,
  scaffold,
  scalarAt,
  slugify,
  sorted,
  targetCandidates,
  type GraphReport,
} from './graph.ts'
import { describe, readHandshake, type Handshake } from './handshake.ts'
import {
  runCorpusViews,
  runRefViews,
  runReports,
  runVocabulary,
  type CorpusViews,
  type Outcome,
  type RefViews,
} from './report-run.ts'
import { Cached, debounce, headOid, spawn, type CacheKey } from './runner.ts'
import {
  readonlyUpdate,
  schemaDelta,
  TASKS,
  type SchemaSettings,
} from './settings.ts'
import {
  corpusTree,
  healthTree,
  localRef,
  openQuestionsTree,
  phasesTree,
  sanghaTree,
  statusLine,
} from './tree/model.ts'
import { NodeTree } from './tree/provider.ts'
import {
  completions,
  inVerbPosition,
  marks,
  subjectLine,
  type VocabularyReport,
} from './vocabulary.ts'

const run = promisify(execFile)

interface State {
  resolution: Resolution
  handshake: Handshake | null
}

let state: State | null = null
let status: vscode.StatusBarItem
let diagnostics: vscode.DiagnosticCollection
let conditions: RepoCondition[] = []
let summary: string | null = null

interface Views {
  corpus: NodeTree
  open: NodeTree
  phases: NodeTree
  health: NodeTree
  sangha: NodeTree
}
let views: Views | null = null

/**
 * Bumped by every save. The reports read the working tree, not the commit, so an OID
 * alone would serve a stale answer for every edit made before committing — which is most
 * of them.
 */
let generation = 0
const cache = new Cached<Outcome>()
const corpusViews = new Cached<CorpusViews>()

/**
 * Bumped by a ref change or a sangha edit, and by nothing else.
 *
 * `phases` costs three git processes per ref — 1.26s against a repository with 23
 * settled resolutions — so it must not ride the save path. See `report-run.ts`.
 */
let refGeneration = 0
const refViews = new Cached<RefViews>()

/**
 * The verb list, fetched once and re-fetched when the vendored prelude changes.
 *
 * Held rather than re-run per keystroke: the list is a property of the pinned binary and
 * the vendored `GRAPH.md`, neither of which moves while somebody types a commit message.
 * The *check* is per-message and does re-run — debounced.
 */
let vocabulary: VocabularyReport | null = null
let commitDiagnostics: vscode.DiagnosticCollection
let claimStyles: Map<Tag, vscode.TextEditorDecorationType> | null = null

/**
 * The corpus, as the CLI resolved it. Refreshed on the save path with the other reports.
 *
 * Held rather than fetched per keystroke: a completion list is a lookup in this, and
 * spawning a process on every character typed in a YAML file would be absurd.
 */
let graph: GraphReport | null = null

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0)
  status.command = 'yidam.showHealth'
  context.subscriptions.push(status)

  diagnostics = vscode.languages.createDiagnosticCollection('yidam')
  commitDiagnostics = vscode.languages.createDiagnosticCollection('yidam-commit')
  context.subscriptions.push(diagnostics, commitDiagnostics)

  // The commit input is a real text document with language id `scminput`, so language
  // features register against it like any other.
  const SCM: vscode.DocumentSelector = { language: 'scminput' }
  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider(SCM, { provideCompletionItems: verbs }),
  )
  const checkCommit = debounce(debounceMs(), (doc: vscode.TextDocument) => void checkSubject(doc))
  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document.languageId !== 'scminput') return
      checkCommit(e.document)
    }),
    vscode.workspace.onDidCloseTextDocument((doc) => {
      if (doc.languageId === 'scminput') commitDiagnostics.delete(doc.uri)
    }),
  )

  views = {
    corpus: new NodeTree(workspaceFolder),
    open: new NodeTree(workspaceFolder),
    phases: new NodeTree(workspaceFolder),
    health: new NodeTree(workspaceFolder),
    sangha: new NodeTree(workspaceFolder),
  }
  context.subscriptions.push(
    vscode.window.createTreeView('yidam.corpus', { treeDataProvider: views.corpus }),
    vscode.window.createTreeView('yidam.openQuestions', { treeDataProvider: views.open }),
    vscode.window.createTreeView('yidam.phases', { treeDataProvider: views.phases }),
    vscode.window.createTreeView('yidam.health', { treeDataProvider: views.health }),
    vscode.window.createTreeView('yidam.sangha', { treeDataProvider: views.sangha }),
  )

  context.subscriptions.push(
    vscode.commands.registerCommand('yidam.showBinaryStatus', showStatus),
    vscode.commands.registerCommand('yidam.refreshDiagnostics', refreshAll),
    vscode.commands.registerCommand('yidam.refreshViews', refreshAll),
    vscode.commands.registerCommand('yidam.blessBaseline', blessBaseline),
    vscode.commands.registerCommand('yidam.checkoutPhase', checkoutPhase),
    vscode.commands.registerCommand('yidam.regen', () => task('regen')),
    vscode.commands.registerCommand('yidam.buildIndex', () => task('index-build')),
    vscode.commands.registerCommand('yidam.vendorStatus', () => task('yidam-vendor-status')),
    vscode.commands.registerCommand('yidam.showHealth', showHealth),
  )

  const debounced = debounce(debounceMs(), () => void report())
  context.subscriptions.push(
    // On save: the reports read the working tree, so this is when the answer changes.
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (!doc.uri.fsPath.includes('.yidam')) return
      // A sangha edit changes the sangha view and nothing the corpus reports say. Sending
      // it down the ref path keeps `phases` off the save path, which is the whole reason
      // the two groups exist.
      if (doc.uri.fsPath.includes('.yidam/sangha')) refGeneration += 1
      else generation += 1
      debounced()
    }),
    // On ref change: a checkout moves the corpus without touching a file.
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration('yidam.lint.showBaselined')) {
        cache.invalidate()
        void report()
      }
    }),
  )

  // Re-check when the pin changes: `.yidam.toml` is what says which yidam governs this
  // corpus, so editing it is exactly when a stale resolution becomes wrong.
  const folder = workspaceFolder()
  if (folder) {
    // `.yidam.toml` says which yidam governs this corpus; the vendored GRAPH.md carries
    // the vocabulary's prose. Both are re-read on change — the second because
    // re-vendoring is exactly how `resolve`, `scope` and `adopt` arrived.
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(folder, '{.yidam.toml,.yidam/.vendor/prelude/GRAPH.md}'),
    )
    watcher.onDidChange(() => void refresh())
    watcher.onDidCreate(() => void refresh())
    context.subscriptions.push(watcher)
  }
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration('yidam.path')) void refresh()
    }),
  )

  const gitHead = folder
    ? vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(folder, '.git/HEAD'))
    : null
  if (gitHead) {
    gitHead.onDidChange(() => {
      refGeneration += 1
      void report()
    })
    context.subscriptions.push(gitHead)
  }

  // ── the three guards ──────────────────────────────────────────────────────
  claimStyles = createClaimStyles()
  context.subscriptions.push(...claimStyles.values())

  const redraw = debounce(80, () => decorateAll())
  context.subscriptions.push(
    vscode.window.onDidChangeVisibleTextEditors(() => redraw()),
    vscode.workspace.onDidChangeTextDocument(() => redraw()),
    vscode.window.onDidChangeActiveColorTheme(() => decorateAll()),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration('yidam.claims.decorate')) decorateAll()
      if (e.affectsConfiguration('yidam.vendor.protect')) void applyVendorGuard()
    }),
  )
  decorateAll()

  // Navigation over corpus YAML. Every provider is a lookup in the held graph plus a read
  // of the line under the cursor; none of them parses YAML and none decides a verdict.
  const CORPUS: vscode.DocumentSelector = { language: 'yaml', scheme: 'file' }
  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider(CORPUS, { provideDefinition: definition }),
    vscode.languages.registerReferenceProvider(CORPUS, { provideReferences: references }),
    vscode.languages.registerHoverProvider(CORPUS, { provideHover: hover }),
    vscode.languages.registerCompletionItemProvider(
      CORPUS,
      { provideCompletionItems: corpusCompletion },
      ' ',
      '/',
      '.',
    ),
    vscode.commands.registerCommand('yidam.newNode', newNode),
  )

  context.subscriptions.push(
    vscode.tasks.registerTaskProvider('yidam', {
      provideTasks: miseTasks,
      resolveTask: () => undefined,
    }),
  )
  for (const t of TASKS) {
    context.subscriptions.push(
      vscode.commands.registerCommand(`yidam.task.${t.name}`, () => task(t.name)),
    )
  }
  context.subscriptions.push(
    vscode.commands.registerCommand('yidam.applySchemaSettings', () => applySchemas(true)),
  )

  await applyVendorGuard()
  await refresh()
  void applySchemas(false)
}

export function deactivate(): void {
  status?.dispose()
  diagnostics?.dispose()
  commitDiagnostics?.dispose()
  claimStyles?.forEach((d) => d.dispose())
}

// ── corpus navigation ─────────────────────────────────────────────────────────

function graphOf(corpus: CorpusViews): void {
  graph = corpus.graph
}

/** The corpus-relative id of a document, or null when it is not in the corpus. */
function nodeIdOf(uri: vscode.Uri): string | null {
  const folder = workspaceFolder()
  if (!folder || !graph) return null
  const prefix = `${folder}/${graph.corpus_dir}/`
  return uri.fsPath.startsWith(prefix) ? uri.fsPath.slice(prefix.length) : null
}

function uriOf(id: string): vscode.Uri | null {
  const folder = workspaceFolder()
  if (!folder || !graph) return null
  return vscode.Uri.file(`${folder}/${graph.corpus_dir}/${id}`)
}

/** The `target:` scalar under the cursor, resolved to a corpus id. */
function targetUnderCursor(
  document: vscode.TextDocument,
  position: vscode.Position,
): { id: string; scalar: NonNullable<ReturnType<typeof scalarAt>> } | null {
  const id = nodeIdOf(document.uri)
  if (id === null) return null
  const scalar = scalarAt(document.lineAt(position.line).text, position.character, 'target')
  if (!scalar) return null
  const resolved = resolveFrom(id, scalar.value)
  return resolved ? { id: resolved, scalar } : null
}

/**
 * Ctrl-click through an edge.
 *
 * Offered without an existence check. `exists` is the gate's answer, computed by the CLI
 * with the same two lines `dangling_edge` uses, and a second one here would be the drift the
 * whole contract exists to prevent — silently disagreeing about which edges are broken. When
 * the path is wrong VS Code says the file cannot be opened, which is a better signal than a
 * jump that quietly does nothing.
 */
function definition(
  document: vscode.TextDocument,
  position: vscode.Position,
): vscode.Location | null {
  const hit = targetUnderCursor(document, position)
  const uri = hit && uriOf(hit.id)
  return uri ? new vscode.Location(uri, new vscode.Position(0, 0)) : null
}

/**
 * Inbound edges — the traversal nothing surfaced.
 *
 * `used-by` covers catalog entries only, and `orphan-in` reports the *absence* of inbound
 * edges without ever naming the present ones. On a target scalar this answers about the
 * target; anywhere else in the file, about the file.
 */
async function references(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.Location[]> {
  if (!graph) return []
  const subject = targetUnderCursor(document, position)?.id ?? nodeIdOf(document.uri)
  if (subject === null) return []
  const out: vscode.Location[] = []
  for (const ref of referencesTo(graph, subject)) {
    const uri = uriOf(ref.from)
    if (!uri) continue
    let line = 0
    try {
      line = lineOfTarget((await vscode.workspace.openTextDocument(uri)).getText(), ref.target)
    } catch {
      // The report says the file is there; if it cannot be opened, the top of it is still
      // the honest answer rather than dropping the reference.
    }
    out.push(new vscode.Location(uri, new vscode.Position(line, 0)))
  }
  return out
}

/** What is on the other end of this edge, without leaving the node. */
function hover(document: vscode.TextDocument, position: vscode.Position): vscode.Hover | null {
  if (!graph) return null
  const hit = targetUnderCursor(document, position)
  if (!hit) return null
  const text = hoverFor(graph, hit.id)
  if (!text) return null
  return new vscode.Hover(
    new vscode.MarkdownString(text),
    new vscode.Range(position.line, hit.scalar.start, position.line, hit.scalar.end),
  )
}

/** The relationship named on the nearest `relationship:` line of this link entry. */
function relationshipNear(document: vscode.TextDocument, line: number): string {
  for (let i = line; i >= 0 && i > line - 8; i -= 1) {
    const m = /^\s*(?:-\s*)?relationship:\s*(\S+)/.exec(document.lineAt(i).text)
    if (m) return m[1].replace(/^["']|["']$/g, '')
    // A new list entry above with no relationship yet — stop rather than borrow the
    // previous link's.
    if (i < line && /^\s*-\s*\w+:/.test(document.lineAt(i).text)) break
  }
  return ''
}

function corpusCompletion(
  document: vscode.TextDocument,
  position: vscode.Position,
): vscode.CompletionItem[] {
  const id = nodeIdOf(document.uri)
  if (!graph || id === null) return []
  const line = document.lineAt(position.line).text

  const onRelationship = /^\s*(?:-\s*)?relationship:/.test(line)
  const onTarget = /^\s*(?:-\s*)?target:/.test(line)
  if (!onRelationship && !onTarget) return []

  const node = nodeById(graph, id)
  const candidates = onRelationship
    ? relationshipCandidates(graph, node?.class ?? '')
    : targetCandidates(graph, id, relationshipNear(document, position.line))

  // Replace whatever is already typed after the key, so a partial path does not end up
  // concatenated with the completion.
  const written = scalarAt(line, position.character, onRelationship ? 'relationship' : 'target')
  const keyEnd = line.indexOf(':') + 1
  const range = new vscode.Range(
    position.line,
    written ? written.start : Math.min(keyEnd + 1, line.length),
    position.line,
    Math.max(position.character, written ? written.end : 0),
  )

  return sorted(candidates).map((c) => {
    const item = new vscode.CompletionItem(
      c.label,
      onRelationship ? vscode.CompletionItemKind.EnumMember : vscode.CompletionItemKind.File,
    )
    item.detail = c.detail
    if (c.documentation) item.documentation = new vscode.MarkdownString(c.documentation)
    item.sortText = c.sortText
    item.range = range
    return item
  })
}

/**
 * Create a node, and refuse to create one without an edge.
 *
 * The link is asked for *before* the file is written, and cancelling at that step writes
 * nothing. A node with no outgoing edge is a lint error the moment it exists, so a command
 * that scaffolded one would be offering to break the gate — politely, with a wizard.
 */
async function newNode(): Promise<void> {
  const folder = workspaceFolder()
  if (!folder || !graph) {
    void vscode.window.showWarningMessage('yidam: no corpus graph yet — is the binary resolved?')
    return
  }
  const g = graph

  const cls = await vscode.window.showQuickPick(
    g.classes.map((c) => ({ label: c.class, description: c.label, detail: c.description })),
    { title: 'New node — class', matchOnDetail: true },
  )
  if (!cls) return

  const label = await vscode.window.showInputBox({ title: 'New node — label', ignoreFocusOut: true })
  if (!label) return
  const name = await vscode.window.showInputBox({
    title: 'New node — filename',
    value: slugify(label),
    ignoreFocusOut: true,
    validateInput: (v) =>
      /^[a-z0-9]+(-[a-z0-9]+)*$/.test(v)
        ? undefined
        : 'kebab-case, lowercase. Filenames are identity — renaming one severs every edge into it.',
  })
  if (!name) return
  const description = await vscode.window.showInputBox({
    title: 'New node — description',
    prompt: 'The node\'s content. One concept, 2–10 sentences.',
    ignoreFocusOut: true,
  })
  if (!description) return

  const id = `${cls.label}/${name}.yml`
  const relationship = await vscode.window.showQuickPick(
    sorted(relationshipCandidates(g, cls.label)).map((c) => ({
      label: c.label,
      description: c.detail,
      detail: c.documentation,
    })),
    { title: 'New node — its first edge (required)', matchOnDetail: true },
  )
  if (!relationship) return
  const target = await vscode.window.showQuickPick(
    sorted(targetCandidates(g, id, relationship.label)).map((c) => ({
      label: c.label,
      description: c.detail,
    })),
    { title: `New node — ${relationship.label} →`, matchOnDetail: true },
  )
  if (!target) return

  const uri = vscode.Uri.file(`${folder}/${g.corpus_dir}/${id}`)
  const body = scaffold(
    {
      class: cls.label,
      name,
      label,
      description,
      relationship: relationship.label,
      target: resolveFrom(id, target.label),
    },
    g.classes.find((c) => c.class === cls.label),
  )
  await vscode.workspace.fs.writeFile(uri, Buffer.from(body, 'utf8'))
  await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(uri))
}

// ── claim tags ────────────────────────────────────────────────────────────────

/**
 * One decoration per tag, carrying both palettes.
 *
 * VS Code picks the arm; nothing here reads the theme to choose a colour. The theme is
 * consulted only to decide whether to decorate at all.
 */
function createClaimStyles(): Map<Tag, vscode.TextEditorDecorationType> {
  const styles = new Map<Tag, vscode.TextEditorDecorationType>()
  for (const tag of TAGS) {
    styles.set(
      tag,
      vscode.window.createTextEditorDecorationType({
        borderRadius: '3px',
        borderWidth: '1px',
        borderStyle: 'solid',
        light: {
          backgroundColor: LIGHT[tag].bg,
          color: LIGHT[tag].fg,
          borderColor: LIGHT[tag].border,
        },
        dark: {
          backgroundColor: DARK[tag].bg,
          color: DARK[tag].fg,
          borderColor: DARK[tag].border,
        },
      }),
    )
  }
  return styles
}

function decorateAll(): void {
  if (!claimStyles) return
  const on = shouldDecorate(
    vscode.window.activeColorTheme.kind,
    vscode.workspace.getConfiguration('yidam').get<boolean>('claims.decorate') ?? true,
  )
  for (const editor of vscode.window.visibleTextEditors) {
    const hits = on ? findClaims(editor.document.getText()) : []
    for (const [tag, style] of claimStyles) {
      editor.setDecorations(
        style,
        hits
          .filter((h) => h.tag === tag)
          .map((h) => new vscode.Range(h.line, h.start, h.line, h.end)),
      )
    }
  }
}

// ── the vendored prelude ──────────────────────────────────────────────────────

/**
 * Make `.yidam/.vendor/` read-only in fact rather than by convention.
 *
 * `AGENTS.md` says an edit there "is silently discarded on the next update", and nothing
 * enforced it. The failure is quiet and delayed: the edit works locally, survives review,
 * and disappears at the next `mise run yidam-vendor-update`.
 *
 * Workspace scope, and idempotent — `readonlyUpdate` returns null when the setting already
 * says this, so activation does not put a settings diff in every session.
 */
async function applyVendorGuard(): Promise<void> {
  const want = vscode.workspace.getConfiguration('yidam').get<boolean>('vendor.protect') ?? true
  const files = vscode.workspace.getConfiguration('files')
  const current = files.inspect<Record<string, boolean>>('readonlyInclude')?.workspaceValue
  const next = readonlyUpdate(current, want)
  if (next === null) return
  try {
    await files.update('readonlyInclude', next, vscode.ConfigurationTarget.Workspace)
  } catch {
    // No workspace to write to (a single loose folder, or a read-only settings file).
    // The guard is a convenience; failing to apply it must not fail activation.
  }
}

// ── schemas ───────────────────────────────────────────────────────────────────

/**
 * Offer to apply `yidam schema --settings`.
 *
 * It works today, and it is the entire editor story: a third-party extension, a manual
 * step at genesis, and a second manual step whenever the schema set changes. This makes the
 * copy-paste a notification with a button.
 *
 * Merged, never replaced — `yaml.schemas` is somewhere people put their own mappings.
 * `force` is the palette command, which says so even when nothing is stale.
 */
async function applySchemas(force: boolean): Promise<void> {
  const folder = workspaceFolder()
  if (!folder || !state?.resolution.command) return
  const r = await spawn(state.resolution.command, ['schema', '--settings'], folder)
  let desired: SchemaSettings
  try {
    desired = JSON.parse(r.stdout) as SchemaSettings
  } catch {
    return
  }
  const deltas = schemaDelta(desired, (key) => {
    const dot = key.lastIndexOf('.')
    return vscode.workspace
      .getConfiguration(key.slice(0, dot))
      .inspect<Record<string, unknown>>(key.slice(dot + 1))?.workspaceValue
  })
  if (deltas.length === 0) {
    if (force) void vscode.window.showInformationMessage('yidam: schema settings are current.')
    return
  }

  const apply = 'Apply'
  const count = deltas.reduce((n, d) => n + d.changed.length, 0)
  const choice = force
    ? apply
    : await vscode.window.showInformationMessage(
        `yidam: ${count} schema mapping(s) missing from this workspace's settings.`,
        { detail: deltas.flatMap((d) => d.changed).join('\n') },
        apply,
      )
  if (choice !== apply) return

  for (const d of deltas) {
    const dot = d.key.lastIndexOf('.')
    await vscode.workspace
      .getConfiguration(d.key.slice(0, dot))
      .update(d.key.slice(dot + 1), d.value, vscode.ConfigurationTarget.Workspace)
  }
}

// ── tasks ─────────────────────────────────────────────────────────────────────

/** The inherited `mise.yidam.toml` layer, reachable from `Run Task` and the palette. */
function miseTasks(): vscode.Task[] {
  const folder = vscode.workspace.workspaceFolders?.[0]
  if (!folder) return []
  return TASKS.map((t) => {
    const task = new vscode.Task(
      { type: 'yidam', task: t.name },
      folder,
      t.name,
      'yidam',
      new vscode.ShellExecution('mise', ['run', t.name]),
    )
    task.detail = t.title
    return task
  })
}

/**
 * Completion in the commit box, from the vocabulary the pinned binary reports.
 *
 * Offered only in the verb position — once `: ` is behind the cursor the user is writing
 * prose, and thirty verbs there is noise rather than help.
 */
function verbs(
  document: vscode.TextDocument,
  position: vscode.Position,
): vscode.CompletionItem[] {
  if (!vocabulary) return []
  const line = document.lineAt(position.line).text
  if (!inVerbPosition(line, position.character, position.line)) return []
  return completions(vocabulary).map((c) => {
    const item = new vscode.CompletionItem(c.label, vscode.CompletionItemKind.Keyword)
    item.detail = c.detail
    item.documentation = new vscode.MarkdownString(c.documentation)
    item.insertText = c.insertText
    item.sortText = c.sortText
    return item
  })
}

/** Check the subject line as it is typed, and publish what the CLI says about it. */
async function checkSubject(document: vscode.TextDocument): Promise<void> {
  const folder = workspaceFolder()
  if (!folder || !state?.resolution.command) return
  const subject = subjectLine(document.getText()).trim()
  if (subject.length === 0) {
    commitDiagnostics.delete(document.uri)
    return
  }
  const report = await runVocabulary(state.resolution.command, folder, spawn, subject)
  if (!report?.subject) {
    commitDiagnostics.delete(document.uri)
    return
  }
  commitDiagnostics.set(
    document.uri,
    marks(report.subject).map((m) => {
      const d = new vscode.Diagnostic(
        new vscode.Range(0, m.start, 0, m.end),
        m.message,
        severityOf(m.severity === 'error' ? 'error' : m.severity === 'warn' ? 'warning' : 'information'),
      )
      d.code = m.code
      d.source = 'yidam (vocabulary)'
      return d
    }),
  )
}

/** Drop every cached answer and ask again. One button, all five views. */
function refreshAll(): Promise<void> {
  cache.invalidate()
  corpusViews.invalidate()
  refViews.invalidate()
  return report()
}

function debounceMs(): number {
  return vscode.workspace.getConfiguration('yidam').get<number>('diagnostics.debounceMs') ?? 400
}

/** Run the reports and publish the result. Silent when there is no usable binary. */
async function report(): Promise<void> {
  const folder = workspaceFolder()
  if (!folder || !state?.resolution.command) return
  const bin = state.resolution.command

  const showBaselined =
    vscode.workspace.getConfiguration('yidam').get<boolean>('lint.showBaselined') ??
    DEFAULT_OPTIONS.showBaselined

  const key: CacheKey = { oid: await headOid(folder, spawn), generation }
  const outcome = await cache.get(key, () =>
    runReports(bin, folder, { showBaselined }, spawn),
  )

  if (!outcome.ok) {
    // Contract skew mid-session: say so and stop asserting verdicts rather than
    // rendering a half-understood report.
    state.handshake = outcome.handshake
    conditions = []
    diagnostics.clear()
    render()
    return
  }

  conditions = outcome.mapped.conditions
  publish(folder, outcome.mapped.findings)

  const oid = key.oid
  const [corpus, refs] = await Promise.all([
    corpusViews.get({ oid, generation }, () => runCorpusViews(bin, folder, spawn)),
    refViews.get({ oid, generation: refGeneration }, () => runRefViews(bin, folder, spawn)),
  ])

  if (views) {
    views.corpus.replace(
      corpus.corpusIndex && corpus.openQuestions
        ? corpusTree(corpus.corpusIndex, corpus.openQuestions)
        : [],
    )
    views.open.replace(corpus.openQuestions ? openQuestionsTree(corpus.openQuestions) : [])
    views.phases.replace(refs.phases ? phasesTree(refs.phases) : [])
    graphOf(corpus)
    views.health.replace(
      healthTree({ lint: outcome.lint, graph: outcome.graph, index: corpus.indexStatus }),
    )
    views.sangha.replace(refs.sangha ? sanghaTree(refs.sangha) : [])
    // The sangha view is hidden until there is one, rather than showing an empty box in
    // every repository that governs itself individually.
    void vscode.commands.executeCommand(
      'setContext',
      'yidam.collective',
      refs.sangha?.collective ?? false,
    )
  }

  summary = statusLine(corpus.status, corpus.indexStatus)
  render()
}

function publish(root: string, findings: Finding[]): void {
  diagnostics.clear()
  const byFile = new Map<string, vscode.Diagnostic[]>()
  for (const f of findings) {
    const line = Math.max(0, f.line - 1)
    const d = new vscode.Diagnostic(
      new vscode.Range(line, 0, line, Number.MAX_SAFE_INTEGER),
      f.message,
      severityOf(f.level),
    )
    d.code = f.code
    d.source = f.source
    // The rationale as hover — `--explain` without a second command.
    d.relatedInformation = []
    if (f.rationale) {
      d.relatedInformation.push(
        new vscode.DiagnosticRelatedInformation(
          new vscode.Location(vscode.Uri.file(`${root}/${f.file}`), new vscode.Position(line, 0)),
          f.rationale,
        ),
      )
    }
    // Faded, the way an unused import is: present, and not the thing to look at.
    if (f.baselined) d.tags = [vscode.DiagnosticTag.Unnecessary]
    const list = byFile.get(f.file) ?? []
    list.push(d)
    byFile.set(f.file, list)
  }
  for (const [file, list] of byFile) {
    diagnostics.set(vscode.Uri.file(`${root}/${file}`), list)
  }
}

function severityOf(level: Finding['level']): vscode.DiagnosticSeverity {
  switch (level) {
    case 'error':
      return vscode.DiagnosticSeverity.Error
    case 'warning':
      return vscode.DiagnosticSeverity.Warning
    case 'information':
      return vscode.DiagnosticSeverity.Information
    case 'hint':
      return vscode.DiagnosticSeverity.Hint
  }
}

/**
 * The one action a stale baseline entry has.
 *
 * Run in a terminal rather than silently: blessing rewrites a committed record of the
 * corpus's debt, and it should be as visible as any other commit-shaped act.
 */
async function blessBaseline(): Promise<void> {
  const term = vscode.window.createTerminal('yidam lint --bless')
  term.sendText('yidam lint --bless')
  term.show()
}

/**
 * The status bar's click target.
 *
 * Health when there is a corpus to be healthy; the binary dialog when there is not. The
 * status bar is one control and the question behind it changes: before a binary resolves,
 * "which yidam is this" is the only answer worth giving.
 */
async function showHealth(): Promise<void> {
  if (!state?.resolution.command || state.handshake?.ok === false) {
    await showStatus()
    return
  }
  await vscode.commands.executeCommand('yidam.health.focus')
}

/** Run a mise task where the user can watch it. */
function task(name: string): void {
  const term = vscode.window.createTerminal(`mise run ${name}`)
  term.sendText(`mise run ${name}`)
  term.show()
}

/**
 * Switching branch is confirmed, then run in a terminal.
 *
 * Not because a checkout is dangerous, but because a *click in a tree* moving the working
 * tree under an unsaved editor is a surprise. The confirmation is what makes the row safe
 * to click by accident; the terminal is what makes the result readable when git refuses.
 */
async function checkoutPhase(ref: unknown): Promise<void> {
  if (typeof ref !== 'string' || ref.length === 0) return
  const target = localRef(ref)
  const go = 'Switch'
  const choice = await vscode.window.showWarningMessage(
    `Switch to ${target}?`,
    { modal: true, detail: 'Runs `git switch` in a terminal.' },
    go,
  )
  if (choice !== go) return
  const term = vscode.window.createTerminal(`git switch ${target}`)
  term.sendText(`git switch ${target}`)
  term.show()
}

function workspaceFolder(): string | null {
  const folders = vscode.workspace.workspaceFolders
  return folders && folders.length > 0 ? folders[0].uri.fsPath : null
}

async function refresh(): Promise<void> {
  const folder = workspaceFolder()
  if (!folder) return

  const configured = vscode.workspace.getConfiguration('yidam').get<string>('path') ?? ''
  const resolution = await resolveBinary({ configured, workspace: folder })

  let handshake: Handshake | null = null
  if (resolution.command) {
    handshake = await shakeHands(resolution.command, folder)
  }

  state = { resolution, handshake }
  vocabulary =
    resolution.command && handshake?.ok
      ? await runVocabulary(resolution.command, folder, spawn)
      : null
  render()
  await report()
}

/**
 * Ask the binary for the cheapest report there is and read only its envelope.
 *
 * `status` is used rather than `lint` because it walks the corpus once and gates on
 * nothing — the handshake must not depend on whether the corpus currently passes.
 */
async function shakeHands(command: string, cwd: string): Promise<Handshake> {
  try {
    const { stdout } = await run(command, ['status', '--format', 'json'], { cwd })
    return readHandshake(stdout)
  } catch (err) {
    // A nonzero exit still carries stdout for gating commands; `status` does not gate, so
    // reaching here means the binary failed to run at all.
    const e = err as { stdout?: string; stderr?: string }
    const stdout = typeof e.stdout === 'string' ? e.stdout : ''
    const stderr = typeof e.stderr === 'string' ? e.stderr : ''
    // A stale binary rejects `--format` outright: nothing on stdout, a usage message on
    // stderr, nonzero exit. Hand both streams over so that reads as contract skew rather
    // than as a broken install.
    if (stdout.trim().length > 0 || stderr.trim().length > 0) {
      return readHandshake(stdout, stderr)
    }
    return {
      ok: false,
      kind: 'not-json',
      message: `Could not run ${command}: ${(err as Error).message}`,
    }
  }
}

function render(): void {
  if (!state) return
  const { resolution, handshake } = state

  if (!resolution.command) {
    status.text = '$(warning) yidam: not found'
    status.tooltip = `${resolution.reason}\n\nVerdict features are disabled.`
    status.show()
    return
  }
  if (handshake && !handshake.ok) {
    status.text = '$(warning) yidam: contract skew'
    status.tooltip = handshake.message
    status.show()
    return
  }
  // The corpus summary once there is one; the handshake until then. A reader wants to know
  // how big the corpus is and how much of it is open — not which binary answered, which is
  // a question they ask once.
  const line = summary ?? describe(handshake!)
  const provenance = `${describe(handshake!)}\nResolved from ${resolution.reason}\n${resolution.command}`

  const stale = conditions.filter((c) => c.kind === 'stale-baseline').length
  const gate = conditions.some((c) => c.kind === 'graph-gate')
  if (stale > 0 || gate) {
    status.text = `$(alert) ${line}`
    status.tooltip = `${conditions.map((c) => c.message).join('\n\n')}\n\n${provenance}`
    status.show()
    return
  }
  status.text = `$(check) ${line}`
  status.tooltip = provenance
  status.show()
}

/**
 * Not-found is a first-class state with exactly one action.
 *
 * Building the binary is offered as a terminal command rather than performed: the
 * extension does not get to decide which yidam governs a corpus, and running the build
 * where the user can watch it is the difference between a tool and a surprise.
 */
async function showStatus(): Promise<void> {
  if (!state) return
  const { resolution, handshake } = state

  if (!resolution.command) {
    const build = 'Run mise run yidam-build'
    const choice = await vscode.window.showWarningMessage(
      `yidam binary not found. ${resolution.reason}`,
      build,
    )
    if (choice === build) {
      const term = vscode.window.createTerminal('yidam-build')
      term.sendText('mise run yidam-build')
      term.show()
    }
    return
  }

  if (handshake && !handshake.ok) {
    const update = 'Run mise run yidam-vendor-update'
    const choice = await vscode.window.showWarningMessage(handshake.message, update)
    if (choice === update) {
      const term = vscode.window.createTerminal('yidam-vendor-update')
      term.sendText('mise run yidam-vendor-update')
      term.show()
    }
    return
  }

  const folder = workspaceFolder()
  await vscode.window.showInformationMessage(
    `${describe(handshake!)} — resolved from ${resolution.reason}.`,
    { modal: false, detail: `${resolution.command}\npin: ${folder ? pinPath(folder) : 'unknown'}` },
  )
}
