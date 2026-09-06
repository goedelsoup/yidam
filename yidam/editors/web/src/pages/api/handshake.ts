/**
 * Which binary answered, and whether this build understands it.
 *
 * The first thing the page asks and the only route that is useful when everything else has
 * failed: a reader whose verdicts are unavailable needs to know *which* yidam was found and
 * *why* it was refused, not an empty list of findings.
 *
 * `status` carries the same envelope every other report does, so this reads the contract
 * without asking for a report anybody wanted.
 */

import type { APIRoute } from 'astro'
import { json, wrongOrigin } from '../../lib/api.ts'
import { session } from '../../lib/session.ts'
import { spawnReport } from '../../lib/cli.ts'
import { describeBinary, describeFailure } from '../../lib/messages.ts'

export const prerender = false

export const GET: APIRoute = async ({ request }) => {
  if (wrongOrigin(request)) {
    return json({ error: 'cross-origin request refused' }, 403)
  }

  const { root, binary } = await session()
  if (binary.command === null) {
    return json({ ok: false, root, origin: binary.origin, reason: binary.reason }, 503)
  }

  const result = await spawnReport({ command: binary.command, root }, 'status')
  if (!result.ok) {
    return json(
      {
        ok: false,
        root,
        origin: binary.origin,
        reason: describeFailure(result.handshake),
        kind: result.handshake.kind,
      },
      503,
    )
  }

  return json({
    ok: true,
    root,
    origin: binary.origin,
    describe: describeBinary(result.handshake, binary.reason),
    format_version: result.handshake.ok ? result.handshake.format : null,
    yidam: result.handshake.ok ? result.handshake.yidam : null,
  })
}
