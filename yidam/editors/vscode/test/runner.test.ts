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
