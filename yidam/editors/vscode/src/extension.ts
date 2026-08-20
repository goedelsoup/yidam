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

import { pinPath, resolveBinary, type Resolution } from './binary'
import { describe, readHandshake, type Handshake } from './handshake'

const run = promisify(execFile)

interface State {
  resolution: Resolution
  handshake: Handshake | null
}

let state: State | null = null
let status: vscode.StatusBarItem

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0)
  status.command = 'yidam.showBinaryStatus'
  context.subscriptions.push(status)

  context.subscriptions.push(
    vscode.commands.registerCommand('yidam.showBinaryStatus', showStatus),
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

  await refresh()
}

export function deactivate(): void {
  status?.dispose()
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
  status.text = `$(check) ${describe(handshake!)}`
  status.tooltip = `Resolved from ${resolution.reason}\n${resolution.command}`
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
