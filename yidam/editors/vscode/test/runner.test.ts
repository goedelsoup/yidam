import assert from 'node:assert/strict'
import { test } from 'node:test'

import { Cached, debounce, sameKey, type CacheKey } from '../src/runner.ts'

const key = (oid: string | null, generation = 0): CacheKey => ({ oid, generation })

test('a repeated key is served from cache', async () => {
  const c = new Cached<number>()
  let calls = 0
  const compute = async () => ++calls
  assert.equal(await c.get(key('a'), compute), 1)
  assert.equal(await c.get(key('a'), compute), 1)
  assert.equal(calls, 1)
})

test('a new commit re-runs', async () => {
  const c = new Cached<number>()
  let calls = 0
  const compute = async () => ++calls
  await c.get(key('a'), compute)
  await c.get(key('b'), compute)
  assert.equal(calls, 2)
})

test('a save re-runs even at the same commit', async () => {
  // The reports read the working tree, not the commit — an OID alone would serve a stale
  // answer for every edit made before committing, which is most of them.
  const c = new Cached<number>()
  let calls = 0
  const compute = async () => ++calls
  await c.get(key('a', 0), compute)
  await c.get(key('a', 1), compute)
  assert.equal(calls, 2)
})

test('concurrent asks for the same key share one run', async () => {
  // Single-flight: a save while a run is in flight must not start a second walk of the
  // same corpus.
  const c = new Cached<number>()
  let calls = 0
  const compute = async () => {
    calls++
    await new Promise((r) => setTimeout(r, 5))
    return calls
  }
  const [a, b] = await Promise.all([c.get(key('a'), compute), c.get(key('a'), compute)])
  assert.equal(calls, 1)
  assert.equal(a, b)
})

test('a late answer for a stale key does not overwrite a fresh one', async () => {
  const c = new Cached<string>()
  const slow = () => new Promise<string>((r) => setTimeout(() => r('old'), 20))
  const fast = async () => 'new'
  const first = c.get(key('a'), slow)
  const second = await c.get(key('b'), fast)
  await first
  assert.equal(second, 'new')
  // The stale run resolved last; the cache must still hold the newer key's value.
  assert.equal(await c.get(key('b'), async () => 'recomputed'), 'new')
})

test('invalidate forces the next ask to recompute', async () => {
  const c = new Cached<number>()
  let calls = 0
  const compute = async () => ++calls
  await c.get(key('a'), compute)
  c.invalidate()
  await c.get(key('a'), compute)
  assert.equal(calls, 2)
})

test('a null OID is a usable key', async () => {
  // Outside a git repository, or before the first commit.
  const c = new Cached<number>()
  let calls = 0
  const compute = async () => ++calls
  await c.get(key(null), compute)
  await c.get(key(null), compute)
  assert.equal(calls, 1)
  assert.equal(sameKey(key(null), key(null)), true)
  assert.equal(sameKey(key(null), key('a')), false)
})

test('debounce fires once, trailing, with the last arguments', () => {
  // Trailing: the interesting state is the one after the burst. A leading edge would
  // report on the corpus as it was before the save that prompted it.
  const seen: string[] = []
  const pending: (() => void)[] = []
  const d = debounce<[string]>(
    10,
    (s) => seen.push(s),
    (cb) => {
      pending.push(cb)
      return pending.length - 1
    },
    (h) => {
      pending[h as number] = () => {}
    },
  )
  d('first')
  d('second')
  d('third')
  pending.forEach((cb) => cb())
  assert.deepEqual(seen, ['third'])
})

