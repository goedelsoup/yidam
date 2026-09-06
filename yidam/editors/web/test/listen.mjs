/**
 * A busy port is a refusal, not a URL.
 *
 * The behaviour these hold was found by running the thing rather than by reading it: with an
 * `astro dev` already on the port, `yidam-edit --port N` printed the URL and exited 0. Nothing
 * in this package could have caught it — every other test here is a pure function, and this is
 * the one property that needs a real socket to be true or false.
 */

import { strict as assert } from 'node:assert'
import net from 'node:net'
import { test } from 'node:test'
import { claimPort, describeBindFailure } from '../bin/listen.mjs'

/** A port the operating system picked and then let go of, so it is free without being guessed. */
function freePort() {
  return new Promise((resolve, reject) => {
    const probe = net.createServer()
    probe.once('error', reject)
    probe.listen(0, '127.0.0.1', () => {
      const { port } = probe.address()
      probe.close(() => resolve(port))
    })
  })
}

/** Something on the port, closed by the caller. */
function occupy(port) {
  return new Promise((resolve, reject) => {
    const server = net.createServer()
    server.once('error', reject)
    server.listen(port, '127.0.0.1', () => resolve(server))
  })
}

test('a free port is claimed and given back', async () => {
  const port = await freePort()
  assert.equal(await claimPort(port, '127.0.0.1'), null)
  // Given back rather than held: the adapter binds it a moment later, so a probe that kept
  // the socket would turn this check into the failure it exists to report.
  assert.equal(await claimPort(port, '127.0.0.1'), null)
})

test('a busy port is refused, and says which one', async () => {
  const port = await freePort()
  const squatter = await occupy(port)
  try {
    const message = await claimPort(port, '127.0.0.1')
    assert.ok(message, `port ${port} was busy and claimPort returned null`)
    assert.match(message, new RegExp(`\\b${port}\\b`))
    assert.match(message, /already in use/)
    // The reason is in the message, because "port busy" alone reads as a nuisance and the
    // actual cost is reading another server's corpus believing it is yours.
    assert.match(message, /--port/)
  } finally {
    squatter.close()
  }
})

test('the messages name the two failures a person can act on', () => {
  assert.match(describeBindFailure(8788, 'EADDRINUSE'), /already in use/)
  assert.match(describeBindFailure(80, 'EACCES'), /privileges/)
  // Anything else still names the port and the code rather than being swallowed.
  const other = describeBindFailure(8788, 'EAFNOSUPPORT')
  assert.match(other, /8788/)
  assert.match(other, /EAFNOSUPPORT/)
})
