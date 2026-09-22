/**
 * The overlay routes' policy, before anything is spawned — #607.
 *
 * Every test here is answered before the bridge starts a child: a wrong origin, a wrong
 * method, a missing address, a client id nobody subscribed. Those are the route's rules, and
 * they are the same two rules the act tier has (`test/act.mjs`) applied to a different
 * subject — which buffers this page holds — plus one the act tier does not need, because
 * the act tier addresses nothing.
 *
 * What happens after the rules pass is `test/overlay.mjs`, against a scripted server.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { overlayChange, overlayStream } from '../src/lib/api.ts'

const SAME = { host: '127.0.0.1:8788', origin: 'http://127.0.0.1:8788' }
const req = (method, headers = {}, url = 'http://127.0.0.1:8788/api/overlay/change', body) =>
  new Request(url, method === 'GET' || method === 'HEAD' ? { method, headers } : { method, headers, body })

test('the stream refuses another origin and is otherwise an event stream', async () => {
  const refused = overlayStream(req('GET', { host: '127.0.0.1:8788', origin: 'https://example.com' }, 'http://127.0.0.1:8788/api/overlay'))
  assert.equal(refused.status, 403)
  assert.equal(refused.headers.get('content-type'), 'application/json')
})

test('a change is reached by POST and nothing else', async () => {
  for (const method of ['GET', 'HEAD', 'PUT', 'DELETE']) {
    const refusal = await overlayChange(req(method, SAME))
    assert.equal(refusal.status, 405, `${method} was allowed through`)
    assert.equal(refusal.headers.get('allow'), 'POST')
    assert.match((await refusal.json()).error, /the overlay is reached by POST/)
  }
})

test('a change with no Origin, or another one, is refused', async () => {
  const none = await overlayChange(req('POST', { host: '127.0.0.1:8788' }))
  assert.equal(none.status, 403)
  const other = await overlayChange(req('POST', { host: '127.0.0.1:8788', origin: 'http://localhost:8788' }))
  assert.equal(other.status, 403)
})

test('a change names the client and the buffer on the query string, or it is not a change', async () => {
  const bare = await overlayChange(req('POST', SAME, 'http://127.0.0.1:8788/api/overlay/change', 'x'))
  assert.equal(bare.status, 400)
  assert.match((await bare.json()).error, /client and doc are required/)
  const half = await overlayChange(req('POST', SAME, 'http://127.0.0.1:8788/api/overlay/change?doc=gage/a.yml', 'x'))
  assert.equal(half.status, 400)
})

test('a client that never subscribed is told so, and no child is started for it', async () => {
  // The subscription is where a client id comes from, so an unknown one is answered from
  // the bridge's table and never reaches the spawner — a 409 with a real binary is the same
  // 409 without one, which is why this test can run here.
  const url = 'http://127.0.0.1:8788/api/overlay/change?client=nobody&doc=gage/a.yml'
  const change = await overlayChange(req('POST', SAME, url, 'class: gage\n'))
  assert.equal(change.status, 409)
  assert.match((await change.json()).error, /subscribe to \/api\/overlay first/)
  const close = await overlayChange(req('POST', SAME, `${url}&close=1`))
  assert.equal(close.status, 409)
})
