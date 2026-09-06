/**
 * Proving the port is free before promising a URL on it.
 *
 * Measured while driving this surface against `mise run ext-fixture`: with something already
 * listening on the port, `yidam-edit --port N` printed the URL, logged two adapter-level
 * unhandled rejections, and **exited 0**. A person who reads the first line and opens it
 * lands on whatever else is on that port. That is not hypothetical here — this repository
 * runs an `astro dev` on 4321 and RFC-0030 deliberately put this server on a *neighbouring*
 * number to keep the coexistence question open, so two servers a digit apart is the designed
 * state rather than an accident.
 *
 * What it costs is the failure this whole surface exists to avoid, arriving one layer below
 * where the surface looks for it: a reader believing they are looking at their corpus when
 * they are looking at another one. `describeRootMismatch` catches that when the envelope
 * disagrees with the flag — but it is the *wrong server's* envelope by then, so it agrees
 * with the wrong server's root and says nothing.
 *
 * The adapter offers no hook. `standalone()` calls `server.listen()` during module evaluation
 * and rejects a promise this file is never handed, so the check has to happen before the
 * entry is imported rather than around it.
 *
 * **This narrows the window rather than closing it.** A process taking the port between this
 * releasing it and the adapter claiming it gets the old behaviour. Closing it properly needs
 * the adapter to accept an already-listening handle, which it does not — so the honest thing
 * is to say so here rather than to imply otherwise by not mentioning it.
 */

import net from 'node:net'

/**
 * Why the bind failed, in the terms the person can act on.
 *
 * Separated from the bind so it is testable without arranging the errno — `EACCES` on a
 * privileged port needs a port under 1024, and asserting on the message should not need
 * root to run.
 *
 * @param {number} port
 * @param {string} code
 * @returns {string}
 */
export function describeBindFailure(port, code) {
  if (code === 'EADDRINUSE') {
    return (
      `port ${port} is already in use, so this server did not start.\n` +
      'Pass --port with a free number, or stop what is on that one.\n' +
      'Refusing rather than continuing: the URL would have opened the other server, and a ' +
      'corpus that is not yours reads exactly like one that is.'
    )
  }
  if (code === 'EACCES') {
    return (
      `port ${port} is not one this user may bind.\n` +
      'Ports below 1024 need privileges. Pass --port with a higher number.'
    )
  }
  return `could not bind 127.0.0.1:${port} (${code}).`
}

/**
 * Take the port, let it go, and report what happened.
 *
 * Resolves `null` when the port was free. Resolves a message — never throws — because the
 * caller's job is to print it and exit, and an exception here would be reported as a crash in
 * the launcher rather than as the plain fact that a port was busy.
 *
 * @param {number} port
 * @param {string} host
 * @returns {Promise<string | null>}
 */
export function claimPort(port, host) {
  return new Promise((resolve) => {
    const probe = net.createServer()
    probe.unref()
    probe.once('error', (/** @type {NodeJS.ErrnoException} */ err) => {
      resolve(describeBindFailure(port, err.code ?? 'unknown'))
    })
    probe.once('listening', () => {
      probe.close(() => resolve(null))
    })
    // Same host the adapter is about to be given. Binding 127.0.0.1 while it binds 0.0.0.0
    // would answer a different question from the one asked, and this server binds loopback
    // and only loopback — see `astro.config.mjs`.
    probe.listen(port, host)
  })
}
