/**
 * What this surface says when the handshake fails.
 *
 * `handshake.ts` is a **byte-identical copy** of the extension's module — `test/parity.mjs`
 * asserts that, and the strength of that gate is exactly its strictness. The cost is that
 * two of its user-facing strings name the VS Code extension, because that is the surface
 * they were written for.
 *
 * Rather than fork the module and weaken the parity test to a fuzzy comparison, this file
 * owns the text for this surface and keys it off the failure *kind*, which is a stable part
 * of the contract. The copy stays exact; the wording stays correct.
 */

import type { Handshake } from './handshake.ts'

const REPIN =
  'Re-pin with `mise run yidam-vendor-update`, then rebuild with `mise run yidam-build`.'

export function describeFailure(h: Handshake): string {
  if (h.ok) return ''
  switch (h.kind) {
    case 'not-json':
      return (
        'This yidam does not speak the JSON report contract — it predates `--format json` ' +
        `(RFC-0016 Phase 0). Verdicts are unavailable. ${REPIN}`
      )
    case 'not-an-envelope':
      return 'yidam returned JSON that is not a report envelope. Verdicts are unavailable.'
    case 'unsupported-version':
      return (
        'This yidam speaks a report contract this editor does not understand. Verdicts are ' +
        `disabled rather than guessed at. Update \`@yidam/edit\`, or re-pin the binary. ${REPIN}`
      )
  }
}

/** One line for the header. Says which binary, so a person can reproduce the answer by hand. */
export function describeBinary(h: Handshake, origin: string): string {
  if (!h.ok) return 'yidam: unavailable'
  return `yidam ${h.yidam.version} (${h.yidam.commit}) — ${origin}`
}

/**
 * The corpus that was asked for, and the one that answered.
 *
 * **No subcommand takes a `--root` flag.** The corpus is resolved by
 * `git rev-parse --show-toplevel` from the working directory, so `--root` on this surface
 * sets a working directory and the binary decides from there. A corpus nested inside another
 * git repository therefore answers about the outer one — `examples/streamflow` is exactly
 * that case, and it renders as a corpus with no nodes in it.
 *
 * An empty page and a wrong page look identical, which is the whole reason this string
 * exists. Returns null when they agree, so the shell shows nothing in the common case.
 */
export function describeRootMismatch(asked: string, resolved: string | null): string | null {
  if (resolved === null || resolved === asked) return null
  return (
    `Asked for ${asked}, and yidam answered about ${resolved}. ` +
    'No yidam subcommand takes a --root flag: the corpus is resolved by ' +
    '`git rev-parse --show-toplevel` from the working directory, so a corpus nested inside ' +
    'another git repository resolves to the outer one. Open it as its own checkout.'
  )
}
