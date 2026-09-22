/**
 * `yidam serve --lsp` on the other end of a pipe — RFC-0030 Phase 2, #607.
 *
 * The overlay is the LSP server's own idea: a buffer the editor holds shadows the file on
 * disk, and the checks run over the shadow. `lsp.rs` already does that for the extension, so
 * the web surface asks the same server the same way rather than growing a second linter —
 * RFC-0016's rule for this package is that TypeScript computes affordances and the CLI
 * computes verdicts, and a browser's unsaved text is the case where a re-derivation would be
 * most tempting and most wrong.
 *
 * # What this file decides
 *
 * Bytes. `Content-Length` framing in both directions, ids on requests and their answers
 * matched back up, `initialize` before anything else and `shutdown`/`exit` after everything
 * else. It does not decide what a diagnostic means, which buffer is a node, or when a child
 * that died should be started again — `overlay.ts` holds the second and third of those, and
 * the binary holds the first.
 *
 * # Why it is a class where `act.ts` is a function
 *
 * `act.ts` opens one connection per request and lets stdin's EOF end it, which is right for a
 * call that answers once. An overlay is a conversation: the server keeps the buffer between
 * messages, and the point of the barrier request (`overlay.ts`) is that the *n*th change is
 * judged against the *n−1* that came before it. So the process outlives the request, and
 * something has to hold it. `test/lsp.mjs` drives this against a fake child; nothing here
 * needs a binary to be tested.
 *
 * # The three failure modes the issue names
 *
 * - **Framing.** `Framer` reassembles messages from whatever chunk boundaries the pipe
 *   chose: a header split across reads, two messages in one read, a body that arrives after
 *   its header. A header block with no `Content-Length` is a process that is not speaking
 *   the protocol, and it is reported as that rather than skipped over.
 * - **Correlation.** Every request carries a fresh id and waits on it; a reply with an id
 *   nothing is waiting on is dropped, and a notification (no id) goes to the one listener.
 * - **A child dying mid-session.** Every pending request is rejected the moment the process
 *   exits, with the stderr tail the server left, so the caller can say *verdicts stopped* and
 *   why — never a promise that hangs until the page is closed.
 */

import { spawn, type ChildProcess } from 'node:child_process'
import type { Readable, Writable } from 'node:stream'
import { pathToFileURL } from 'node:url'

export interface Message {
  jsonrpc?: string
  id?: number | string | null
  method?: string
  params?: unknown
  result?: unknown
  error?: { code: number; message: string; data?: unknown }
}

/** One message, framed the way `lsp.rs` reads it: the byte length, not the string length. */
export function frame(message: unknown): Buffer {
  const body = Buffer.from(JSON.stringify(message), 'utf8')
  return Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, 'ascii'), body])
}

/**
 * Messages out of a byte stream, across whatever chunk boundaries the pipe produces.
 *
 * `feed` returns every message that is complete after the chunk and keeps the rest. It
 * throws on a header block with no `Content-Length`, because the only way to resynchronise
 * after that is to guess, and a guess here is a diagnostic attributed to the wrong buffer.
 */
export class Framer {
  private pending: Buffer = Buffer.alloc(0)

  feed(chunk: Buffer): Message[] {
    this.pending = this.pending.length === 0 ? chunk : Buffer.concat([this.pending, chunk])
    const out: Message[] = []
    for (;;) {
      const headerEnd = this.pending.indexOf('\r\n\r\n')
      if (headerEnd === -1) break
      const header = this.pending.subarray(0, headerEnd).toString('ascii')
      const length = contentLength(header)
      if (length === null) {
        throw new Error(`a header block with no Content-Length: ${JSON.stringify(header)}`)
      }
      const bodyStart = headerEnd + 4
      if (this.pending.length < bodyStart + length) break
      const body = this.pending.subarray(bodyStart, bodyStart + length).toString('utf8')
      this.pending = this.pending.subarray(bodyStart + length)
      out.push(JSON.parse(body) as Message)
    }
    return out
  }
}

