/**
 * Which corpus, and which binary answers for it.
 *
 * One resolution per process, memoised, because "which yidam" is a property of the checkout
 * the server was started in and not of a request. Re-resolving per request would let the
 * answer change under a reader mid-session — the editor disagreeing with itself, which is a
 * smaller version of the disagreement `binary.ts` exists to prevent.
 *
 * The root arrives through `YIDAM_EDIT_ROOT`, set by `bin/yidam-edit.mjs`. It is a candidate
 * rather than a validated path: the rule for what counts as a corpus root is #549's and it
 * lives in the binary, so the first spawn is what refuses.
 */

import { realpathSync } from 'node:fs'
import { resolveBinary, type Resolution } from './binary.ts'

export interface Session {
  root: string
  binary: Resolution
}

let cached: Promise<Session> | null = null

export function session(): Promise<Session> {
  cached ??= resolve()
  return cached
}

/**
 * The root, with symlinks followed.
 *
 * `git rev-parse --show-toplevel` returns a real path, so the binary's answer is always
 * canonical while the flag's argument need not be. On macOS `/tmp` is a symlink to
 * `/private/tmp`, which is enough on its own: asking for `/tmp/corpus` and being told
 * `/private/tmp/corpus` is the same directory reported two ways, and comparing the strings
 * raises a mismatch warning about a corpus that is perfectly correct. A warning that cries
 * wolf is a warning people learn to scroll past, which costs more than not having it.
 *
 * Falls back to the given path when it does not resolve — a root that does not exist is the
 * binary's refusal to make, not this function's.
 */
function canonical(dir: string): string {
  try {
    return realpathSync(dir)
  } catch {
    return dir
  }
}

async function resolve(): Promise<Session> {
  const root = canonical(process.env.YIDAM_EDIT_ROOT ?? process.cwd())
  // `configured` is empty: this surface has no settings store. The extension reads
  // `yidam.path` from the editor's configuration, and the equivalent here would be a flag
  // this design has deliberately not added — see `bin/args.mjs` on the two missing flags.
  const binary = await resolveBinary({ configured: '', workspace: root })
  return { root, binary }
}

/** Reset between tests. Not exported to the app. */
export function resetSessionForTests(): void {
  cached = null
}
