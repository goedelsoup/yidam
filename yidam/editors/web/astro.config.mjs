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
})
