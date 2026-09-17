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
  /**
   * Which run is current. Bumped by every start and by every `invalidate()`.
   *
   * The publish guard cannot be about the *key*, which is what #689's second half was: two
   * runs of the same corpus at the same `{ oid, generation }` are indistinguishable by key,
   * and `invalidate()` is reached by two paths that change neither. So a run compares the
   * epoch it started at against the current one, and publishes only if it is still the run
   * anyone is waiting on.
   */
  private epoch = 0

  async get(key: CacheKey, compute: () => Promise<T>): Promise<T> {
    if (this.key && this.value !== null && sameKey(this.key, key)) {
      return this.value
    }
    if (this.inflight && this.inflightKey && sameKey(this.inflightKey, key)) {
      return this.inflight
    }
    const epoch = ++this.epoch
    this.inflightKey = key
    this.inflight = compute().then(
      (v) => {
        // Only publish if nothing newer started meanwhile, and nothing dropped it — a late
        // answer for a superseded run must not overwrite a fresh one or resurrect itself
        // after a Refresh.
        if (epoch === this.epoch) {
          this.key = key
          this.value = v
          this.inflight = null
          this.inflightKey = null
        }
        return v
      },
      (err: unknown) => {
        // Robustness, not a live bug. Without this the rejected promise stays in `inflight`
        // and every later `get()` for the key re-throws it — the key is poisoned for the
        // life of the session. Unreachable today only because `spawn` catches everything
        // and `report-run`'s one unguarded `JSON.parse` runs on a string `readHandshake`
        // already parsed, which is an invariant two files away.
        if (epoch === this.epoch) {
          this.inflight = null
          this.inflightKey = null
        }
        throw err
      },
    )
    return this.inflight
  }

  /**
   * Drop every cached answer, including one still being computed.
   *
   * All four fields, and the epoch. Clearing `key`/`value` alone left `inflight` and
   * `inflightKey` set, so the next `get()` for the same key took the single-flight early
   * return and handed back the very run the caller had just asked to discard — and that run
   * then published, caching the pre-invalidation answer until a save or a checkout moved the
   * key. Refresh did nothing if pressed while a report was running, which is exactly when
   * someone reaches for it, and the `yidam.lint.showBaselined` toggle did nothing mid-run
   * and kept showing the old setting's findings (#689).
   *
   * The run already in flight is **not** cancelled — `execFile` is running and cannot be —
   * and it is deliberately not published either. Its result is an answer nobody is waiting
   * on, and the epoch bump is what says so; do not "fix" the publish guard back to comparing
   * keys, which cannot tell it from the run that replaced it.
   */
  invalidate(): void {
    this.key = null
    this.value = null
    this.inflight = null
    this.inflightKey = null
    this.epoch++
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
