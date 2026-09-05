/**
 * The flags, including the two that are absent.
 *
 * `--bind` and `--allow-origin` are not oversights and their absence is asserted here rather
 * than left to review. RFC-0030 declines both: a server that authenticates nobody should not
 * carry the flag that turns a loopback editor into the deployed reader #236 closed, and the
 * only legitimate client is the page this server served.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { DEFAULT_PORT, USAGE, parseArgs } from '../bin/args.mjs'

test('no flags opens the working directory on the default port', () => {
  const parsed = parseArgs([], '/tmp/corpus')
  assert.ok(parsed.ok)
  assert.deepEqual(parsed.args, { root: '/tmp/corpus', port: DEFAULT_PORT, open: true })
})

test('the default port is not the MCP server’s', () => {
  // RFC-0030 leaves coexistence open — whether an MCP server and this may run against one
  // corpus at once — and a shared default would answer it by collision.
  assert.notEqual(DEFAULT_PORT, 8787)
})

test('--root takes the next argument', () => {
  const parsed = parseArgs(['--root', '/srv/corpus'], '/tmp')
  assert.ok(parsed.ok)
  assert.equal(parsed.args.root, '/srv/corpus')
})

test('--root without a value is refused rather than defaulted', () => {
  const parsed = parseArgs(['--root'], '/tmp')
  assert.equal(parsed.ok, false)
  const next = parseArgs(['--root', '--no-open'], '/tmp')
  assert.equal(next.ok, false)
})

test('--port refuses anything that is not a port', () => {
  for (const bad of ['0', '65536', 'eight', '80.5', '-1']) {
    const parsed = parseArgs(['--port', bad], '/tmp')
    assert.equal(parsed.ok, false, `--port ${bad} was accepted`)
  }
  const good = parseArgs(['--port', '9000'], '/tmp')
  assert.ok(good.ok)
  assert.equal(good.args.port, 9000)
})

test('--no-open is a flag, not a value', () => {
  const parsed = parseArgs(['--no-open'], '/tmp')
  assert.ok(parsed.ok)
  assert.equal(parsed.args.open, false)
})

test('an unknown flag is refused, not ignored', () => {
  // `binary.ts`'s reason about a wrong `yidam.path`: silently doing something other than what
  // somebody asked for is how a tool starts lying about what it is doing.
  const parsed = parseArgs(['--bind', '0.0.0.0'], '/tmp')
  assert.equal(parsed.ok, false)
  assert.match(parsed.message, /unknown argument --bind/)
})

test('the flags RFC-0030 declined are still declined', () => {
  for (const declined of ['--bind', '--allow-origin', '--host']) {
    assert.ok(!USAGE.includes(declined), `${declined} has appeared in the usage text`)
    const parsed = parseArgs([declined, 'x'], '/tmp')
    assert.equal(parsed.ok, false, `${declined} is now accepted`)
  }
})

test('--help prints usage and is not an error', () => {
  const parsed = parseArgs(['--help'], '/tmp')
  assert.equal(parsed.ok, false)
  assert.equal(parsed.usage, true)
  assert.equal(parsed.message, USAGE)
})