/** Case-insensitive, because the spec says header names are and `lsp.rs` reads them so. */
function contentLength(header: string): number | null {
  for (const line of header.split('\r\n')) {
    const colon = line.indexOf(':')
    if (colon === -1) continue
    if (line.slice(0, colon).trim().toLowerCase() !== 'content-length') continue
    const value = Number(line.slice(colon + 1).trim())
    if (Number.isInteger(value) && value >= 0) return value
  }
  return null
}

/** What a process looks like from here — the subset of `ChildProcess` this file touches. */
export interface Child {
  stdin: Writable
  stdout: Readable
  stderr: Readable | null
  on(event: 'exit', listener: (code: number | null, signal: string | null) => void): unknown
  on(event: 'error', listener: (error: Error) => void): unknown
  kill(): unknown
}

export type Spawn = (command: string, args: string[], options: { cwd: string }) => Child

export interface Exit {
  code: number | null
  signal: string | null
  /** The last of what the server wrote to stderr — where `serve` explains itself. */
  stderr: string
}

export interface StartInput {
  /** Absolute path to the binary `resolveBinary` found. */
  command: string
  /** The corpus root, already resolved; both the working directory and the `rootUri`. */
  root: string
  /** Injected so tests need no binary. */
  spawn?: Spawn
}

/** The argv, held here so `test/boundary.mjs` can see it: the LSP server and nothing else. */
export const LSP_ARGS = ['serve', '--lsp']

const STDERR_TAIL = 4096

/** Thrown into every pending request when the process ends; carries what stderr said. */
export class LspExited extends Error {
  readonly exit: Exit
  constructor(exit: Exit) {
    super(describeExit(exit))
    this.name = 'LspExited'
    this.exit = exit
  }
}

export function describeExit(exit: Exit): string {
  const how =
    exit.signal !== null
      ? `killed by ${exit.signal}`
      : exit.code !== null
        ? `exited ${exit.code}`
        : 'could not be started'
  const tail = exit.stderr.trim()
  return tail === '' ? `yidam serve --lsp ${how}` : `yidam serve --lsp ${how}:\n${tail}`
}

/**
 * One LSP server, from spawn to exit.
 *
 * `initialize` is a separate step from construction so a caller can hold the process and the
 * handshake's answer apart: the capabilities block is where the server says whether it lints
 * a buffer that is not a file yet (`experimental.yidam.unsavedInstances`), and a client that
 * needs that fact needs it before the first change, not after.
 */
export class LspClient {
  private readonly child: Child
  private readonly framer = new Framer()
  private readonly pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>()
  private listener: ((method: string, params: unknown) => void) | null = null
  private nextId = 1
  private stderrTail = ''
  private ended: Exit | null = null
  private readonly exitListeners: ((exit: Exit) => void)[] = []

  /** Resolves when the process has ended, however it ended. Never rejects. */
  readonly exited: Promise<Exit>

