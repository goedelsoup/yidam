export type CommitKind = 'Epistemic' | 'Operational'

export interface CommitEvent {
  hash: string
  kind: CommitKind
  verb: string
  subject: string
  context?: string
}

/**
 * Verbs marking a commit as pipeline work rather than a change in understanding.
 *
 * Kept in sync with the commit vocabulary in `prelude/GRAPH.md` and with the
 * `OperationalVerbs` set in `prelude/sdks/spec/graph.dfy`.
 */
export const OPERATIONAL_VERBS = new Set([
  'extract', 'refresh', 'compute', 'index', 'bundle', 'reconcile', 'regen',
  'build', 'implement', 'scaffold', 'catalog', 'migrate', 'fix', 'vendor', 'consume',
])

/**
 * Verbs marking a commit as a change in understanding.
 *
 * `classifyCommit` does not consult this list — Operational is the explicitly marked case
 * and everything else is Epistemic, a totality the Dafny spec proves. The list exists so
 * that a verb belonging to *neither* set can be recognized as outside the vocabulary
 * entirely; see `isRecognizedVerb`.
 */
export const EPISTEMIC_VERBS = new Set([
  'establish', 'revise', 'assess', 'scope', 'synthesize', 'withdraw', 'open', 'close',
  'resolve', 'adopt', 'decide', 'phase', 'genesis', 'overlay',
])

/**
 * Whether `verb` is in the closed vocabulary defined by `prelude/GRAPH.md`.
 *
 * Deliberately separate from `classifyCommit`, which must remain total: every commit gets
 * a kind, including ones written before a repository adopted the vocabulary. Recognition
 * asks whether the log is legible; classification asks what a commit did.
 */
export function isRecognizedVerb(verb: string): boolean {
  return OPERATIONAL_VERBS.has(verb) || EPISTEMIC_VERBS.has(verb)
}

export function classifyCommit(hash: string, message: string): CommitEvent {
  const firstLine = (message.split('\n')[0] ?? '').trim()
  const colonPos = firstLine.indexOf(': ')
  let verb: string
  let subject: string
  if (colonPos !== -1) {
    verb = firstLine.slice(0, colonPos).trim()
    subject = firstLine.slice(colonPos + 2).trim()
  } else {
    verb = ''
    subject = firstLine
  }

  const kind: CommitKind = OPERATIONAL_VERBS.has(verb) ? 'Operational' : 'Epistemic'

  return { hash, kind, verb, subject }
}
