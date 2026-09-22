/**
 * The overlay bridge, supervised against a scripted server — #607, RFC-0030 Phase 2.
 *
 * The server here is a fake that behaves the way `lsp.rs` is pinned to behave: it answers
 * `initialize` with the capability block, publishes diagnostics for a buffer only when the
 * test's `judge` returns some (or when it returned some last time and now returns none), and
 * answers `$/yidam/barrier` with `null` after those publishes. Every verdict a test reads is
 * one the fake produced; what a real finding means is the binary's and is tested there.
 *
 * What is held here is supervision and addressing: which subscriber gets which verdict, what a
 * buffer's URI is, what happens when the child dies with a change in flight, when it is
 * started again and when the bridge gives up, and that the last page leaving stops the child.
 */

import { strict as assert } from 'node:assert'
import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { test } from 'node:test'
import { Framer, frame } from '../src/lib/lsp.ts'
import { OverlayBridge, docIdError, sseEvent } from '../src/lib/overlay.ts'

const ROOT = '/tmp/corpus'
const CORPUS_DIR = '.yidam/corpus'

/**
 * A scripted `yidam serve --lsp`. `judge(uri, text)` is the test's stand-in for the checks.
 * Returns the spawn function and a window onto every process it started.
 */
function scriptedServer({ judge = () => [], capabilities = { experimental: { yidam: { unsavedInstances: true } } } } = {}) {
  const children = []
  const spawn = (_command, args, options) => {
    const child = new EventEmitter()
    child.stdin = new PassThrough()
    child.stdout = new PassThrough()
    child.stderr = new PassThrough()
    child.killed = false
    child.alive = true
    /** Records but does not answer — a wedged server, for the in-flight tests. */
    child.mute = false
    child.args = args
    child.cwd = options.cwd
    child.received = []
    const texts = new Map()
    const published = new Set()
    const reply = (id, result) => child.stdout.write(frame({ jsonrpc: '2.0', id, result }))
    const publish = (uri, diagnostics) => {
      child.stdout.write(
        frame({ jsonrpc: '2.0', method: 'textDocument/publishDiagnostics', params: { uri, diagnostics } }),
      )
    }
    const framer = new Framer()
    child.stdin.on('data', (chunk) => {
      for (const m of framer.feed(chunk)) {
        child.received.push(m)
        if (!child.alive || child.mute) continue
        switch (m.method) {
          case 'initialize':
            reply(m.id, { capabilities })
            break
          case 'textDocument/didOpen':
            texts.set(m.params.textDocument.uri, m.params.textDocument.text)
            break
          case 'textDocument/didChange':
            texts.set(m.params.textDocument.uri, m.params.contentChanges[0].text)
            break
          case 'textDocument/didClose':
            texts.delete(m.params.textDocument.uri)
            break
          case 'shutdown':
            reply(m.id, null)
            break
          case 'exit':
            child.die(0, null)
            break
          default:
            if (m.id !== undefined) {
              // The barrier, or anything else unknown: publish what changed, then answer.
              for (const [uri, text] of texts) {
                const diagnostics = judge(uri, text)
                if (diagnostics.length > 0) {
                  published.add(uri)
                  publish(uri, diagnostics)
                } else if (published.has(uri)) {
                  published.delete(uri)
                  publish(uri, [])
                }
              }
              reply(m.id, null)
            }
        }
      }
    })
    child.die = (code, signal, stderr = '') => {
      if (!child.alive) return
      child.alive = false
      if (stderr) child.stderr.write(stderr)
      setImmediate(() => child.emit('exit', code, signal))
    }
    child.kill = () => {
      child.killed = true
      child.die(null, 'SIGTERM')
      return true
    }
    children.push(child)
    return child
  }
  return { spawn, children, last: () => children[children.length - 1] }
}

const session = async () => ({ root: ROOT, binary: { command: '/bin/yidam', reason: 'test' } })
const graph = async () => ({ ok: true, handshake: { ok: true }, report: { corpus_dir: CORPUS_DIR }, resolvedRoot: ROOT })

