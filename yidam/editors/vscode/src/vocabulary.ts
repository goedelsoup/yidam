/**
 * The closed commit vocabulary, in the commit box.
 *
 * Both rules here — a verb outside the list, a conventional-commits `(scope)` suffix —
 * already exist and are already checked, by `yidam lint --commits`, **after** the commit.
 * This moves them to before it, and that is the entire feature. GRAPH.md is explicit about
 * why it matters: the check is Warn severity and correctly so, since history cannot be
 * rewritten to fix a verb — *"that also means it reports drift only after the drift is
 * permanent."*
 *
 * Nothing here decides whether a verb is legal. The verdict crosses the process boundary
 * as `yidam vocabulary --check <subject> --format json`, computed by
 * `classify_commit` and `is_recognized_verb` — the parity-certified pair, proven total
 * against the Dafny spec. A TypeScript copy of the list would be a second source of truth
 * for a rule the prelude owns and re-vendoring may change: `resolve`, `scope` and `adopt`
 * were all added exactly that way.
 *
 * What *is* decided here: when to offer completion, where to draw the underline, and how
 * to sort the list. Affordances, whose failure mode is not helping.
 *
 * No `vscode` import.
 */

import type { Envelope } from './reports.ts'

export interface Verb {
  verb: string
  kind: 'epistemic' | 'operational'
  /** The **When** column, or empty when no GRAPH.md was found. */
  when: string
}

export interface SubjectViolation {
  rule: 'scope-suffix' | 'unrecognized-verb' | 'no-verb'
  severity: 'error' | 'warn' | 'info'
  message: string
}

export interface SubjectCheck {
  text: string
  verb: string
  kind: 'epistemic' | 'operational'
  recognized: boolean
  violations: SubjectViolation[]
}

export interface VocabularyReport extends Envelope {
  source: string
  verbs: Verb[]
  drift: string[]
  subject?: SubjectCheck
}

/**
 * Whether the cursor is in the verb position.
 *
 * The verb is everything before the first `: ` on the subject line, so once that separator
 * is behind the cursor the user is writing prose and a list of thirty verbs is noise. Line
 * 0 only: the body of a commit message is not a subject.
 */
export function inVerbPosition(line: string, character: number, lineNumber: number): boolean {
  if (lineNumber !== 0) return false
  return !line.slice(0, character).includes(': ')
}

export interface Completion {
  label: string
  detail: string
  documentation: string
  /** `insertText` completes the separator too — the verb is never written without it. */
  insertText: string
  /** Epistemic first, then operational, each alphabetical as the CLI ordered them. */
  sortText: string
}

/**
 * The verb list as completions, epistemic before operational.
 *
 * Order is not alphabetical across the whole set on purpose. The two kinds are the
 * distinction the vocabulary exists to make, and interleaving them puts `bundle` between
 * `assess` and `close` — which is exactly the confusion a closed vocabulary is for.
 */
export function completions(report: VocabularyReport): Completion[] {
  const rank = (v: Verb) => (v.kind === 'epistemic' ? '0' : '1')
  return report.verbs.map((v, i) => ({
    label: v.verb,
    detail: v.when || v.kind,
    documentation:
      (v.when ? `${v.when}\n\n` : '') +
      (v.kind === 'epistemic'
        ? 'Epistemic — understanding was added, revised, or retracted.'
        : 'Operational — the pipeline advanced; no understanding changed.'),
    insertText: `${v.verb}: `,
    sortText: `${rank(v)}${String(i).padStart(3, '0')}`,
  }))
}

export interface Mark {
  /** 0-based character offsets into the subject line. */
  start: number
  end: number
  message: string
  severity: SubjectViolation['severity']
  code: SubjectViolation['rule']
}

/**
 * Where to draw each underline.
 *
 * Under the *verb token* for the two rules that are about a verb, and under the whole line
 * for the one that is about its absence. A whole-line squiggle for a bad verb would point
 * at the subject the author got right.
 */
export function marks(subject: SubjectCheck): Mark[] {
  return subject.violations.map((v) => {
    const verbEnd = subject.verb.length
    const wholeLine = v.rule === 'no-verb' || verbEnd === 0
    return {
      start: 0,
      end: wholeLine ? Math.max(subject.text.length, 1) : verbEnd,
      message: v.message,
      // Rendered at the severity the CLI reported, never escalated. `lint --commits` is
      // Warn, and a commit box that squiggled harder than the gate would be asserting a
      // verdict nobody agreed to.
      severity: v.severity,
      code: v.rule,
    }
  })
}

/**
 * The subject line, as the CLI will read it.
 *
 * `classify_commit` takes `message.lines().next()`, and a commit message's later lines are
 * body. Sending the whole box would have the CLI check the first line anyway; sending only
 * the first line makes what is being checked visible on this side too.
 */
export function subjectLine(message: string): string {
  return message.split('\n')[0] ?? ''
}
