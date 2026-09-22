/**
 * The overlay bridge: buffers in from browsers, verdicts out to them — RFC-0030 Phase 2, #607.
 *
 * One `yidam serve --lsp` per process, started when the first page subscribes and stopped
 * when the last one leaves, holding every open buffer the way an editor would. The browser
 * never sees the protocol: it `POST`s text to `/api/overlay/change` and reads `verdict`
 * events off `GET /api/overlay`, and this file is the whole of the translation.
 *
 * # What this file decides
 *
 * Supervision and addressing, and nothing about a node. It decides when to start the child,
 * when a dead one may be started again, which subscriber holds which buffer, and how a
 * corpus-relative id like `gage/new.yml` becomes the file URI the server keys its overlay by.
 * Every diagnostic it relays is the server's, verbatim: `code` is the check's id, `message`
 * is the check's detail and rationale, `source` says whether the baseline already forgives it.
 *
 * # The barrier
 *
 * The server publishes diagnostics only for a buffer that has findings, or that had some and
 * now has none. A clean buffer that was always clean publishes nothing, so *pending* and
 * *clean* would be the same silence. `lsp.rs` answers any request it does not know with
 * `null`, after everything it has already published — and `yidam/cli/src/cmd/lsp.rs` pins
 * that order. So every change is followed by a `$/yidam/barrier` request, and the verdict
 * event goes out when the reply comes back: whatever the map holds for the buffer at that
 * moment is the server's answer to this text, empty included.
 *
 * # Degrading loudly
 *
 * A child that dies takes every verdict with it. The bridge says so — a `status` event with
 * `state: 'exited'` and the stderr tail — forgets what the server held, and starts a fresh
 * one on the next change, replaying every buffer it still has so the page's verdicts are
 * about the page's text again. Three starts in a minute is a binary that cannot run here,
 * and the bridge stops trying and says that instead: a supervisor that restarts forever is
 * a supervisor that hides the reason.
 *
 * # Why `corpus_dir` comes from a graph report
 *
 * The server keys the overlay by absolute path, and a browser has no business knowing one —
 * a doc id is corpus-relative, as a node's identity is everywhere else on this surface. The
 * one report that says where the corpus directory is relative to the root is `yidam graph`,
 * so it is asked once, before the first start, and refused loudly if it does not answer:
 * a bridge that guessed `.yidam/corpus` would lint the right text at the wrong path, and the
 * server would say the node was not in the corpus.
 */

import { randomUUID } from 'node:crypto'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { spawnReport } from './cli.ts'
import { LspClient, LspExited, describeExit, type Exit, type Spawn } from './lsp.ts'
import { describeFailure } from './messages.ts'
import type { Session } from './session.ts'

/** A diagnostic as `lsp.rs` publishes it. Relayed whole; nothing here reads inside it. */
export interface Diagnostic {
  range: { start: { line: number; character: number }; end: { line: number; character: number } }
  severity?: number
  code?: string | number
  source?: string
  message: string
}

export type BridgeState = 'idle' | 'starting' | 'ready' | 'exited' | 'unavailable'

export interface Status {
  state: BridgeState
  /** The subscriber's own id, to put on every change it sends. */
  client: string
  /**
   * Whether the server lints a buffer that is not a file yet, as its handshake said.
   * `null` until the handshake answers, and `null` when the server did not say — a binary
   * older than #607 lints saved nodes only, and the page says so rather than waiting.
   */
  unsavedInstances: boolean | null
  /** The corpus directory relative to the root, off `yidam graph`. */
  corpusDir: string | null
  /** Why `exited` or `unavailable`, in the server's own words where it left any. */
  detail: string | null
  /** How many times this process has started the child. Two is a restart. */
  starts: number
}

export interface Verdict {
  doc: string
  diagnostics: Diagnostic[]
  /** Monotonic per bridge; a page keeps the highest it has seen for a doc. */
  seq: number
}

export type OverlayEvent = { event: 'status'; data: Status } | { event: 'verdict'; data: Verdict }

/** One server-sent event, wire form. JSON carries no raw newline, so one `data:` line is enough. */
export function sseEvent(event: OverlayEvent): string {
  return `event: ${event.event}\ndata: ${JSON.stringify(event.data)}\n\n`
}

