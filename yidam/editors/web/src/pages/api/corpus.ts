/**
 * The nodes, the classes, and the edges — already resolved.
 *
 * `yidam graph --format json` reports every edge resolved, with `exists` answered by the same
 * test the gate uses. That matters more here than it did in the extension, and for the reason
 * `graph.ts` gives about itself: a corpus edge is a filesystem-relative path resolved against
 * the instance's own directory, that rule belongs to `dangling_edge` and `orphan_in`, and
 * **the CLI applies it**. Nothing on this side decides whether an edge is broken.
 *
 * Under the shape RFC-0030 originally proposed that was true by construction — there was no
 * process here to decide it in. It is now true by discipline, and `test/boundary.mjs` is what
 * holds it.
 */

import type { APIRoute } from 'astro'
import { reportRoute } from '../../lib/api.ts'

export const prerender = false

export const GET: APIRoute = ({ request }) => reportRoute(request, 'graph')
