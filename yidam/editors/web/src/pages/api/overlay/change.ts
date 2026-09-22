/**
 * `POST /api/overlay/change?client=&doc=` — one buffer's text, to the overlay. Phase 2, #607.
 *
 * `ALL` rather than `POST` so a `GET` is told `405` and what the route wants, the way the
 * act routes do. The method and origin rule is `postRefusal`'s and is tested without a socket.
 */

import type { APIRoute } from 'astro'
import { overlayChange } from '../../../lib/api.ts'

export const prerender = false

export const ALL: APIRoute = ({ request }) => overlayChange(request)