function bridgeWith(server, extra = {}) {
  return new OverlayBridge({
    session,
    report: graph,
    spawn: server.spawn,
    onProcessExit: () => {},
    ...extra,
  })
}

/** A subscriber that records its events and can wait for the next one matching. */
function page(bridge) {
  const events = []
  const waiters = []
  const send = (event) => {
    events.push(event)
    const i = waiters.findIndex((w) => w.match(event))
    if (i !== -1) waiters.splice(i, 1)[0].resolve(event)
  }
  const sub = bridge.subscribe(send)
  const next = (match) =>
    new Promise((resolve) => {
      waiters.push({ match, resolve })
    })
  return { ...sub, events, next }
}

const tick = () => new Promise((r) => setImmediate(r))
const settle = async (n = 5) => {
  for (let i = 0; i < n; i += 1) await tick()
}

const DANGLING = [
  {
    range: { start: { line: 5, character: 0 }, end: { line: 5, character: 10 } },
    severity: 1,
    code: 'dangling-edge',
    source: 'yidam',
    message: 'target gone.yml does not exist',
  },
]

test('the first event is a status carrying the client id; the handshake follows', async () => {
  const server = scriptedServer()
  const bridge = bridgeWith(server)
  const p = page(bridge)
  assert.equal(p.events[0].event, 'status')
  assert.equal(p.events[0].data.client, p.client)
  assert.ok(['idle', 'starting'].includes(p.events[0].data.state))

  const ready = await p.next((e) => e.event === 'status' && e.data.state === 'ready')
  assert.equal(ready.data.unsavedInstances, true)
  assert.equal(ready.data.corpusDir, CORPUS_DIR)
  assert.equal(ready.data.starts, 1)
  assert.deepEqual(server.last().args, ['serve', '--lsp'])
  assert.equal(server.last().cwd, ROOT)
  const init = server.last().received.find((m) => m.method === 'initialize')
  assert.equal(init.params.rootUri, 'file:///tmp/corpus')
  p.close()
})

test('a server that does not declare unsavedInstances is reported as null, not false', async () => {
  const server = scriptedServer({ capabilities: { textDocumentSync: 1 } })
  const bridge = bridgeWith(server)
  const p = page(bridge)
  const ready = await p.next((e) => e.event === 'status' && e.data.state === 'ready')
  assert.equal(ready.data.unsavedInstances, null)
  p.close()
})

test('a change is refused before anything is spawned when it cannot be addressed', async () => {
  const server = scriptedServer()
  const bridge = bridgeWith(server)
  const stranger = await bridge.change('nobody', 'gage/x.yml', '')
  assert.deepEqual(stranger, {
    ok: false,
    status: 409,
    error: 'subscribe to /api/overlay first; this client id is not open',
  })
  const p = page(bridge)
  const bad = await bridge.change(p.client, 'gage.ont.yml', '')
  assert.equal(bad.status, 400)
  assert.match(bad.error, /class directory/)
  p.close()
})

test('docIdError applies the walker’s predicate, and refuses what a path should not carry', () => {
  assert.equal(docIdError('gage/new.yml'), null)
  assert.equal(docIdError('concept/deep/new.yml'), null)
  assert.match(docIdError('new.yml'), /class directory/)
  assert.match(docIdError('gage/new.yaml'), /\.yml file/)
  assert.match(docIdError('gage/gage.ont.yml'), /class/)
  assert.match(docIdError('gage/../x.yml'), /not a path segment: "\.\."/)
  assert.match(docIdError('/gage/x.yml'), /not a path segment: ""/)
  assert.match(docIdError('gage/.hidden.yml'), /not a path segment/)
  assert.match(docIdError('gage/a b.yml'), /not a path segment/)
})

