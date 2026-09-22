/**
 * `GET /api/overlay` — verdicts on unsaved buffers, as server-sent events. Phase 2, #607.
 *
 * RFC-0030's route table named this row as *buffer text in, diagnostics out*. The buffer
 * comes in on `change.ts` beside this; what comes out here is every `publishDiagnostics` the
 * `yidam serve --lsp` child made for the buffers this page holds, relayed by `overlayStream`
 * and computed by nothing on this side of the pipe.
 */

import type { APIRoute } from 'astro'
import { overlayStream } from '../../../lib/api.ts'

export const prerender = false

export const GET: APIRoute = ({ request }) => overlayStream(request)
