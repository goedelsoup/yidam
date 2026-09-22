/**
 * `POST /api/act/propose[?dry_run=true]` — the one write in the contract, behind a second
 * framing.
 *
 * RFC-0030's route table named this row and deferred it to RFC-0029's build; the build landed
 * in #864 and the row is this file. It is the `propose` tool of the `act` tier, reached
 * through `yidam serve --mcp` — see `src/lib/act.ts` for why it is not `yidam propose`.
 *
 * `ALL` rather than `POST`, so a `GET` is told `405` and what the route wants instead of a
 * `404` that reads as the route not existing. The method rule is `actRefusal`'s and is tested
 * without a socket.
 */

import type { APIRoute } from 'astro'
import { actRoute } from '../../../lib/api.ts'

export const prerender = false

export const ALL: APIRoute = ({ request }) => actRoute(request, 'propose')
