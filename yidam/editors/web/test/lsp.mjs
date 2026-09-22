/**
 * The LSP framing and the client, driven against a fake child — #607, RFC-0030 Phase 2.
 *
 * The three failure modes the issue names are the three sections here. Framing is tested on
 * chunk boundaries the pipe actually produces: a header split mid-word, two messages in one
 * read, a body that lags its header. Correlation is tested by answering out of order and by
 * answering an id nothing asked for. Death is tested by ending the fake process while a
 * request is in flight and holding the promise to a rejection that carries stderr.
 *
 * Nothing here needs a binary, and nothing here reads a diagnostic: what a finding means is
 * the binary's, and `yidam/cli/src/cmd/lsp.rs` pins the server's half of this conversation.
 */

import { strict as assert } from 'node:assert'
import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { test } from 'node:test'
import { Framer, LspClient, LspExited, describeExit, frame } from '../src/lib/lsp.ts'

const body = (message) => Buffer.from(JSON.stringify(message))

test('a frame is the byte length, not the string length', () => {
  const framed = frame({ jsonrpc: '2.0', method: 'x', params: { text: 'ü — ✓' } })
  const text = framed.toString('utf8')
  const [header, rest] = text.split('\r\n\r\n')
  assert.equal(header, `Content-Length: ${Buffer.byteLength(rest, 'utf8')}`)
  assert.notEqual(Buffer.byteLength(rest, 'utf8'), rest.length, 'the probe must be non-ASCII')
  assert.deepEqual(new Framer().feed(framed), [
    { jsonrpc: '2.0', method: 'x', params: { text: 'ü — ✓' } },
  ])
})

test('the framer reassembles across the boundaries a pipe produces', () => {
  const framer = new Framer()
  const a = frame({ id: 1, result: 'a' })
  const b = frame({ id: 2, result: 'b' })
  const c = frame({ method: 'c', params: { long: 'x'.repeat(500) } })
  const all = Buffer.concat([a, b, c])

  // Header split mid-word, then the rest of both a and b plus the head of c, then c's tail.
  assert.deepEqual(framer.feed(all.subarray(0, 7)), [])
  assert.deepEqual(framer.feed(all.subarray(7, a.length + b.length + 30)), [
    { id: 1, result: 'a' },
    { id: 2, result: 'b' },
  ])
  assert.deepEqual(framer.feed(all.subarray(a.length + b.length + 30)), [
    { method: 'c', params: { long: 'x'.repeat(500) } },
  ])
  // Nothing is left over.
  assert.deepEqual(framer.feed(Buffer.alloc(0)), [])
})

test('header names are case-insensitive and other headers are tolerated', () => {
  const b = body({ id: 9, result: null })
  const framed = Buffer.concat([
    Buffer.from(`content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH: ${b.length}\r\n\r\n`),
    b,
  ])
  assert.deepEqual(new Framer().feed(framed), [{ id: 9, result: null }])
})

test('a header block with no length is refused, not skipped', () => {
  assert.throws(
    () => new Framer().feed(Buffer.from('Content-Type: text/plain\r\n\r\n{}')),
    /no Content-Length/,
  )
})

// ── the client ───────────────────────────────────────────────────────────────

/** A process that is three pipes and an exit, with the server's half written by the test. */
function fakeChild() {
  const child = new EventEmitter()
  child.stdin = new PassThrough()
  child.stdout = new PassThrough()
  child.stderr = new PassThrough()
  child.killed = false
  child.kill = () => {
    child.killed = true
    // Real children exit asynchronously; so does this one.
    setImmediate(() => child.emit('exit', null, 'SIGTERM'))
    return true
  }
  const inbound = new Framer()
  const received = []
  const waiters = []
  child.stdin.on('data', (chunk) => {
    for (const m of inbound.feed(chunk)) {
      received.push(m)
      const i = waiters.findIndex((w) => w.match(m))
      if (i !== -1) waiters.splice(i, 1)[0].resolve(m)
    }
  })
  /** The next message the client sends that matches, or one already received. */
  const next = (match) =>
    new Promise((resolve) => {
      const seen = received.find(match)
      if (seen) resolve(seen)
      else waiters.push({ match, resolve })
    })
  const send = (message) => child.stdout.write(frame(message))
  return { child, received, next, send }
}

function start(fake) {
  let spawned
  const client = LspClient.start({
    command: '/nowhere/yidam',
    root: '/tmp/corpus',
    spawn: (command, args, options) => {
      spawned = { command, args, options }
      return fake.child
    },
  })
  return { client, spawned: () => spawned }
}

test('initialize goes first, carries the root as a file URI, and is followed by initialized', async () => {
  const fake = fakeChild()
  const { client, spawned } = start(fake)
  assert.deepEqual(spawned().args, ['serve', '--lsp'])
  assert.equal(spawned().options.cwd, '/tmp/corpus')

  const pending = client.initialize('/tmp/corpus')
  const init = await fake.next((m) => m.method === 'initialize')
  assert.equal(init.params.rootUri, 'file:///tmp/corpus')
  assert.equal(init.params.clientInfo.name, 'yidam-edit')
  fake.send({ jsonrpc: '2.0', id: init.id, result: { capabilities: { experimental: { yidam: { unsavedInstances: true } } } } })

  const result = await pending
  assert.equal(result.capabilities.experimental.yidam.unsavedInstances, true)
  const initialized = await fake.next((m) => m.method === 'initialized')
  assert.equal(initialized.id, undefined, 'initialized is a notification')
  assert.equal(fake.received.indexOf(initialized), fake.received.indexOf(init) + 1)
  client.kill()
  await client.exited
})