test('a change opens the buffer at the corpus path, then changes it, and each is judged', async () => {
  const server = scriptedServer({
    judge: (_uri, text) => (text.includes('gone.yml') ? DANGLING : []),
  })
  const bridge = bridgeWith(server)
  const p = page(bridge)
  await p.next((e) => e.event === 'status' && e.data.state === 'ready')

  const first = await bridge.change(p.client, 'gage/new.yml', 'links:\n  - target: gone.yml\n')
  assert.equal(first.ok, true)
  const child = server.last()
  const open = child.received.find((m) => m.method === 'textDocument/didOpen')
  assert.equal(open.params.textDocument.uri, 'file:///tmp/corpus/.yidam/corpus/gage/new.yml')
  assert.equal(open.params.textDocument.languageId, 'yaml')
  const barrier = child.received.find((m) => m.method === '$/yidam/barrier')
  assert.ok(barrier, 'no barrier followed the open')
  assert.ok(child.received.indexOf(barrier) > child.received.indexOf(open))

  const verdict = p.events.find((e) => e.event === 'verdict')
  assert.deepEqual(verdict.data.doc, 'gage/new.yml')
  assert.deepEqual(verdict.data.diagnostics, DANGLING)
  assert.equal(verdict.data.seq, first.seq)

  const second = await bridge.change(p.client, 'gage/new.yml', 'links: []\n')
  assert.equal(second.ok, true)
  const change = child.received.find((m) => m.method === 'textDocument/didChange')
  assert.deepEqual(change.params.contentChanges, [{ text: 'links: []\n' }])
  const clean = p.events.filter((e) => e.event === 'verdict').at(-1)
  assert.deepEqual(clean.data.diagnostics, [])
  assert.ok(clean.data.seq > verdict.data.seq)
  p.close()
})

test('a buffer that was always clean still gets a verdict — the barrier, not the silence', async () => {
  const server = scriptedServer()
  const bridge = bridgeWith(server)
  const p = page(bridge)
  await p.next((e) => e.event === 'status' && e.data.state === 'ready')
  const result = await bridge.change(p.client, 'gage/clean.yml', 'class: gage\n')
  assert.equal(result.ok, true)
  const verdicts = p.events.filter((e) => e.event === 'verdict')
  assert.equal(verdicts.length, 1)
  assert.deepEqual(verdicts[0].data, { doc: 'gage/clean.yml', diagnostics: [], seq: result.seq })
  assert.ok(
    !server.last().received.some((m) => m.method === 'textDocument/publishDiagnostics'),
    'the fake published nothing; the verdict came from the barrier',
  )
  p.close()
})

test('a verdict goes to the pages holding the buffer and to no other', async () => {
  const server = scriptedServer({ judge: () => DANGLING })
  const bridge = bridgeWith(server)
  const a = page(bridge)
  const b = page(bridge)
  await a.next((e) => e.event === 'status' && e.data.state === 'ready')
  await bridge.change(a.client, 'gage/a.yml', 'x')
  await settle()
  assert.equal(a.events.filter((e) => e.event === 'verdict').length, 1)
  assert.equal(b.events.filter((e) => e.event === 'verdict').length, 0)
  a.close()
  b.close()
})

