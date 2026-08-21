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

test('this repository\'s own build outranks a machine-wide one', async () => {
  // `~/.cargo/bin/yidam` is whichever yidam built last on this machine. `.yidam/bin/yidam`
  // is the commit THIS repository pins. Preferring PATH would let one repo's binary answer
  // for another's corpus.
  const r = await resolveBinary({
    configured: '',
    workspace: '/w',
    fileExists: (p) => p === '/w/.yidam/bin/yidam',
    lookupOnPath: async () => '/usr/local/bin/yidam',
    miseWhich: async () => null,
  })
  assert.equal(r.origin, 'repo')
  assert.equal(r.command, '/w/.yidam/bin/yidam')
})

test('an explicit setting still outranks the repository\'s own build', async () => {
  // The repo-local binary is a default, not a decision. A setting is a decision.
  const r = await resolveBinary({
    configured: '/opt/yidam',
    workspace: '/w',
    fileExists: () => true,
    lookupOnPath: async () => '/usr/local/bin/yidam',
    miseWhich: async () => null,
  })
  assert.equal(r.origin, 'setting')
  assert.equal(r.command, '/opt/yidam')
})

test('with no repo-local build, PATH answers as before', async () => {
  const r = await resolveBinary({
    configured: '',
    workspace: '/w',
    fileExists: () => false,
    lookupOnPath: async () => '/usr/local/bin/yidam',
    miseWhich: async () => null,
  })
  assert.equal(r.origin, 'path')
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
  assert.match(r.reason, /no .yidam\/bin\/yidam, none on PATH/)
  assert.match(r.reason, /mise run yidam-build/, 'the reason names the fix')
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