test('invalidate() during an in-flight run makes the next ask recompute', async () => {
  // #689. `invalidate()` cleared `key`/`value` and left `inflight`/`inflightKey` set, so the
  // next `get()` for the same key took the single-flight early return and handed back the
  // promise that was already running when the caller asked for everything to be dropped.
  //
  // The cache key is `{ oid, generation }` and two call paths invalidate without changing
  // either — `refreshAll()` ("Drop every cached answer and ask again. One button, all five
  // views.") and the `yidam.lint.showBaselined` listener. So Refresh did nothing if pressed
  // while a report was running, which is precisely when someone reaches for it.
  const c = new Cached<string>()
  let release!: (v: string) => void
  const first = c.get(key('a'), () => new Promise<string>((r) => (release = r)))

  c.invalidate()

  let recomputed = false
  const second = c.get(key('a'), async () => {
    recomputed = true
    return 'FRESH'
  })
  release('STALE')

  assert.equal(await second, 'FRESH')
  assert.equal(recomputed, true, 'refresh must recompute, not reuse the abandoned run')
  assert.equal(await first, 'STALE', 'the abandoned run still resolves for its own caller')
})

test('an answer computed before invalidate() is not published after it', async () => {
  // The second half of the same bug, and the one a values-only test misses: the abandoned
  // run's `.then` still saw a matching `inflightKey` and wrote `key`/`value`, so the
  // pre-invalidation answer was cached until a save or a checkout moved the key.
  const c = new Cached<string>()
  let release!: (v: string) => void
  const first = c.get(key('a'), () => new Promise<string>((r) => (release = r)))
  c.invalidate()
  const second = c.get(key('a'), async () => 'FRESH')
  // Released before either is awaited: on the unfixed cache `second` *is* `first`, so
  // awaiting it first would deadlock rather than fail, and a regression test that hangs is
  // a worse signal than one that goes red.
  release('STALE')
  await Promise.allSettled([first, second])

  assert.equal(
    await c.get(key('a'), async () => 'recomputed'),
    'FRESH',
    'the abandoned run published over the answer that replaced it',
  )
})

test('the abandoned run does not publish even when it finishes first', async () => {
  // The ordering that decides *how* the publish guard has to be written, and the reason
  // `Cached` carries an epoch rather than simply clearing all four fields on `invalidate()`.
  //
  // Both runs are for the same `{ oid, generation }` — `refreshAll()` and the
  // `showBaselined` listener change neither — so key equality cannot tell them apart. When
  // the abandoned run resolves first it finds a matching `inflightKey` (its replacement just
  // set it to the same value), publishes STALE, and nulls `inflight`, after which the
  // replacement's own guard fails and FRESH is never cached at all. Clearing four fields
  // fixes the recompute and leaves this; only run identity fixes both.
  const c = new Cached<string>()
  let releaseStale!: (v: string) => void
  let releaseFresh: ((v: string) => void) | undefined

  const first = c.get(key('a'), () => new Promise<string>((r) => (releaseStale = r)))
  c.invalidate()
  const second = c.get(key('a'), () => new Promise<string>((r) => (releaseFresh = r)))

  // The abandoned run started first and the corpus did not shrink, so this is the ordinary
  // order, not the exotic one.
  releaseStale('STALE')
  await Promise.allSettled([first])
  releaseFresh?.('FRESH')
  await Promise.allSettled([second])

  assert.equal(
    await c.get(key('a'), async () => 'recomputed'),
    'FRESH',
    'the answer the caller waited for was lost to the one they discarded',
  )
})

test('a rejected compute does not poison the key', async () => {
  // Robustness, not a live bug: `runReports` is the only `compute` on this path, `spawn`
  // catches everything and returns a `RunResult`, and `report-run.ts`'s one unguarded
  // `JSON.parse` runs only after `readHandshake` parsed the same string. Without the
  // `.catch`, `inflight` stays set to the rejected promise and every later `get()` for that
  // key re-throws it — safe today because of an invariant two files away.
  const c = new Cached<string>()
  await assert.rejects(
    c.get(key('a'), async () => {
      throw new Error('boom')
    }),
  )
  assert.equal(await c.get(key('a'), async () => 'RECOVERED'), 'RECOVERED')
})
