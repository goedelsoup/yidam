/**
 * The command line, as data.
 *
 * ```
 * npx @yidam/edit [--root DIR] [--port N] [--no-open]
 * ```
 *
 * Three flags, and the two that are missing are the point. There is no `--bind`: this server
 * authenticates nobody, and RFC-0030 declines the flag that would turn a loopback editor into
 * the deployed reader #236 closed. There is no `--allow-origin`: the only legitimate client is
 * the page this server served, so any other origin is refused rather than configured.
 *
 * **`--root` is parsed here and validated by the binary, not by this file.** The rule for what
 * counts as a corpus root is #549's and it lives in Rust; restating it in TypeScript would be a
 * second copy of a rule the parity apparatus exists to prevent. So this returns a candidate and
 * the first spawn decides — slower than a local `statSync`, and correct. RFC-0030 records the
 * cost as an open question and asks Phase 1 to measure it.
 *
 * Plain `.mjs` under `bin/` rather than TypeScript under `src/`, because `bin/yidam-edit.mjs`
 * is the one file that runs *before* anything is bundled and must therefore exist in the
 * published tarball in the form Node executes. A TypeScript copy for the server to import
 * would be a second parser to keep in step, which is the failure this file avoids by being
 * the only one.
 */

/**
 * The port. Not `--http`'s 8787, deliberately.
 *
 * RFC-0030 leaves coexistence open — whether an MCP server and this may run against one
 * corpus at once — and a shared default would answer it by collision, which is the worst way
 * to answer anything. A neighbouring number keeps the question open and cheap.
 */
export const DEFAULT_PORT = 8788

export const USAGE = `usage: yidam-edit [--root DIR] [--port N] [--no-open]

  --root DIR   The corpus to open. Defaults to the working directory.
  --port N     Loopback port. Defaults to ${DEFAULT_PORT}.
  --no-open    Do not launch a browser.

Binds 127.0.0.1 only, and that is not configurable.`

/**
 * @param {string[]} argv
 * @param {string} cwd
 * @returns {{ok: true, args: {root: string, port: number, open: boolean}} | {ok: false, message: string, usage: boolean}}
 */
export function parseArgs(argv, cwd) {
  const args = { root: cwd, port: DEFAULT_PORT, open: true }

  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i]
    if (flag === '--root' || flag === '--port') {
      const value = argv[i + 1]
      if (value === undefined || value.startsWith('--')) {
        return { ok: false, usage: false, message: `${flag} needs a value.` }
      }
      if (flag === '--root') {
        args.root = value
      } else {
        const port = Number(value)
        if (!Number.isInteger(port) || port < 1 || port > 65535) {
          return { ok: false, usage: false, message: `--port ${value} is not a port number.` }
        }
        args.port = port
      }
      i += 1
    } else if (flag === '--no-open') {
      args.open = false
    } else if (flag === '--help' || flag === '-h') {
      return { ok: false, usage: true, message: USAGE }
    } else {
      // Unknown flags are refused rather than ignored, for `binary.ts`'s reason about a wrong
      // `yidam.path`: silently doing something other than what somebody asked for is how a
      // tool starts lying about what it is doing.
      return { ok: false, usage: false, message: `unknown argument ${flag}\n\n${USAGE}` }
    }
  }

  return { ok: true, args }
}
