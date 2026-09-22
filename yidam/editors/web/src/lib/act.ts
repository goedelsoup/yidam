/**
 * The write tier, reached through the MCP dispatch and nowhere else — RFC-0030 Phase 3, #608.
 *
 * RFC-0029 §2.2 closes with the clause this file exists to honour: *not a second route into
 * the tier: the loopback surface serves the same operations through the same `super::handle`
 * seam.* The obvious shape — spawn `yidam propose --format json` the way `cli.ts` spawns
 * `lint` — would have been that second route. `yidam propose` is a command a person runs in
 * their own shell, so it carries none of the tier: not the `[serve] act = true` declaration,
 * not the git-identity refusal at startup, not the frozen `capability-not-supported` shape a
 * corpus that did not opt in answers with. All of that lives in `ServerState::load` and
 * `tools::call`, behind `handle`. A route that skipped it would let a browser write into a
 * corpus that never said it could be written to by anything but a person.
 *
 * So this spawns `yidam serve --mcp` — stdio, in the default feature set — and speaks the
 * protocol to it for exactly one call: `initialize`, `notifications/initialized`, one
 * `tools/call`, then stdin closes and the server exits. One connection per request, the same
 * per-request shape every read route already has, and `tests/mcp_act_tier.rs` on the CLI side
 * pins that a one-shot connection answers and exits cleanly.
 *
 * # What this file decides
 *
 * Nothing about the corpus. Whether `act` is declared, whether the checkout has an author,
 * whether the working tree is clean enough to draft from, what a finding licenses — every one
 * of those is the binary's answer, relayed. What this file decides is *framing*: which three
 * lines go down the pipe and which two come back. The one policy it holds is negative — the
 * only argument it will ever send is `dry_run`, so `force` cannot arrive from a browser by any
 * spelling. The contract keeps `--force` for a person who can see what they are replacing;
 * this surface is not that person.
 *
 * # Why the answer carries three states and not a boolean
 *
 * A binary that predates the tier (contract < 0.22.0) answers `initialize` perfectly well with
 * no `act` key at all, and a corpus that has the key and did not opt in answers `false`. Those
 * are different repairs — re-pin the binary, or write `[serve] act = true` — and collapsing
 * them into "no" would send a person to edit a config file that the binary they have cannot
 * read. Degrade loudly, and say which.
 */

import { spawn } from 'node:child_process'

/** The two tools in the tier, and the only names this file will put on the wire. */
export type ActTool = 'propose' | 'cycle'

/** Whether the server declared `act`, as its handshake said it — never inferred here. */
export type Declared = 'declared' | 'undeclared' | 'predates-act'

export type ActResult<T> =
  | {
      ok: true
      tool: ActTool
      act: 'declared'
      contract: string
      result: T
    }
  | {
      /** The server answered and the tool refused — an MCP tool error, `isError: true`. */
      ok: false
      kind: 'refused'
      tool: ActTool
      act: Declared
      contract: string | null
      /**
       * The refusal, verbatim. Its leading token is frozen by the contract —
       * `capability-not-supported`, `propose-refused` — and a client acts on the token, so
       * it is carried whole rather than reworded.
       */
      error: string
    }
  | {
      /**
       * No server answered. The process refused to start (a declared `act` with no git
       * identity, a directory that is not a corpus), the binary is not there, or what came
       * back on stdout was not the protocol. `error` is whatever stderr said, because that
       * is where `serve` explains itself.
       */
      ok: false
      kind: 'no-server'
      tool: ActTool
      error: string
    }

export interface ActInput {
  /** Absolute path to the binary `resolveBinary` found. */
  command: string
  /** The corpus root, already resolved. */
  root: string
  /** Injected so tests need no binary. */
  exec?: Exec
}

export type Exec = (
  command: string,
  args: string[],
  options: { cwd: string; input: string },
) => Promise<{ stdout: string; stderr: string; code: number | null }>

const INITIALIZE_ID = 1
const CALL_ID = 2

/**
 * The three lines this surface sends, and the only three.
 *
 * Exported so the test can hold the frame to its shape without a process: the argument object
 * for `propose` is `{ dry_run }` and nothing else, and `cycle` takes `{}` — RFC-0029's *no
 * `force`* is enforced here by there being no field to carry it in.
 */
export function frame(tool: ActTool, dryRun: boolean): string {
  const args = tool === 'propose' ? { dry_run: dryRun } : {}
  const lines = [
    {
      jsonrpc: '2.0',
      id: INITIALIZE_ID,
      method: 'initialize',
      params: { protocolVersion: '2024-11-05', clientInfo: { name: 'yidam-edit' } },
    },
    { jsonrpc: '2.0', method: 'notifications/initialized' },
    { jsonrpc: '2.0', id: CALL_ID, method: 'tools/call', params: { name: tool, arguments: args } },
  ]
  return lines.map((l) => JSON.stringify(l)).join('\n') + '\n'
}

