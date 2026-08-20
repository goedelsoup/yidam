import assert from 'node:assert/strict'
import { test } from 'node:test'

import { resolveBinary } from '../src/binary.ts'

const never = async () => null

test('an explicit setting outranks discovery', async () => {
  const r = await resolveBinary({
    configured: '/opt/yidam',
    workspace: '/w',
    fileExists: (p) => p === '/opt/yidam',
    lookupOnPath: async () => '/usr/bin/yidam',
    miseWhich: never,
  })
  assert.equal(r.origin, 'setting')
  assert.equal(r.command, '/opt/yidam')
})

test('a setting pointing at nothing fails rather than falling through', async () => {
  // Silently using a different binary than the one somebody configured is how an editor
  // starts lying about which rules it is enforcing.
  const r = await resolveBinary({
    configured: '/opt/missing',
    workspace: '/w',
    fileExists: () => false,
    lookupOnPath: async () => '/usr/bin/yidam',
    miseWhich: async () => '/shim/yidam',
  })
  assert.equal(r.origin, 'none')
  assert.equal(r.command, null)
  assert.match(r.reason, /not a file/)
})

test('PATH is preferred over the mise shim', async () => {
  // What the user's own shell would run is what makes the editor reproducible by hand.
  const r = await resolveBinary({
    configured: '',
    workspace: '/w',
    fileExists: () => false,
    lookupOnPath: async () => '/usr/bin/yidam',
    miseWhich: async () => '/shim/yidam',
  })
  assert.equal(r.origin, 'path')
  assert.equal(r.command, '/usr/bin/yidam')
})

test('the mise shim is the fallback, not the first choice', async () => {
  const r = await resolveBinary({
    configured: '',
    workspace: '/w',
    fileExists: () => false,
    lookupOnPath: never,
    miseWhich: async () => '/shim/yidam',
  })
  assert.equal(r.origin, 'mise')
})

test('not found is a state with a reason, not an exception', async () => {
  const r = await resolveBinary({
    configured: '',
    workspace: '/w',
    fileExists: () => false,
    lookupOnPath: never,
    miseWhich: never,
  })
  assert.equal(r.origin, 'none')
  assert.equal(r.command, null)
  assert.match(r.reason, /no yidam on PATH/)
})

test('whitespace is not a configured path', async () => {
  const r = await resolveBinary({
    configured: '   ',
    workspace: '/w',
    fileExists: () => false,
    lookupOnPath: async () => '/usr/bin/yidam',
    miseWhich: never,
  })
  assert.equal(r.origin, 'path')
})
