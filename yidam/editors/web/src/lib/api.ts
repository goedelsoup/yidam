/**
 * What an API route returns, and the one check every route runs first.
 *
 * Two shapes and no third: a report the binary printed, or a named reason there is none.
 * A route that invents a third — a partial result, a best-effort parse, a default — is the
 * failure `handshake.ts` names: *a consumer that best-effort-parses an envelope it does not
 * understand is a consumer that reports wrong verdicts confidently, which is worse than
 * reporting none.*
 *
 * The origin check lives here rather than in middleware because it is the same rule for
 * every route and belongs beside the response shape it produces. The only legitimate client
 * is the page this server served, so a cross-origin request is refused rather than allowed by
 * a flag — a narrower rule than `serve --mcp --http`'s `--allow-origin`, because the
 * situation is narrower.
 */

import { session } from './session.ts'
import { spawnReport, type ReportCommand } from './cli.ts'
import { describeFailure } from './messages.ts'

const JSON_HEADERS = { 'content-type': 'application/json' }

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: JSON_HEADERS })
}

/**
 * Refuse anything that did not come from the page this server served.
 *
 * A same-origin request either carries this server's own `Origin` or carries none at all —
 * browsers omit it on same-origin navigations and simple GETs. Anything else is another site
 * asking, and there is no configuration that makes that legitimate here.
 *
 * **Compared against `Host`, and deliberately not against Astro's `url.origin`.** With the
 * node adapter `url.origin` is a synthesised `http://localhost` with no port in it, so
 * comparing an `Origin` header to it refuses every same-origin request — which is what the
 * first version of this function did, and what a browser would have hit on the first fetch.
 * `Host` is the value the request actually arrived with.
 */
export function wrongOrigin(request: Request): boolean {
  const origin = request.headers.get('origin')
  if (origin === null) return false

  const host = request.headers.get('host')
  // An `Origin` with no `Host` is not a case to guess at. Refusing is the conservative
  // reading and costs nothing: every real client sends both.
  if (host === null) return true

  try {
    return new URL(origin).host !== host
  } catch {
    return true
  }
}

/** One report route, end to end: origin, binary, spawn, envelope. */
export async function reportRoute(request: Request, command: ReportCommand): Promise<Response> {
  if (wrongOrigin(request)) {
    return json({ error: 'cross-origin request refused' }, 403)
  }

  const { root, binary } = await session()
  if (binary.command === null) {
    return json({ error: 'no yidam binary', reason: binary.reason }, 503)
  }

  const result = await spawnReport({ command: binary.command, root }, command)
  if (!result.ok) {
    return json({ error: describeFailure(result.handshake), kind: result.handshake.kind }, 503)
  }
  return json(result.report)
}
