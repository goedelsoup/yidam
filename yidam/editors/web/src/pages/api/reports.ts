/**
 * `lint` and `graph-check`, as the RFC-0001 envelope, byte-identical to `--format json`.
 *
 * Byte-identical is a constraint rather than a description. The moment this route reshapes an
 * envelope it becomes a second contract, and a page reading the second one is a page whose
 * verdicts can differ from `yidam lint`'s without anybody editing a check. So it forwards.
 *
 * Two reports rather than one because the ordering rule the extension settled applies here
 * too: lint owns the diagnostics, graph-check fills the gap. A page that showed only one
 * would show a clean corpus for a class of finding that is not clean.
 */

import type { APIRoute } from 'astro'
import { json, wrongOrigin } from '../../lib/api.ts'
import { session } from '../../lib/session.ts'
import { spawnReport } from '../../lib/cli.ts'
import { describeFailure } from '../../lib/messages.ts'

export const prerender = false

export const GET: APIRoute = async ({ request }) => {
  if (wrongOrigin(request)) {
    return json({ error: 'cross-origin request refused' }, 403)
  }

  const { root, binary } = await session()
  if (binary.command === null) {
    return json({ error: 'no yidam binary', reason: binary.reason }, 503)
  }

  const input = { command: binary.command, root }
  // Sequential, not concurrent. Two spawns of the same binary over the same working tree is
  // the cheap case, and a reader waiting on the slower of two is waiting on the same wall
  // clock either way; what concurrency would buy here is not worth two processes competing
  // for the same file cache on a laptop.
  const lint = await spawnReport(input, 'lint')
  const graphCheck = await spawnReport(input, 'graph-check')

  if (!lint.ok) {
    return json({ error: describeFailure(lint.handshake), kind: lint.handshake.kind }, 503)
  }

  return json({
    lint: lint.report,
    // Null rather than an error: one failed report must not take the other with it, and a
    // view whose report is missing renders as unavailable. That is `report-run.ts`'s
    // discipline in the extension and it is right for the same reason here.
    graph_check: graphCheck.ok ? graphCheck.report : null,
  })
}
