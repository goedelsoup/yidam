#!/usr/bin/env node
/**
 * `npx @yidam/edit` — the entry point this whole surface exists to provide.
 *
 * #420's sentence is *every surface arrives in a terminal*, and this is the one editor
 * surface that did not. `serve --lsp` needs an LSP-capable editor and a hand-written config
 * block; the extension needs VS Code and, until #314, a sideloaded `.vsix`. A person who has
 * a corpus and a terminal and no configured editor was served by nothing. `npx` is a
 * terminal, and it is the one that person already has.
 *
 * This file does three things and decides nothing: parse the flags, hand the resolved root to
 * the server through the environment, and start it. The corpus root it passes is a
 * *candidate* — `src/lib/cli.ts`'s first spawn is what validates it, because the rule for
 * what counts as a corpus root lives in the binary.
 */

import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from './args.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const ENTRY = path.join(here, '..', 'dist', 'server', 'entry.mjs')

const parsed = parseArgs(process.argv.slice(2), process.cwd())
if (!parsed.ok) {
  const stream = parsed.usage ? process.stdout : process.stderr
  stream.write(`${parsed.message}\n`)
  process.exit(parsed.usage ? 0 : 2)
}
const { root, port, open } = parsed.args

if (!existsSync(ENTRY)) {
  process.stderr.write(
    `no server build at ${ENTRY}\n` +
      'This package publishes a built server. From a checkout, run `npm run build` first.\n',
  )
  process.exit(1)
}

process.env.YIDAM_EDIT_ROOT = path.resolve(root)
process.env.HOST = '127.0.0.1'
process.env.PORT = String(port)

const url = `http://127.0.0.1:${port}/`
process.stdout.write(`yidam-edit — ${process.env.YIDAM_EDIT_ROOT}\n${url}\n`)

await import(ENTRY)

if (open) {
  // Best effort, and silent on failure: a browser that did not launch is a mild
  // inconvenience beside a URL already printed above, and an error here would read as the
  // server having failed to start when it did not.
  const opener =
    process.platform === 'darwin' ? 'open' : process.platform === 'win32' ? 'start' : 'xdg-open'
  try {
    spawn(opener, [url], { stdio: 'ignore', detached: true }).unref()
  } catch {
    /* the URL is on stdout */
  }
}
