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
import { spawnAct, type ActTool } from './act.ts'
import { overlay, sseEvent } from './overlay.ts'
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

/**
 * Whether a request may reach the act tier at all, before anything is spawned.
 *
 * Pure, and exported for the same reason `wrongOrigin` is: this is the policy half of a
 * route that writes, and a policy with no test is a coin flip. Two rules and no third:
 *
 * - **`POST` and nothing else.** A `GET` is what a link, a prefetch and an image tag send,
 *   and none of them should be able to draft a branch. `405` rather than `404` so a person
 *   reading the response learns the route exists and what it wants.
 * - **The origin rule every route has**, and it is doing more work here. On a read it stops
 *   another site from *seeing* a corpus; on a write it stops another site from *acting* on
 *   one. A browser sends `Origin` on every `POST` it makes, cross-site or not, so the
 *   no-`Origin` allowance `wrongOrigin` keeps for simple GETs does not weaken this: a `POST`
 *   with no `Origin` did not come from a browser page.
 *
 * What it does not do is the thing RFC-0029 §2.2's fourth clause says no server can: know
 * that the process on the other end of loopback is the person who started this one. Every
 * socket transport has an unbounded peer set. `act` is configuration precisely because that
 * fact is not observable, and this route adds no authenticator and claims none.
 */
export function actRefusal(request: Request): Response | null {
  return postRefusal(request, 'the act tier')
}

/**
 * The same two rules for anything that changes state on the server, named for what it is.
 *
 * The overlay's `change` route is not a write to the corpus — nothing on this surface is,
 * short of the act tier — but it is a `POST` that puts text into a process, and a `GET` or
 * a cross-site page has no more business doing that than drafting a branch.
 */
export function postRefusal(request: Request, what: string): Response | null {
  if (request.method !== 'POST') {
    return new Response(JSON.stringify({ error: `${what} is reached by POST` }), {
      status: 405,
      headers: { ...JSON_HEADERS, allow: 'POST' },
    })
  }
  if (wrongOrigin(request) || request.headers.get('origin') === null) {
    return json({ error: 'cross-origin request refused' }, 403)
  }
  return null
}

/**
 * `dry_run`, off the query string, and off nothing else.
 *
 * A query parameter rather than a JSON body, and not for brevity: the body would need
 * parsing, and `test/boundary.mjs` holds that this process parses JSON in exactly the places
 * that read the binary's own output. A request body is not one of them, and the one argument
 * the whole tier takes does not need one. Absent means `false`, which is the contract's own
 * default — this frame does not reinterpret the tool it frames, so a caller that wants the
 * draft without the branch says so, exactly as an MCP client would.
 */
export function dryRunOf(request: Request): boolean {
  const value = new URL(request.url).searchParams.get('dry_run')
  return value === 'true' || value === '1'
}

/**
 * One act route, end to end: method and origin, binary, one MCP connection, the answer.
 *
 * The statuses are the frame's, not the verdict's. `409` carries a refusal the *binary*
 * made — `capability-not-supported`, `propose-refused` — with its text intact, because the
 * leading token is frozen by the contract and a client is entitled to read it. `503` is a
 * server that never answered. Nothing here inspects the refusal to decide which it was.
 */
export async function actRoute(request: Request, tool: ActTool): Promise<Response> {
  const refusal = actRefusal(request)
  if (refusal !== null) return refusal

  const { root, binary } = await session()
  if (binary.command === null) {
    return json({ ok: false, tool, error: 'no yidam binary', reason: binary.reason }, 503)
  }

  const result = await spawnAct({ command: binary.command, root }, tool, dryRunOf(request))
  if (result.ok) {
    return json({ ok: true, tool, act: result.act, contract: result.contract, result: result.result })
  }
  if (result.kind === 'refused') {
    return json(
      { ok: false, tool, act: result.act, contract: result.contract, error: result.error },
      409,
    )
  }
  return json({ ok: false, tool, kind: result.kind, error: result.error }, 503)
}

/**
 * `GET /api/overlay` — the verdict stream, as server-sent events.
 *
 * One subscription per open page, for as long as the connection lasts. The bridge sends a
 * `status` first (carrying the client id the page will put on its changes) and a `verdict`
 * per judged buffer after that; when the browser goes, Astro aborts the request's signal and
 * the subscription closes with it, which is what lets the bridge stop a child nobody is
 * listening to. Origin-checked like every read route: another site may not watch a corpus.
 */
export function overlayStream(request: Request): Response {
  if (wrongOrigin(request)) {
    return json({ error: 'cross-origin request refused' }, 403)
  }
  const bridge = overlay(session)
  const encoder = new TextEncoder()
  let close: (() => void) | null = null
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      const sub = bridge.subscribe((event) => {
        try {
          controller.enqueue(encoder.encode(sseEvent(event)))
        } catch {
          // The stream is already closed; the abort below is what removes the subscriber.
        }
      })
      close = () => {
        sub.close()
        try {
          controller.close()
        } catch {
          // Closed twice is closed.
        }
      }
      request.signal.addEventListener('abort', close, { once: true })
    },
    cancel() {
      close?.()
    },
  })
  return new Response(stream, {
    headers: {
      'content-type': 'text/event-stream',
      'cache-control': 'no-cache',
      connection: 'keep-alive',
    },
  })
}

/**
 * `POST /api/overlay/change?client=&doc=` — one buffer's new text, judged.
 *
 * The body is the buffer, `text/plain`, read with `request.text()` and parsed by nobody on
 * this side: `test/boundary.mjs` holds that this process parses JSON only where it reads the
 * binary's own output, and a request body is the one thing that must never become such a
 * place. The addressing rides the query string, as `dry_run` does on the act route. The
 * answer is the bridge's — `400` for an id that is not a node, `409` for a client that did
 * not subscribe, `503` when no server is there to judge — and the verdict itself arrives on
 * the stream, not here.
 */
export async function overlayChange(request: Request): Promise<Response> {
  const refusal = postRefusal(request, 'the overlay')
  if (refusal !== null) return refusal
  const params = new URL(request.url).searchParams
  const client = params.get('client')
  const doc = params.get('doc')
  if (client === null || doc === null) {
    return json({ ok: false, error: 'client and doc are required on the query string' }, 400)
  }
  const bridge = overlay(session)
  // `close=1` is the page letting a buffer go — a renamed draft, a cleared form. The same
  // route and the same method rule, because it is the same subject: which buffers this
  // client holds.
  const result = params.has('close')
    ? bridge.release(client, doc)
    : await bridge.change(client, doc, await request.text())
  if (result.ok) return json({ ok: true, seq: result.seq })
  return json({ ok: false, error: result.error }, result.status)
}
