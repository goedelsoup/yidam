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
import { DEFAULT_OPTIONS, type Finding, type RepoCondition } from './diagnostics.ts'
import { describe, readHandshake, type Handshake } from './handshake.ts'
import {
  runCorpusViews,
  runRefViews,
  runReports,
  type CorpusViews,
  type Outcome,
  type RefViews,
} from './report-run.ts'
import { Cached, debounce, headOid, spawn, type CacheKey } from './runner.ts'
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

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0)
  status.command = 'yidam.showHealth'
  context.subscriptions.push(status)

  diagnostics = vscode.languages.createDiagnosticCollection('yidam')
  context.subscriptions.push(diagnostics)

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
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(folder, '.yidam.toml'),
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

  await refresh()
}

export function deactivate(): void {
  status?.dispose()
  diagnostics?.dispose()
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
