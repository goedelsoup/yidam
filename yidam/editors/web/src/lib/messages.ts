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
        `disabled rather than guessed at. Update \`@goedelsoup/yidam-edit\`, or re-pin the binary. ${REPIN}`
      )
  }
}

/** One line for the header. Says which binary, so a person can reproduce the answer by hand. */
export function describeBinary(h: Handshake, origin: string): string {
  if (!h.ok) return 'yidam: unavailable'
  return `yidam ${h.yidam.version} (${h.yidam.commit}) — ${origin}`
}

/**
 * Why a report is unavailable, preferring the binary's own words.
 *
 * A refusal — a named root holding no corpus (#1000), a directory that is not a git
 * repository — is the binary saying what is wrong with *this corpus*. `describeFailure` can
 * only say what is wrong with the binary, and for an empty stdout it says the binary predates
 * `--format json`, which would send the reader to re-pin one that is current.
 */
export function describeUnavailable(result: {
  handshake: Handshake
  refusal: string | null
}): string {
  return result.refusal ?? describeFailure(result.handshake)
}

/**
 * The corpus that was asked for, and the one that answered.
 *
 * Two ways they differ, and they need different repairs:
 *
 * - **`working-directory`** — the binary predates `--root` on reports (cli/v0.16.0) and
 *   resolved the working directory with `git rev-parse --show-toplevel`. A corpus nested
 *   inside another git repository answers about the outer one, and renders as a corpus with
 *   no nodes — `examples/streamflow` is exactly that case. The repair is a re-pin.
 * - **`flag`** — the binary took `--root` and walked up to the nearest `.yidam/`, so a
 *   directory *inside* a corpus answers for the corpus around it. Nothing is wrong; the
 *   reader is told which corpus they are looking at.
 *
 * An empty page and a wrong page look identical, which is the whole reason this string
 * exists. Returns null when they agree, so the shell shows nothing in the common case.
 */
export function describeRootMismatch(
  asked: string,
  resolved: string | null,
  by: 'flag' | 'working-directory',
): string | null {
  if (resolved === null || resolved === asked) return null
  if (by === 'flag') {
    return (
      `Asked for ${asked}, and yidam answered about ${resolved}: the nearest directory at or ` +
      'above it holding a .yidam/ corpus.'
    )
  }
  return (
    `Asked for ${asked}, and yidam answered about ${resolved}. ` +
    'This yidam predates --root on reports (cli/v0.16.0), so it resolved the corpus from the ' +
    'working directory with `git rev-parse --show-toplevel`, and a corpus nested inside ' +
    `another git repository resolves to the outer one. ${REPIN}`
  )
}

/**
 * Why the act tier is not available here, keyed off the server's own handshake.
 *
 * Three states and three repairs, and collapsing them would send a person to the wrong one.
 * A binary that predates the tier answers `initialize` with no `act` key at all, and the fix
 * is a re-pin; a corpus on a current binary that has not opted in answers `false`, and the
 * fix is one key in a file it owns. Neither is a defect: RFC-0029 §2.1 makes the declaration
 * configuration, never inference, so a corpus that has not said it may be written to by a
 * process is exactly as it should be until somebody says so.
 *
 * Returns null when the tier is declared, so the page shows nothing in the case that works.
 */
export function describeUndeclared(act: 'declared' | 'undeclared' | 'predates-act'): string | null {
  switch (act) {
    case 'declared':
      return null
    case 'undeclared':
      return (
        'This corpus has not declared the act tier, so nothing here may write to it. ' +
        'That is configuration and never inference (RFC-0029 §2.1): to let this surface draft ' +
        'proposal branches, add `[serve]\\nact = true` to `.yidam/config.toml` in the checkout ' +
        'and reload. A checkout with no git author identity is refused at startup rather ' +
        'than downgraded.'
      )
    case 'predates-act':
      return (
        'This yidam predates the act tier (MCP contract 0.22.0, cli/v0.13.0): its handshake ' +
        `carries no \`act\` capability at all. Writes are unavailable. ${REPIN}`
      )
  }
}
