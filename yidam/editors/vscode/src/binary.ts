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
export type Origin = 'setting' | 'repo' | 'path' | 'mise' | 'none'

/**
 * Where `mise run yidam-build` installs, relative to the workspace.
 *
 * A derived repository pins the yidam commit that governs its corpus in `.yidam.toml`, and
 * the build task installs *that* commit here — beside the pin, rather than into
 * `~/.cargo/bin`, which is one location per machine while the pin is one per repository.
 */
export const REPO_LOCAL = '.yidam/bin/yidam'

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
 * Resolve: setting, this repository's own build, PATH, mise shim, not found.
 *
 * The order is deliberate. An explicit setting is somebody's decision and outranks
 * discovery. `.yidam/bin/yidam` is the binary built from the commit this repository pins,
 * and it comes next because a machine-wide install is not a statement about *this* corpus —
 * see [`REPO_LOCAL`]. `PATH` is what the user's own shell would run, which is what makes the
 * editor's answers reproducible by hand; under mise it is the repo-local one anyway, since
 * the build task puts `.yidam/bin` first. The mise shim is the fallback rather than the
 * first choice, because a shim silently overriding an explicit install would be the editor
 * disagreeing with the terminal.
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

  // The repository's own build, ahead of PATH. On a machine with several yidam
  // repositories `~/.cargo/bin/yidam` is whichever one built last, so preferring PATH here
  // would let one repository's pinned binary answer for another's corpus — the exact
  // disagreement between the editor and CI that this surface exists to prevent, arriving by
  // a route nobody chose. An explicit `yidam.path` still outranks it: that is somebody's
  // decision, and this is a default.
  //
  // `.exe` too, because this is a direct file test rather than a PATH lookup, and `where`
  // on Windows would have found it.
  for (const rel of [REPO_LOCAL, `${REPO_LOCAL}.exe`]) {
    const local = path.join(input.workspace, rel)
    if (fileExists(local)) {
      return { origin: 'repo', command: local, reason: `${rel}, built from this repo's pin` }
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
    reason:
      'no .yidam/bin/yidam, none on PATH, and no mise shim in this workspace. ' +
      'Run `mise run yidam-build`.',
  }
}

/** Absolute path to a workspace's provenance pin, whether or not it exists. */
export function pinPath(workspace: string): string {
  return path.join(workspace, '.yidam.toml')
}
