/**
 * `POST /api/act/cycle` — where this corpus is in its loop, and what the next act is.
 *
 * It writes nothing, and it is under `/api/act/` anyway, because that is where the contract
 * put it: RFC-0029 §2.3 places `cycle` in the `act` tier so that *an agent never reads "here
 * is your next act" from a surface where it cannot act.* On a corpus that has not declared
 * `act` this answers with the same refusal `propose` does, and the page renders that as the
 * answer — it does not fall back to a read that would tell a person what to do next on a
 * surface that cannot do it.
 *
 * `POST` for the same reason, even though the call reads: the tier is reached one way.
 */

import type { APIRoute } from 'astro'
import { actRoute } from '../../../lib/api.ts'

export const prerender = false

export const ALL: APIRoute = ({ request }) => actRoute(request, 'cycle')