/**
 * Whether a string may name a buffer, and why not.
 *
 * The same predicate `walk_corpus_instances` applies on disk and `Overlay::unsaved_instances`
 * applies to buffers: corpus-relative, at least one directory deep, `.yml` and not `.ont.yml`.
 * Stricter on characters than the walker is, because an id is going into a path the server
 * will open and a browser is the one proposing it — a segment is a plain filename here or
 * it is refused. A class file is refused too: this surface drafts nodes, not ontologies.
 */
export function docIdError(doc: string): string | null {
  const segments = doc.split('/')
  if (segments.length < 2) return 'a node lives under its class directory: expected <class>/<name>.yml'
  for (const segment of segments) {
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(segment) || segment === '..' || segment === '.') {
      return `not a path segment: ${JSON.stringify(segment)}`
    }
  }
  const name = segments[segments.length - 1]
  if (!name.endsWith('.yml')) return 'a node is a .yml file'
  if (name.endsWith('.ont.yml')) return 'an .ont.yml is a class, and this surface drafts nodes'
  return null
}

export type ChangeResult =
  | { ok: true; seq: number }
  | { ok: false; status: 400 | 409 | 503; error: string }

export interface BridgeDeps {
  session: () => Promise<Session>
  /** Injected so tests need no binary. */
  report?: typeof spawnReport
  spawn?: Spawn
  now?: () => number
  /** Where to hang the kill-on-exit hook; tests pass a no-op. */
  onProcessExit?: (hook: () => void) => void
}

interface Subscriber {
  send: (event: OverlayEvent) => void
  docs: Set<string>
}

interface Doc {
  text: string
  holders: Set<string>
  /** Whether *this* child has been told about it. Reset when the child dies. */
  opened: boolean
}

interface Corpus {
  command: string
  root: string
  corpusDir: string
}

/** Three starts inside this window is a binary that cannot run here. */
const START_BUDGET = 3
const START_WINDOW_MS = 60_000

export class OverlayBridge {
  private readonly deps: Required<Pick<BridgeDeps, 'session' | 'report' | 'now' | 'onProcessExit'>> &
    Pick<BridgeDeps, 'spawn'>
  private readonly subscribers = new Map<string, Subscriber>()
  private readonly docs = new Map<string, Doc>()
  /** Keyed by absolute path, not URI: the server may spell a URI differently than node does. */
  private readonly diagnostics = new Map<string, Diagnostic[]>()
  private client: LspClient | null = null
  private starting: Promise<LspClient | string> | null = null
  private corpus: Promise<Corpus | string> | null = null
  private state: BridgeState = 'idle'
  private detail: string | null = null
  private unsavedInstances: boolean | null = null
  private corpusDir: string | null = null
  private root: string | null = null
  private readonly startTimes: number[] = []
  private seq = 0
  private hooked = false

  constructor(deps: BridgeDeps) {
    this.deps = {
      session: deps.session,
      report: deps.report ?? spawnReport,
      now: deps.now ?? Date.now,
      onProcessExit: deps.onProcessExit ?? ((hook) => process.once('exit', hook)),
      spawn: deps.spawn,
    }
  }

  /** A page arriving. Its first event is a `status` carrying the id it will send changes as. */
  subscribe(send: (event: OverlayEvent) => void): { client: string; close: () => void } {
    const client = randomUUID()
    this.subscribers.set(client, { send, docs: new Set() })
    send({ event: 'status', data: this.status(client) })
    // Start eagerly rather than on the first change, so the handshake's answer — whether an
    // unsaved buffer is linted at all — reaches the page before anyone has typed.
    void this.ensure()
    return { client, close: () => this.unsubscribe(client) }
  }

  /** One buffer's new text. Resolves when the server has judged it and the verdict is sent. */
  async change(client: string, doc: string, text: string): Promise<ChangeResult> {
    const subscriber = this.subscribers.get(client)
    if (subscriber === undefined) {
      return { ok: false, status: 409, error: 'subscribe to /api/overlay first; this client id is not open' }
    }
    const invalid = docIdError(doc)
    if (invalid !== null) return { ok: false, status: 400, error: invalid }

    const entry = this.docs.get(doc) ?? { text, holders: new Set<string>(), opened: false }
    entry.text = text
    entry.holders.add(client)
    this.docs.set(doc, entry)
    subscriber.docs.add(doc)

    const ready = await this.ensure()
    if (typeof ready === 'string') return { ok: false, status: 503, error: ready }
    try {
      await this.sync(ready, doc)
    } catch (e) {
      // The child went while this change was in flight. The exit handler has already told
      // every subscriber; this answers the one request that was waiting on it.
      return { ok: false, status: 503, error: e instanceof LspExited ? e.message : String(e) }
    }
    return { ok: true, seq: this.publish(doc) }
  }