test('a child dying mid-session is said out loud, and the next change starts another', async () => {
  const server = scriptedServer({ judge: () => DANGLING })
  const bridge = bridgeWith(server)
  const p = page(bridge)
  await p.next((e) => e.event === 'status' && e.data.state === 'ready')
  await bridge.change(p.client, 'gage/a.yml', 'x')

  // Dies while a change is in flight: the change is answered 503, the page is told.
  const before = p.events.filter((e) => e.event === 'verdict').at(-1).data.seq
  const first = server.last()
  first.mute = true
  const inFlight = bridge.change(p.client, 'gage/a.yml', 'y')
  await tick()
  first.die(101, null, 'thread panicked at lint/mod.rs\n')
  const result = await inFlight
  assert.equal(result.ok, false)
  assert.equal(result.status, 503)
  assert.match(result.error, /exited 101/)
  assert.match(result.error, /panicked/)
  const exited = p.events.filter((e) => e.event === 'status').at(-1)
  assert.equal(exited.data.state, 'exited')
  assert.match(exited.data.detail, /exited 101/)
  assert.match(exited.data.detail, /panicked/)
  assert.equal(bridge.isRunning, false)

  // The next change starts a fresh child, which is told about every buffer the bridge still
  // holds before the new change is judged.
  const again = await bridge.change(p.client, 'gage/b.yml', 'z')
  assert.equal(again.ok, true)
  assert.equal(server.children.length, 2)
  const second = server.last()
  const opens = second.received.filter((m) => m.method === 'textDocument/didOpen').map((m) => m.params.textDocument)
  assert.deepEqual(
    opens.map((d) => [d.uri.split('/').slice(-2).join('/'), d.text]).sort(),
    [
      ['gage/a.yml', 'y'],
      ['gage/b.yml', 'z'],
    ],
  )
  const ready = p.events.filter((e) => e.event === 'status').at(-1)
  assert.equal(ready.data.state, 'ready')
  assert.equal(ready.data.starts, 2)
  // Both buffers' verdicts are fresh — the replayed one and the new one.
  const docs = p.events.filter((e) => e.event === 'verdict' && e.data.seq > before).map((e) => e.data.doc)
  assert.ok(docs.includes('gage/a.yml') && docs.includes('gage/b.yml'), `verdicts after restart: ${docs}`)
  p.close()
})

test('three starts in a minute is a binary that cannot run here; the bridge stops trying', async () => {
  let now = 1_000_000
  const server = scriptedServer()
  const bridge = bridgeWith(server, { now: () => now })
  const p = page(bridge)
  await p.next((e) => e.event === 'status' && e.data.state === 'ready')

  for (let i = 0; i < 2; i += 1) {
    server.last().die(1, null, `crash ${i}\n`)
    await p.next((e) => e.event === 'status' && e.data.state === 'exited')
    now += 1000
    const r = await bridge.change(p.client, 'gage/a.yml', 'x')
    assert.equal(r.ok, true, `restart ${i + 1} should have worked`)
  }
  assert.equal(server.children.length, 3)
  server.last().die(1, null, 'crash 2\n')
  await p.next((e) => e.event === 'status' && e.data.state === 'exited')
  now += 1000
  const r = await bridge.change(p.client, 'gage/a.yml', 'x')
  assert.equal(r.ok, false)
  assert.equal(r.status, 503)
  assert.match(r.error, /started 3 times in the last minute/)
  assert.match(r.error, /crash 2/)
  assert.equal(server.children.length, 3, 'a fourth child was spawned')
  const status = p.events.filter((e) => e.event === 'status').at(-1)
  assert.equal(status.data.state, 'unavailable')

  // Outside the window the budget is fresh again.
  now += 61_000
  const later = await bridge.change(p.client, 'gage/a.yml', 'x')
  assert.equal(later.ok, false, 'unavailable is sticky within a process; the page reloads to retry')
  p.close()
})

test('the last page leaving stops the child; a shared buffer is closed only by its last holder', async () => {
  const server = scriptedServer()
  const bridge = bridgeWith(server)
  const a = page(bridge)
  const b = page(bridge)
  await a.next((e) => e.event === 'status' && e.data.state === 'ready')
  await bridge.change(a.client, 'gage/shared.yml', 'x')
  await bridge.change(b.client, 'gage/shared.yml', 'x')
  await bridge.change(a.client, 'gage/mine.yml', 'x')
  const child = server.last()

  a.close()
  await settle()
  const closed = child.received.filter((m) => m.method === 'textDocument/didClose').map((m) => m.params.textDocument.uri)
  assert.deepEqual(closed, ['file:///tmp/corpus/.yidam/corpus/gage/mine.yml'])
  assert.equal(bridge.isRunning, true, 'a page is still open')
  assert.equal(bridge.open, 1)

  b.close()
  await settle()
  assert.ok(child.received.some((m) => m.method === 'shutdown'), 'shutdown was not requested')
  assert.ok(child.received.some((m) => m.method === 'exit'), 'exit was not sent')
  assert.equal(child.alive, false)
  assert.equal(child.killed, false, 'a server that answers shutdown is not killed')
  assert.equal(bridge.isRunning, false)
})

