// @ts-check
import { defineConfig } from 'astro/config'
import node from '@astrojs/node'
import react from '@astrojs/react'

/**
 * An Astro application on `@astrojs/node`, served over loopback from a person's checkout.
 *
 * `output: 'server'` is the whole point of the reversal recorded in RFC-0030: there is a
 * process, so the page can carry a verdict rather than a snapshot of one. What that process
 * may NOT do is compute the verdict — see `src/lib/cli.ts` and the boundary gates in
 * `test/boundary.mjs`.
 *
 * `mode: 'standalone'` rather than `'middleware'`: this ships as an npm package a person
 * runs, not as a handler someone mounts. There is no host application to be middleware for.
 */
export default defineConfig({
  output: 'server',
  adapter: node({ mode: 'standalone' }),
  integrations: [react()],
  // Loopback only, and deliberately not configurable. A server that authenticates nobody
  // and now has a Node process in it should not carry the flag that turns it into #236.
  // A container reaches this by publishing a port, which is the container's decision.
  server: { host: '127.0.0.1' },
  // The loopback names, so that Astro's own cross-site check is *correct* rather than off.
  //
  // Astro 5 builds `Astro.url` from the `Host` header only when the host is listed here, and
  // otherwise from the literal `localhost` — with no port. Its CSRF middleware compares a
  // POST's `Origin` to that `url.origin`, so with the list empty it refused every POST a
  // page on `127.0.0.1:8788` could make, and passed only a client that spelled
  // `Origin: http://localhost`. `src/lib/api.ts` compares against `Host` for the same reason
  // (see `wrongOrigin`), and #608's write route was where the two rules first had to agree.
  //
  // Not `checkOrigin: false`. Astro's check and `actRefusal` refuse the same request for the
  // same reason, and a second layer that agrees costs nothing; a switched-off one is what a
  // person finds when they go looking for why a write from another site went through.
  // Hostnames only, no port: `--port` is a flag, and a pattern with no port matches any.
  security: {
    allowedDomains: [{ hostname: '127.0.0.1' }, { hostname: 'localhost' }, { hostname: '[::1]' }],
  },
})