  /**
   * A page done with a buffer — it renamed the draft, or cleared the form. The doc is closed
   * on the server when nobody else holds it, so a draft renamed on every keystroke does not
   * leave a trail of ghost nodes for `duplicate-label` to find each other.
   */
  release(client: string, doc: string): ChangeResult {
    const subscriber = this.subscribers.get(client)
    if (subscriber === undefined) {
      return { ok: false, status: 409, error: 'subscribe to /api/overlay first; this client id is not open' }
    }
    subscriber.docs.delete(doc)
    this.releaseDoc(client, doc)
    return { ok: true, seq: this.seq }
  }

  /** How many subscribers hold the stream. Exported for the supervision tests. */
  get open(): number {
    return this.subscribers.size
  }

  get isRunning(): boolean {
    return this.client !== null && this.client.isAlive
  }

  /** Stop the child now, whatever the subscriber count. The process-exit hook, and tests. */
  kill(): void {
    this.client?.kill()
  }

  private status(client: string): Status {
    return {
      state: this.state,
      client,
      unsavedInstances: this.unsavedInstances,
      corpusDir: this.corpusDir,
      detail: this.detail,
      starts: this.startTimes.length,
    }
  }

  private broadcast(): void {
    for (const [client, subscriber] of this.subscribers) {
      subscriber.send({ event: 'status', data: this.status(client) })
    }
  }

  private unsubscribe(client: string): void {
    const subscriber = this.subscribers.get(client)
    if (subscriber === undefined) return
    this.subscribers.delete(client)
    for (const doc of subscriber.docs) this.releaseDoc(client, doc)
    if (this.subscribers.size === 0 && this.client !== null) {
      // The last page left. A child with nobody to answer is the leak the issue names.
      const leaving = this.client
      this.client = null
      this.state = 'idle'
      void leaving.shutdown()
    }
  }

  private releaseDoc(client: string, doc: string): void {
    const entry = this.docs.get(doc)
    if (entry === undefined) return
    entry.holders.delete(client)
    if (entry.holders.size > 0) return
    this.docs.delete(doc)
    this.diagnostics.delete(this.pathOf(doc))
    if (entry.opened && this.client?.isAlive) {
      this.client.notify('textDocument/didClose', { textDocument: { uri: this.uriOf(doc) } })
    }
  }

  /** The running child, or the reason there is none. Starts one when it may. */
  private ensure(): Promise<LspClient | string> {
    if (this.client !== null && this.client.isAlive) return Promise.resolve(this.client)
    if (this.starting !== null) return this.starting
    if (this.state === 'unavailable') return Promise.resolve(this.detail ?? 'unavailable')

    const now = this.deps.now()
    const recent = this.startTimes.filter((t) => now - t < START_WINDOW_MS)
    if (recent.length >= START_BUDGET) {
      this.state = 'unavailable'
      this.detail =
        `yidam serve --lsp has been started ${recent.length} times in the last minute and has ` +
        `not stayed up. Not trying again.\nLast exit: ${this.detail ?? 'unknown'}`
      this.broadcast()
      return Promise.resolve(this.detail)
    }

    this.starting = this.start().finally(() => {
      this.starting = null
    })
    return this.starting
  }