test('a renamed draft releases its old name; a page cannot release what it never opened', async () => {
  const server = scriptedServer()
  const bridge = bridgeWith(server)
  const a = page(bridge)
  const b = page(bridge)
  await a.next((e) => e.event === 'status' && e.data.state === 'ready')
  await bridge.change(a.client, 'gage/ne.yml', 'x')
  await bridge.change(a.client, 'gage/shared.yml', 'x')
  await bridge.change(b.client, 'gage/shared.yml', 'x')
  const child = server.last()

  assert.equal(bridge.release(a.client, 'gage/ne.yml').ok, true)
  // Held by b as well: a's release does not close it.
  assert.equal(bridge.release(a.client, 'gage/shared.yml').ok, true)
  await settle()
  const closed = child.received.filter((m) => m.method === 'textDocument/didClose').map((m) => m.params.textDocument.uri)
  assert.deepEqual(closed, ['file:///tmp/corpus/.yidam/corpus/gage/ne.yml'])
  // Releasing twice, or a doc that was never opened, is not an error: the page's view of what
  // it holds may lag the bridge's, and the answer either way is "you hold nothing by that name".
  assert.equal(bridge.release(a.client, 'gage/ne.yml').ok, true)
  assert.equal(bridge.release('not-a-client', 'gage/ne.yml').status, 409)

  a.close()
  await settle()
  assert.equal(bridge.isRunning, true, 'b still holds the stream')
  b.close()
  await settle()
  assert.equal(bridge.isRunning, false)
})

test('a corpus that cannot be addressed is refused before a child is started', async () => {
  // No binary.
  {
    const server = scriptedServer()
    const bridge = new OverlayBridge({
      session: async () => ({ root: ROOT, binary: { command: null, reason: 'nothing on PATH' } }),
      report: graph,
      spawn: server.spawn,
      onProcessExit: () => {},
    })
    const p = page(bridge)
    const status = await p.next((e) => e.event === 'status' && e.data.state === 'unavailable')
    assert.match(status.data.detail, /no yidam binary: nothing on PATH/)
    assert.equal(server.children.length, 0)
    const r = await bridge.change(p.client, 'gage/a.yml', 'x')
    assert.equal(r.status, 503)
    p.close()
  }
  // A graph report with no corpus_dir — a binary older than the field.
  {
    const server = scriptedServer()
    const bridge = new OverlayBridge({
      session,
      report: async () => ({ ok: true, handshake: { ok: true }, report: { nodes: [] }, resolvedRoot: ROOT }),
      spawn: server.spawn,
      onProcessExit: () => {},
    })
    const p = page(bridge)
    const status = await p.next((e) => e.event === 'status' && e.data.state === 'unavailable')
    assert.match(status.data.detail, /did not report corpus_dir/)
    assert.equal(server.children.length, 0)
    p.close()
  }
})

test('the process-exit hook kills the child', async () => {
  const server = scriptedServer()
  let hook = null
  const bridge = bridgeWith(server, { onProcessExit: (h) => (hook = h) })
  const p = page(bridge)
  await p.next((e) => e.event === 'status' && e.data.state === 'ready')
  assert.equal(typeof hook, 'function')
  hook()
  await settle()
  assert.equal(server.last().killed, true)
  p.close()
})

test('sseEvent is one event name and one data line', () => {
  assert.equal(
    sseEvent({ event: 'verdict', data: { doc: 'a/b.yml', diagnostics: [], seq: 3 } }),
    'event: verdict\ndata: {"doc":"a/b.yml","diagnostics":[],"seq":3}\n\n',
  )
  // A message with a newline in it stays on one line: JSON escapes it.
  const text = sseEvent({ event: 'verdict', data: { doc: 'a/b.yml', diagnostics: [{ message: 'x\ny' }], seq: 4 } })
  assert.equal(text.split('\n').length, 4)
})