/**
 * One act-tier call, as the binary answered it.
 *
 * `dryRun` is read for `propose` only; `cycle` has no arguments. Errors are values rather
 * than throws, as in `cli.ts`, because a refused write is the ordinary case on a corpus that
 * has not opted in and a page must render it as an answer rather than a crash.
 */
export async function spawnAct<T>(
  input: ActInput,
  tool: ActTool,
  dryRun = false,
): Promise<ActResult<T>> {
  const exec = input.exec ?? defaultExec
  let stdout = ''
  let stderr = ''
  try {
    const out = await exec(input.command, ['serve', '--mcp'], {
      cwd: input.root,
      input: frame(tool, dryRun),
    })
    stdout = out.stdout
    stderr = out.stderr
  } catch (e) {
    // The binary could not be started at all — `ENOENT`, a permission bit. Nothing on the
    // wire to read, and the exception's own text is the only account there is.
    const err = e as { message?: string; stdout?: string; stderr?: string }
    stdout = err.stdout ?? ''
    stderr = err.stderr ?? err.message ?? ''
  }

  const responses = new Map<number, Record<string, unknown>>()
  for (const line of stdout.split('\n')) {
    if (line.trim() === '') continue
    // Every line on stdout is one JSON-RPC message; the banner goes to stderr. A line that
    // is not JSON is a binary that is not speaking the protocol, and the loop below reports
    // that as no server rather than guessing at the rest.
    let message: unknown
    try {
      message = JSON.parse(line)
    } catch {
      return { ok: false, kind: 'no-server', tool, error: notProtocol(line, stderr) }
    }
    if (typeof message === 'object' && message !== null && 'id' in message) {
      const id = (message as { id: unknown }).id
      if (typeof id === 'number') responses.set(id, message as Record<string, unknown>)
    }
  }

  const init = responses.get(INITIALIZE_ID)
  if (init === undefined || 'error' in init) {
    return {
      ok: false,
      kind: 'no-server',
      tool,
      error: stderr.trim() || 'yidam serve --mcp answered nothing and said nothing on stderr',
    }
  }
  const capabilities = readCapabilities(init)

  const call = responses.get(CALL_ID)
  if (call === undefined || 'error' in call) {
    // A protocol-level error on the call — `-32601`, `-32602` — is not a tool refusal: the
    // frame above was not understood, which is a fact about this file and that binary
    // rather than about the corpus.
    const detail =
      call && 'error' in call ? JSON.stringify(call.error) : 'no response to tools/call'
    return { ok: false, kind: 'no-server', tool, error: `${detail}\n${stderr}`.trim() }
  }

  const result = call.result as { isError?: boolean; content?: { text?: string }[] }
  const text = result.content?.[0]?.text ?? ''
  if (result.isError === true) {
    return {
      ok: false,
      kind: 'refused',
      tool,
      act: capabilities.act,
      contract: capabilities.contract,
      error: text,
    }
  }
  if (capabilities.act !== 'declared' || capabilities.contract === null) {
    // A tool result with no error from a server that did not declare the tier is a server
    // this file does not understand — the contract says the call is refused. Do not render
    // it as a write that happened.
    return {
      ok: false,
      kind: 'no-server',
      tool,
      error: `\`${tool}\` answered on a server whose handshake declared act=${capabilities.act}`,
    }
  }
  let parsed: T
  try {
    parsed = JSON.parse(text) as T
  } catch {
    return { ok: false, kind: 'no-server', tool, error: notProtocol(text, stderr) }
  }
  return { ok: true, tool, act: 'declared', contract: capabilities.contract, result: parsed }
}

/** The `yidam` capability block off `initialize`, reduced to the two facts this surface reads. */
function readCapabilities(init: Record<string, unknown>): {
  act: Declared
  contract: string | null
} {
  const result = init.result as { capabilities?: { yidam?: Record<string, unknown> } } | undefined
  const yidam = result?.capabilities?.yidam
  const contract = typeof yidam?.contract === 'string' ? yidam.contract : null
  const act: Declared =
    yidam === undefined || !('act' in yidam)
      ? 'predates-act'
      : yidam.act === true
        ? 'declared'
        : 'undeclared'
  return { act, contract }
}

function notProtocol(line: string, stderr: string): string {
  const shown = line.length > 200 ? `${line.slice(0, 200)}…` : line
  return `yidam serve --mcp wrote something that is not JSON-RPC: ${shown}\n${stderr}`.trim()
}

const defaultExec: Exec = (command, args, options) =>
  new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: options.cwd, stdio: ['pipe', 'pipe', 'pipe'] })
    let stdout = ''
    let stderr = ''
    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk: string) => (stdout += chunk))
    child.stderr.on('data', (chunk: string) => (stderr += chunk))
    child.on('error', reject)
    child.on('close', (code) => resolve({ stdout, stderr, code }))
    // All three lines at once, then EOF. The server reads them in order and exits when
    // stdin closes; a server that refused to start never reads them and the write lands on
    // a closed pipe, which is an `EPIPE` on stdin and not a failure of the call.
    child.stdin.on('error', () => {})
    child.stdin.end(options.input)
  })