  private constructor(child: Child) {
    this.child = child
    this.exited = new Promise((resolve) => this.exitListeners.push(resolve))

    child.stdout.on('data', (chunk: Buffer) => {
      let messages: Message[]
      try {
        messages = this.framer.feed(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))
      } catch (e) {
        // Not the protocol. Kill it rather than read on: every later byte would be a guess.
        this.stderrTail += `\n${(e as Error).message}`
        child.kill()
        return
      }
      for (const message of messages) this.dispatch(message)
    })
    child.stderr?.on('data', (chunk: Buffer | string) => {
      this.stderrTail = (this.stderrTail + chunk.toString()).slice(-STDERR_TAIL)
    })
    child.on('exit', (code, signal) => this.end({ code, signal, stderr: this.stderrTail }))
    child.on('error', (error) => {
      // `ENOENT`, a permission bit: the process never ran. Node emits no `exit` after this,
      // so it is the end of the client as much as an exit is.
      this.stderrTail = (this.stderrTail + `\n${error.message}`).slice(-STDERR_TAIL)
      this.end({ code: null, signal: null, stderr: this.stderrTail })
    })
    // A server that refused to start never reads its stdin, and the write lands on a closed
    // pipe. That `EPIPE` is reported through `exit`, not here.
    child.stdin.on('error', () => {})
  }

  static start(input: StartInput): LspClient {
    const spawnChild: Spawn =
      input.spawn ??
      ((command, args, options) =>
        spawn(command, args, { cwd: options.cwd, stdio: ['pipe', 'pipe', 'pipe'] }) as ChildProcess as Child)
    return new LspClient(spawnChild(input.command, LSP_ARGS, { cwd: input.root }))
  }

  /** The handshake, as the server answered it. `rootUri` is the corpus root as a file URI. */
  async initialize(root: string): Promise<Record<string, unknown>> {
    const result = await this.request('initialize', {
      processId: process.pid,
      rootUri: pathToFileURL(root).href,
      capabilities: {},
      clientInfo: { name: 'yidam-edit' },
    })
    this.notify('initialized', {})
    return typeof result === 'object' && result !== null ? (result as Record<string, unknown>) : {}
  }

  request(method: string, params: unknown): Promise<unknown> {
    if (this.ended !== null) return Promise.reject(new LspExited(this.ended))
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.write({ jsonrpc: '2.0', id, method, params })
    })
  }

  notify(method: string, params: unknown): void {
    if (this.ended !== null) return
    this.write({ jsonrpc: '2.0', method, params })
  }

  /** One listener, replaced rather than added: the bridge is the only reader there is. */
  onNotification(listener: (method: string, params: unknown) => void): void {
    this.listener = listener
  }

  onExit(listener: (exit: Exit) => void): void {
    if (this.ended !== null) listener(this.ended)
    else this.exitListeners.push(listener)
  }

  get isAlive(): boolean {
    return this.ended === null
  }

  /**
   * The polite end, then the blunt one.
   *
   * `shutdown` asks, `exit` tells, and a server that has not gone after `grace` milliseconds
   * is killed — a supervisor that waits forever on a wedged child is a supervisor that leaks
   * it. Resolves with how the process actually ended.
   */
  async shutdown(grace = 2000): Promise<Exit> {
    if (this.ended !== null) return this.ended
    // The clock starts before the question is asked: a server that never answers
    // `shutdown` is the wedged case, and it is the one the kill is for.
    const timer = setTimeout(() => this.child.kill(), grace)
    try {
      await Promise.race([this.request('shutdown', null), this.exited])
    } catch {
      // Rejected because it exited under us; that is the outcome wanted.
    }
    this.notify('exit', null)
    const exit = await this.exited
    clearTimeout(timer)
    return exit
  }

  kill(): void {
    if (this.ended === null) this.child.kill()
  }

  private write(message: Message): void {
    this.child.stdin.write(frame(message))
  }

  private dispatch(message: Message): void {
    if (message.id !== undefined && message.id !== null && message.method === undefined) {
      const waiting = typeof message.id === 'number' ? this.pending.get(message.id) : undefined
      if (waiting === undefined) return
      this.pending.delete(message.id as number)
      if (message.error !== undefined) {
        waiting.reject(new Error(`${message.error.code}: ${message.error.message}`))
      } else {
        waiting.resolve(message.result ?? null)
      }
      return
    }
    if (message.method !== undefined && (message.id === undefined || message.id === null)) {
      this.listener?.(message.method, message.params)
      return
    }
    // A request from the server. `lsp.rs` sends none today; answer so a future one does not
    // hang the server waiting on a client that has no handler.
    if (message.method !== undefined && message.id !== undefined) {
      this.write({
        jsonrpc: '2.0',
        id: message.id,
        error: { code: -32601, message: `yidam-edit does not answer ${message.method}` },
      })
    }
  }

  private end(exit: Exit): void {
    if (this.ended !== null) return
    this.ended = exit
    const failure = new LspExited(exit)
    for (const waiting of this.pending.values()) waiting.reject(failure)
    this.pending.clear()
    for (const listener of this.exitListeners) listener(exit)
    this.exitListeners.length = 0
  }
}