  private async start(): Promise<LspClient | string> {
    this.state = 'starting'
    this.broadcast()

    this.corpus ??= this.locate()
    const corpus = await this.corpus
    if (typeof corpus === 'string') {
      this.state = 'unavailable'
      this.detail = corpus
      this.broadcast()
      return corpus
    }
    this.corpusDir = corpus.corpusDir
    this.root = corpus.root

    this.startTimes.push(this.deps.now())
    const client = LspClient.start({ command: corpus.command, root: corpus.root, spawn: this.deps.spawn })
    this.client = client
    if (!this.hooked) {
      this.hooked = true
      this.deps.onProcessExit(() => this.kill())
    }
    client.onNotification((method, params) => this.onNotification(method, params))
    client.onExit((exit) => this.onExit(client, exit))

    let handshake: Record<string, unknown>
    try {
      handshake = await client.initialize(corpus.root)
    } catch (e) {
      // The exit handler has set `exited` and told everyone; this is the answer for whoever
      // was waiting on the start.
      return e instanceof LspExited ? e.message : String(e)
    }
    this.unsavedInstances = readUnsavedInstances(handshake)
    this.state = 'ready'
    this.detail = null

    // A restart: the buffers survived the child, so it is told about all of them and every
    // page's verdicts are about its own text again before anyone types.
    const held = [...this.docs.keys()]
    try {
      for (const doc of held) await this.sync(client, doc)
    } catch (e) {
      return e instanceof LspExited ? e.message : String(e)
    }
    this.broadcast()
    for (const doc of held) this.publish(doc)
    return client
  }

  /** Where the corpus is: root and binary off the session, `corpus_dir` off one graph report. */
  private async locate(): Promise<Corpus | string> {
    const { root, binary } = await this.deps.session()
    if (binary.command === null) return `no yidam binary: ${binary.reason}`
    const graph = await this.deps.report<{ corpus_dir?: unknown }>({ command: binary.command, root }, 'graph')
    if (!graph.ok) return describeFailure(graph.handshake)
    if (typeof graph.report.corpus_dir !== 'string') {
      return 'yidam graph did not report corpus_dir, so this bridge cannot address a buffer; re-pin the binary'
    }
    return { command: binary.command, root, corpusDir: graph.report.corpus_dir }
  }

  /** Tell the child about a buffer, then wait until it has judged everything sent so far. */
  private async sync(client: LspClient, doc: string): Promise<void> {
    const entry = this.docs.get(doc)
    if (entry === undefined) return
    const uri = this.uriOf(doc)
    if (entry.opened) {
      client.notify('textDocument/didChange', {
        textDocument: { uri, version: ++this.seq },
        contentChanges: [{ text: entry.text }],
      })
    } else {
      entry.opened = true
      client.notify('textDocument/didOpen', {
        textDocument: { uri, languageId: 'yaml', version: ++this.seq, text: entry.text },
      })
    }
    await client.request('$/yidam/barrier', {})
  }

  /** Send the doc's current verdict to everyone holding it. Returns the seq it went out as. */
  private publish(doc: string): number {
    const entry = this.docs.get(doc)
    const seq = ++this.seq
    if (entry === undefined) return seq
    const verdict: Verdict = { doc, diagnostics: this.diagnostics.get(this.pathOf(doc)) ?? [], seq }
    for (const holder of entry.holders) {
      this.subscribers.get(holder)?.send({ event: 'verdict', data: verdict })
    }
    return seq
  }

  private onNotification(method: string, params: unknown): void {
    if (method !== 'textDocument/publishDiagnostics') return
    const { uri, diagnostics } = params as { uri: string; diagnostics: Diagnostic[] }
    let file: string
    try {
      file = path.normalize(fileURLToPath(uri))
    } catch {
      return
    }
    this.diagnostics.set(file, diagnostics)
  }

  private onExit(client: LspClient, exit: Exit): void {
    if (this.client !== client) return
    this.client = null
    this.state = 'exited'
    this.detail = describeExit(exit)
    this.diagnostics.clear()
    for (const entry of this.docs.values()) entry.opened = false
    this.broadcast()
  }

  /** Only meaningful once `locate` has answered; every caller runs after `ensure`. */
  private pathOf(doc: string): string {
    return path.normalize(path.join(this.root ?? '', this.corpusDir ?? '', doc))
  }

  private uriOf(doc: string): string {
    return pathToFileURL(this.pathOf(doc)).href
  }
}

function readUnsavedInstances(handshake: Record<string, unknown>): boolean | null {
  const caps = handshake.capabilities as { experimental?: { yidam?: { unsavedInstances?: unknown } } } | undefined
  const value = caps?.experimental?.yidam?.unsavedInstances
  return typeof value === 'boolean' ? value : null
}

let shared: OverlayBridge | null = null

/** The process-wide bridge, one per server the way `session()` is one per server. */
export function overlay(session: () => Promise<Session>): OverlayBridge {
  shared ??= new OverlayBridge({ session })
  return shared
}
