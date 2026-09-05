/**
 * The only legitimate client is the page this server served.
 *
 * There is no `--allow-origin` here. `serve --mcp --http` needs one because its client is
 * another site; this server's client is the page it just returned, so any other origin is
 * refused rather than configured — a narrower rule than `--http`'s, because the situation is
 * narrower.
 *
 * **This file exists because the first version was wrong in the worst direction.** It compared
 * the `Origin` header against Astro's `url.origin`, which under the node adapter is a
 * synthesised `http://localhost` with no port in it. Every same-origin request was refused
 * with a 403, and nothing caught it: the pages are server-rendered, so a browser never had to
 * make the fetch that would have failed. It was found by curling a running server, and the
 * lesson is that a security rule with no test is a coin flip whose result nobody has looked
 * at.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { wrongOrigin } from '../src/lib/api.ts'

const req = (headers) => new Request('http://127.0.0.1:8788/api/reports', { headers })

test('no Origin header is allowed', () => {
  // Browsers omit `Origin` on same-origin navigations and simple GETs, which is most of what
  // this surface serves. Refusing here would refuse the ordinary case.
  assert.equal(wrongOrigin(req({ host: '127.0.0.1:8788' })), false)
})

test('the server’s own origin is allowed', () => {
  assert.equal(
    wrongOrigin(req({ host: '127.0.0.1:8788', origin: 'http://127.0.0.1:8788' })),
    false,
  )
})

test('localhost and 127.0.0.1 are different origins, and that is correct', () => {
  // Not a nicety: they are distinct origins to a browser too, so a page served from one does
  // not get to speak for the other. Whichever the person typed is the one that works.
  assert.equal(
    wrongOrigin(req({ host: '127.0.0.1:8788', origin: 'http://localhost:8788' })),
    true,
  )
  assert.equal(
    wrongOrigin(req({ host: 'localhost:8788', origin: 'http://localhost:8788' })),
    false,
  )
})

test('another site is refused', () => {
  for (const origin of [
    'https://evil.example',
    'http://127.0.0.1:9999',
    'http://127.0.0.1',
    'null',
  ]) {
    assert.equal(
      wrongOrigin(req({ host: '127.0.0.1:8788', origin })),
      true,
      `${origin} was allowed`,
    )
  }
})

test('an Origin with no Host is refused rather than guessed at', () => {
  assert.equal(wrongOrigin(req({ origin: 'http://127.0.0.1:8788' })), true)
})

test('an unparseable Origin is refused', () => {
  assert.equal(wrongOrigin(req({ host: '127.0.0.1:8788', origin: 'not a url' })), true)
})
