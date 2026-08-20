/**
 * Running the reports, and not running them more than necessary.
 *
 * The reports walk the whole corpus per invocation. That is fine at hundreds of nodes and
 * is not fine on every keystroke, so two things bound the work: a debounce, and a cache
 * keyed by the git OID the corpus is currently at.
 *
 * The OID is the right key because it is exactly what the reports are a function of —
 * plus the working tree, which is why a save invalidates regardless. Keying on a timestamp
 * would re-run after every idle minute; keying on nothing would re-run on every event.
 *
 * No `vscode` import. Timers and process spawning are injected, so the caching logic is
 * exercised by plain node with no editor and no clock.
 */

import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

const exec = promisify(execFile)

export interface RunResult {
  stdout: string
  stderr: string
  code: number
}

export type Spawn = (bin: string, args: string[], cwd: string) => Promise<RunResult>

/**
 * Run and keep both streams whatever the exit code.
 *
 * `lint` and `graph-check` gate: a nonzero exit is a verdict, not a failure to produce one,
 * and the envelope is on stdout regardless. An extension treating `exit != 0` as "binary
 * unusable" would go blind exactly when the corpus needs attention.
 */
export const spawn: Spawn = async (bin, args, cwd) => {
  try {
    const { stdout, stderr } = await exec(bin, args, { cwd, maxBuffer: 32 * 1024 * 1024 })
    return { stdout, stderr, code: 0 }
  } catch (err) {
    const e = err as { stdout?: string; stderr?: string; code?: number }
    return { stdout: e.stdout ?? '', stderr: e.stderr ?? '', code: e.code ?? 1 }
  }
}

/** The revision the corpus is at, or null outside a repository. */
export async function headOid(cwd: string, run: Spawn = spawn): Promise<string | null> {
  const r = await run('git', ['rev-parse', 'HEAD'], cwd)
  const oid = r.stdout.trim()
  return r.code === 0 && oid.length > 0 ? oid : null
}

export interface CacheKey {
  /** Git OID, or null when there is none to key on. */
  oid: string | null
  /** Bumped by any save, because the reports read the working tree, not the commit. */
  generation: number
}

export function sameKey(a: CacheKey, b: CacheKey): boolean {
  return a.oid === b.oid && a.generation === b.generation
}

/**
 * A single-flight, key-aware cache.
 *
 * Single-flight matters more than the cache: a save while a run is in flight would
 * otherwise start a second walk of the same corpus, and on a large one the two would
 * finish out of order and the older answer could win.
 */
export class Cached<T> {
  private key: CacheKey | null = null
  private value: T | null = null
  private inflight: Promise<T> | null = null
  private inflightKey: CacheKey | null = null

  async get(key: CacheKey, compute: () => Promise<T>): Promise<T> {
    if (this.key && this.value !== null && sameKey(this.key, key)) {
      return this.value
    }
    if (this.inflight && this.inflightKey && sameKey(this.inflightKey, key)) {
      return this.inflight
    }
    this.inflightKey = key
    this.inflight = compute().then((v) => {
      // Only publish if nothing newer started meanwhile — a late answer for a stale key
      // must not overwrite a fresh one.
      if (this.inflightKey && sameKey(this.inflightKey, key)) {
        this.key = key
        this.value = v
        this.inflight = null
      }
      return v
    })
    return this.inflight
  }

  invalidate(): void {
    this.key = null
    this.value = null
  }
}

/**
 * Collapse a burst of events into one trailing call.
 *
 * Trailing rather than leading: the interesting state is the one after the burst, and a
 * leading edge would report on the corpus as it was before the save that prompted it.
 */
export function debounce<A extends unknown[]>(
  ms: number,
  fn: (...args: A) => void,
  schedule: (cb: () => void, ms: number) => unknown = setTimeout,
  cancel: (h: unknown) => void = (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
): (...args: A) => void {
  let handle: unknown = null
  return (...args: A) => {
    if (handle !== null) cancel(handle)
    handle = schedule(() => {
      handle = null
      fn(...args)
    }, ms)
  }
}
