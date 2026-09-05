/**
 * The open questions, as the binary counts them.
 *
 * A corpus's open questions are one of the few things a person opens this surface *to see*
 * rather than to edit, which is why the read phase carries them. `yidam open-questions` is
 * the same command `mise run open-questions` runs, so the page and the terminal agree by
 * construction rather than by care.
 */

import type { APIRoute } from 'astro'
import { reportRoute } from '../../lib/api.ts'

export const prerender = false

export const GET: APIRoute = ({ request }) => reportRoute(request, 'open-questions')