test('replies are matched by id, in any order, and a stray id is dropped', async () => {
  const fake = fakeChild()
  const { client } = start(fake)
  const a = client.request('a', {})
  const b = client.request('b', {})
  const [ra, rb] = await Promise.all([
    fake.next((m) => m.method === 'a'),
    fake.next((m) => m.method === 'b'),
  ])
  assert.notEqual(ra.id, rb.id)
  fake.send({ jsonrpc: '2.0', id: 999, result: 'nobody asked' })
  fake.send({ jsonrpc: '2.0', id: rb.id, result: 'B' })
  fake.send({ jsonrpc: '2.0', id: ra.id, result: 'A' })
  assert.equal(await b, 'B')
  assert.equal(await a, 'A')
  client.kill()
  await client.exited
})

test('a null result resolves null — the barrier relies on this', async () => {
  const fake = fakeChild()
  const { client } = start(fake)
  const p = client.request('$/yidam/barrier', {})
  const req = await fake.next((m) => m.method === '$/yidam/barrier')
  fake.send({ jsonrpc: '2.0', id: req.id, result: null })
  assert.equal(await p, null)
  client.kill()
  await client.exited
})

test('an error reply rejects with its code and message', async () => {
  const fake = fakeChild()
  const { client } = start(fake)
  const p = client.request('nope', {})
  const req = await fake.next((m) => m.method === 'nope')
  fake.send({ jsonrpc: '2.0', id: req.id, error: { code: -32601, message: 'method not found' } })
  await assert.rejects(p, /-32601: method not found/)
  client.kill()
  await client.exited
})

test('notifications reach the listener and never the pending map', async () => {
  const fake = fakeChild()
  const { client } = start(fake)
  const seen = []
  client.onNotification((method, params) => seen.push({ method, params }))
  fake.send({ jsonrpc: '2.0', method: 'textDocument/publishDiagnostics', params: { uri: 'file:///x', diagnostics: [] } })
  await new Promise((r) => setImmediate(r))
  assert.deepEqual(seen, [
    { method: 'textDocument/publishDiagnostics', params: { uri: 'file:///x', diagnostics: [] } },
  ])
  client.kill()
  await client.exited
})

test('a child dying mid-request rejects the request with what stderr said', async () => {
  const fake = fakeChild()
  const { client } = start(fake)
  const p = client.request('textDocument/hover', {})
  await fake.next((m) => m.method === 'textDocument/hover')
  fake.child.stderr.write('error: not a corpus: /tmp/corpus\n')
  await new Promise((r) => setImmediate(r))
  fake.child.emit('exit', 2, null)

  await assert.rejects(p, (e) => {
    assert.ok(e instanceof LspExited)
    assert.equal(e.exit.code, 2)
    assert.match(e.message, /exited 2/)
    assert.match(e.message, /not a corpus/)
    return true
  })
  assert.equal(client.isAlive, false)
  // After death, a request is refused immediately rather than written to nothing.
  await assert.rejects(client.request('x', {}), LspExited)
  const exit = await client.exited
  assert.equal(exit.code, 2)
})

test('a binary that cannot be started ends the client the way an exit does', async () => {
  const fake = fakeChild()
  const { client } = start(fake)
  const p = client.initialize('/tmp/corpus')
  fake.child.emit('error', new Error('spawn /nowhere/yidam ENOENT'))
  await assert.rejects(p, /could not be started[\s\S]*ENOENT/)
})

test('bytes that are not the protocol end the session rather than being read past', async () => {
  const fake = fakeChild()
  const { client } = start(fake)
  const p = client.request('x', {})
  fake.child.stdout.write('yidam 0.5.0\nusage: yidam <command>\r\n\r\n')
  await assert.rejects(p, /no Content-Length/)
  assert.equal(fake.child.killed, true)
})

test('shutdown asks, then tells, then kills what has not gone', async () => {
  // A server that answers shutdown and exits on exit — the ordinary case.
  {
    const fake = fakeChild()
    const { client } = start(fake)
    const done = client.shutdown()
    const req = await fake.next((m) => m.method === 'shutdown')
    fake.send({ jsonrpc: '2.0', id: req.id, result: null })
    await fake.next((m) => m.method === 'exit')
    fake.child.emit('exit', 0, null)
    const exit = await done
    assert.equal(exit.code, 0)
    assert.equal(fake.child.killed, false)
  }
  // A server that wedges: killed after the grace period, and the promise still resolves.
  {
    const fake = fakeChild()
    const { client } = start(fake)
    const exit = await client.shutdown(20)
    assert.equal(fake.child.killed, true)
    assert.equal(exit.signal, 'SIGTERM')
  }
})

test('describeExit names the outcome and carries the stderr tail', () => {
  assert.equal(describeExit({ code: 1, signal: null, stderr: '' }), 'yidam serve --lsp exited 1')
  assert.equal(
    describeExit({ code: null, signal: 'SIGKILL', stderr: '  boom \n' }),
    'yidam serve --lsp killed by SIGKILL:\nboom',
  )
  assert.equal(
    describeExit({ code: null, signal: null, stderr: '' }),
    'yidam serve --lsp could not be started',
  )
})
