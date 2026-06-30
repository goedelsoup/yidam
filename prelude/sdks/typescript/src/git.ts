export type CommitKind = 'Epistemic' | 'Operational'

export interface CommitEvent {
  hash: string
  kind: CommitKind
  verb: string
  subject: string
  context?: string
}

const OPERATIONAL_VERBS = new Set([
  'extract', 'refresh', 'compute', 'index', 'bundle', 'reconcile', 'build',
])

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
