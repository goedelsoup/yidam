/**
 * Finding the `yidam` binary that governs a workspace.
 *
 * No `vscode` import: everything here is a pure function over inputs the extension host
 * supplies, so it is testable with `node --test` and needs no Electron. That split is the
 * point — the logic worth getting right should not require an editor to exercise.
 *
 * **The extension never bundles, downloads, or builds a binary.** A derived repository's
 * `.yidam.toml` records which yidam commit governs its corpus; installing some other one
 * behind the user's back would make the editor's verdicts disagree with CI's, which is the
 * single failure this whole surface exists to avoid.
 */

import { execFile } from 'node:child_process'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { promisify } from 'node:util'

const run = promisify(execFile)

/** Where a resolved binary came from. Shown to the user, because "which yidam" matters. */
export type Origin = 'setting' | 'path' | 'mise' | 'none'

export interface Resolution {
  origin: Origin
  /** Absolute path, or null when nothing was found. */
  command: string | null
  /** Why resolution ended where it did — rendered verbatim in the not-found state. */
  reason: string
}

export interface ResolveInput {
  /** The `yidam.path` setting, empty when unset. */
  configured: string
  /** Workspace folder to resolve mise shims against. */
  workspace: string
  /** Injected so tests need no real filesystem or PATH. */
  fileExists?: (p: string) => boolean
  lookupOnPath?: (name: string) => Promise<string | null>
  miseWhich?: (workspace: string, name: string) => Promise<string | null>
}

const defaultFileExists = (p: string): boolean => {
  try {
    return fs.statSync(p).isFile()
  } catch {
    return false
  }
}

const defaultLookupOnPath = async (name: string): Promise<string | null> => {
  try {
    const { stdout } = await run(process.platform === 'win32' ? 'where' : 'which', [name])
    const first = stdout.split('\n')[0].trim()
    return first.length > 0 ? first : null
  } catch {
    return null
  }
}

const defaultMiseWhich = async (workspace: string, name: string): Promise<string | null> => {
  try {
    const { stdout } = await run('mise', ['which', name], { cwd: workspace })
    const found = stdout.trim()
    return found.length > 0 ? found : null
  } catch {
    return null
  }
}

/**
 * Resolve in the order RFC-0016 fixes: setting, PATH, mise shim, not found.
 *
 * The order is deliberate. An explicit setting is somebody's decision and outranks
 * discovery; `PATH` is what the user's own shell would run, which is what makes the
 * editor's answers reproducible by hand; the mise shim is the repository's own pinned
 * toolchain and is the fallback rather than the first choice, because a shim silently
 * overriding an explicit install would be the editor disagreeing with the terminal.
 */
export async function resolveBinary(input: ResolveInput): Promise<Resolution> {
  const fileExists = input.fileExists ?? defaultFileExists
  const lookupOnPath = input.lookupOnPath ?? defaultLookupOnPath
  const miseWhich = input.miseWhich ?? defaultMiseWhich

  const configured = input.configured.trim()
  if (configured.length > 0) {
    if (fileExists(configured)) {
      return { origin: 'setting', command: configured, reason: 'yidam.path' }
    }
    // A wrong setting is not a reason to fall through: silently using a different binary
    // than the one someone configured is how an editor starts lying about which rules it
    // is enforcing.
    return {
      origin: 'none',
      command: null,
      reason: `yidam.path is set to ${configured}, which is not a file.`,
    }
  }

  const onPath = await lookupOnPath('yidam')
  if (onPath) {
    return { origin: 'path', command: onPath, reason: 'found on PATH' }
  }

  const shim = await miseWhich(input.workspace, 'yidam')
  if (shim) {
    return { origin: 'mise', command: shim, reason: 'mise shim in this workspace' }
  }

  return {
    origin: 'none',
    command: null,
    reason: 'no yidam on PATH and no mise shim in this workspace.',
  }
}

/** Absolute path to a workspace's provenance pin, whether or not it exists. */
export function pinPath(workspace: string): string {
  return path.join(workspace, '.yidam.toml')
}
