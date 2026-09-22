/**
 * The act frame, held to its shape without a binary — #608, RFC-0030 Phase 3.
 *
 * Two things are tested, and they are the two things `src/lib/act.ts` decides. What goes down
 * the pipe: three JSON-RPC lines, an argument object that has one field for `propose` and
 * none for `cycle`, and `serve --mcp` on the argv and nothing else. What comes back up: a
 * tool result, a refusal with its token intact, or *no server* — and which of the three
 * `act` states the handshake reported, because those are three different repairs.
 *
 * Everything else — whether the corpus opted in, whether the checkout has an author, what a
 * finding licenses — is the binary's, and `yidam/cli/tests/mcp_act_tier.rs` holds it there.
 * The route's method and origin rule is tested here too, next to the frame it guards, in the
 * same shape `origin.mjs` gave the read rule: a security rule with no test is a coin flip.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { frame, spawnAct } from '../src/lib/act.ts'
import { actRefusal, dryRunOf } from '../src/lib/api.ts'

const lines = (text) =>
  text
    .split('\n')
    .filter((l) => l !== '')
    .map((l) => JSON.parse(l))

// A fake server that records what it was sent and answers with what the test hands it.
const server = (stdout, { stderr = '', code = 0 } = {}) => {
  const calls = []
  const exec = async (command, args, options) => {
    calls.push({ command, args, options })
    return { stdout, stderr, code }
  }
  return { exec, calls }
}

const handshake = (yidam) =>
  JSON.stringify({
    jsonrpc: '2.0',
    id: 1,
    result: {
      protocolVersion: '2024-11-05',
      capabilities: { tools: {}, resources: {}, yidam },
    },
  })

const tool = (text, isError = false) =>
  JSON.stringify({ jsonrpc: '2.0', id: 2, result: { content: [{ type: 'text', text }], isError } })

const DECLARED = { contract: '0.24.0', act: true, corpus: { domain: 'streamflow' } }

test('propose is framed with dry_run and nothing else — force has no field to arrive in', () => {
  for (const dryRun of [true, false]) {
    const sent = lines(frame('propose', dryRun))
    assert.equal(sent.length, 3)
    assert.equal(sent[0].method, 'initialize')
    assert.equal(sent[0].id, 1)
    assert.equal(sent[1].method, 'notifications/initialized')
    assert.equal(sent[1].id, undefined, 'a notification carries no id')
    assert.equal(sent[2].method, 'tools/call')
    assert.equal(sent[2].id, 2)
    assert.equal(sent[2].params.name, 'propose')
    assert.deepEqual(sent[2].params.arguments, { dry_run: dryRun })
  }
  assert.ok(!frame('propose', false).includes('force'), 'the frame must never spell `force`')
})

test('cycle is framed with no arguments', () => {
  const sent = lines(frame('cycle', true))
  assert.equal(sent[2].params.name, 'cycle')
  assert.deepEqual(sent[2].params.arguments, {}, 'dry_run is propose’s and cycle has none')
})

test('the argv is `serve --mcp` in the corpus root, and the frame is the whole of stdin', async () => {
  const { exec, calls } = server([handshake(DECLARED), tool('{"due":0}')].join('\n'))
  await spawnAct({ command: '/bin/yidam', root: '/corpus', exec }, 'cycle')
  assert.equal(calls.length, 1)
  assert.deepEqual(calls[0].args, ['serve', '--mcp'])
  assert.equal(calls[0].command, '/bin/yidam')
  assert.equal(calls[0].options.cwd, '/corpus')
  assert.equal(calls[0].options.input, frame('cycle', false))
})

test('a declared server’s answer is the tool’s JSON, with the contract it declared', async () => {
  const { exec } = server([handshake(DECLARED), tool('{"branch":"propose/abc","proposals":[]}')].join('\n'))
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'propose', true)
  assert.equal(result.ok, true)
  assert.equal(result.act, 'declared')
  assert.equal(result.contract, '0.24.0')
  assert.deepEqual(result.result, { branch: 'propose/abc', proposals: [] })
})

test('responses are matched by id, not by position', async () => {
  // A server may interleave a notification or log line; the frame reads ids.
  const { exec } = server(
    [
      JSON.stringify({ jsonrpc: '2.0', method: 'notifications/message', params: {} }),
      tool('{"due":1}'),
      handshake(DECLARED),
    ].join('\n'),
  )
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'cycle')
  assert.equal(result.ok, true)
  assert.deepEqual(result.result, { due: 1 })
})

test('a tool refusal is carried whole, with the token at its head', async () => {
  const text =
    'capability-not-supported: `propose` is served only by a server declaring the `act` ' +
    'capability, and this one declares it false.'
  const { exec } = server([handshake({ ...DECLARED, act: false }), tool(text, true)].join('\n'))
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'propose')
  assert.equal(result.ok, false)
  assert.equal(result.kind, 'refused')
  assert.equal(result.act, 'undeclared')
  assert.equal(result.contract, '0.24.0')
  assert.equal(result.error, text, 'the refusal is not reworded')
})

test('a handshake with no act key is a binary that predates the tier, not one that said no', async () => {
  const { exec } = server(
    [handshake({ contract: '0.21.0', corpus: {} }), tool('unknown tool: propose', true)].join('\n'),
  )
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'propose')
  assert.equal(result.ok, false)
  assert.equal(result.kind, 'refused')
  assert.equal(result.act, 'predates-act')
})

test('a server that refused to start is no server, and stderr is the account', async () => {
  // `act = true` with no git identity: `serve` prints why and exits before reading stdin.
  const { exec } = server('', {
    stderr: 'refusing to serve: `[serve] act = true` but no git author identity',
    code: 2,
  })
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'propose')
  assert.equal(result.ok, false)
  assert.equal(result.kind, 'no-server')
  assert.match(result.error, /no git author identity/)
})

test('a binary that could not be started is no server', async () => {
  const exec = async () => {
    throw Object.assign(new Error('spawn y ENOENT'), { code: 'ENOENT' })
  }
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'cycle')
  assert.equal(result.ok, false)
  assert.equal(result.kind, 'no-server')
  assert.match(result.error, /ENOENT/)
})

test('something on stdout that is not JSON-RPC is no server, not a partial answer', async () => {
  const { exec } = server(['yidam 0.13.0 serving streamflow', handshake(DECLARED)].join('\n'))
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'cycle')
  assert.equal(result.ok, false)
  assert.equal(result.kind, 'no-server')
  assert.match(result.error, /not JSON-RPC/)
})

test('a protocol error on the call is no server — the frame was not understood', async () => {
  const { exec } = server(
    [
      handshake(DECLARED),
      JSON.stringify({ jsonrpc: '2.0', id: 2, error: { code: -32601, message: 'method not found' } }),
    ].join('\n'),
  )
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'cycle')
  assert.equal(result.ok, false)
  assert.equal(result.kind, 'no-server')
  assert.match(result.error, /-32601/)
})

test('a tool answer from a server that did not declare act is not rendered as a write', async () => {
  // Cannot happen on a conforming binary — the contract says the call is refused — which is
  // exactly why the frame refuses to understand it rather than trusting it.
  const { exec } = server([handshake({ ...DECLARED, act: false }), tool('{"written":{}}')].join('\n'))
  const result = await spawnAct({ command: 'y', root: '/c', exec }, 'propose')
  assert.equal(result.ok, false)
  assert.equal(result.kind, 'no-server')
  assert.match(result.error, /act=undeclared/)
})

// ---- the route's policy, before anything is spawned ----

const req = (method, headers = {}, url = 'http://127.0.0.1:8788/api/act/propose') =>
  new Request(url, { method, headers })

const SAME = { host: '127.0.0.1:8788', origin: 'http://127.0.0.1:8788' }

test('the act tier is reached by POST and nothing else', async () => {
  for (const method of ['GET', 'HEAD', 'PUT', 'DELETE']) {
    const refusal = actRefusal(req(method, SAME))
    assert.ok(refusal, `${method} was allowed through`)
    assert.equal(refusal.status, 405)
    assert.equal(refusal.headers.get('allow'), 'POST')
  }
  assert.equal(actRefusal(req('POST', SAME)), null)
})

test('a POST with no Origin did not come from a browser page, and is refused', () => {
  // `wrongOrigin` lets a missing `Origin` through because simple GETs omit it. A browser
  // never omits it on a POST, so on this route the allowance would only ever serve a client
  // that is not a page — and this route is where that distinction costs something.
  const refusal = actRefusal(req('POST', { host: '127.0.0.1:8788' }))
  assert.ok(refusal)
  assert.equal(refusal.status, 403)
})

test('a POST from another origin is refused, and there is no flag that allows it', () => {
  const refusal = actRefusal(
    req('POST', { host: '127.0.0.1:8788', origin: 'http://localhost:8788' }),
  )
  assert.ok(refusal)
  assert.equal(refusal.status, 403)
  const evil = actRefusal(req('POST', { host: '127.0.0.1:8788', origin: 'https://example.com' }))
  assert.equal(evil.status, 403)
})

test('dry_run is read off the query string, and absent is the contract’s own default', () => {
  const at = (qs) => dryRunOf(req('POST', SAME, `http://127.0.0.1:8788/api/act/propose${qs}`))
  assert.equal(at(''), false)
  assert.equal(at('?dry_run=true'), true)
  assert.equal(at('?dry_run=1'), true)
  assert.equal(at('?dry_run=false'), false)
  assert.equal(at('?dry_run=yes'), false, 'only the two spellings; anything else is not a preview')
})
