/**
 * Reading the report contract, and refusing to guess when it does not fit.
 *
 * The extension is versioned independently of the binary a repository pins in
 * `.yidam.toml`, so skew is normal rather than exceptional: a contributor opens a corpus
 * pinned six months back and the editor must say so instead of mis-reading it.
 *
 * The rule this file enforces, from RFC-0016: **never mis-parse, never guess.** An unknown
 * major version disables every verdict feature and states why. Degrading loudly is the
 * whole contract — a consumer that best-effort-parses an envelope it does not understand
 * is a consumer that reports wrong verdicts confidently, which is worse than reporting
 * none.
 */

/** The contract major this build of the extension understands. */
export const SUPPORTED_FORMAT_VERSION = '1'

export interface YidamBlock {
  version: string
  commit: string
  features: string[]
}

export type Handshake =
  | { ok: true; format: string; yidam: YidamBlock }
  | { ok: false; kind: HandshakeFailure; message: string }

export type HandshakeFailure =
  /** Output was not JSON at all — almost always a binary predating `--format`. */
  | 'not-json'
  /** JSON, but not a report envelope. */
  | 'not-an-envelope'
  /** A contract this build does not understand. */
  | 'unsupported-version'

/**
 * Inspect the envelope of any `yidam <command> --format json` run.
 *
 * Takes the raw stdout rather than a parsed object so that "the binary printed prose"
 * — the single most likely failure, and the one a naive `JSON.parse` turns into a stack
 * trace — is a named state with an actionable message.
 */
const PREDATES =
  'This yidam does not speak the JSON report contract — it predates `--format json` ' +
  '(RFC-0016 Phase 0). Verdict features are disabled. Re-pin with ' +
  '`mise run yidam-vendor-update`, then rebuild with `mise run yidam-build`.'

/** Clap's rejection of a flag the binary has never heard of. */
function looksLikeUnknownFormatFlag(stderr: string): boolean {
  return /unexpected argument .*--format/.test(stderr) || /unknown flag: --format/.test(stderr)
}

export function readHandshake(stdout: string, stderr = ''): Handshake {
  // A binary predating the contract has two shapes, and only one of them was obvious.
  //
  // It may print prose where JSON was asked for — the case a naive `JSON.parse` turns
  // into a stack trace. But clap REJECTS an unknown flag outright: no stdout at all, a
  // usage message on stderr, and a nonzero exit. That is what a stale `yidam` on a
  // developer's PATH actually does, and it is the more common of the two, because the
  // flag is newer than the commands it was added to.
  //
  // Both mean the same thing to a user and get the same actionable message. Reporting
  // the second as "could not run yidam" would send them looking for a broken install
  // instead of a stale one.
  if (looksLikeUnknownFormatFlag(stderr)) {
    return { ok: false, kind: 'not-json', message: PREDATES }
  }

  let parsed: unknown
  try {
    parsed = JSON.parse(stdout)
  } catch {
    return { ok: false, kind: 'not-json', message: PREDATES }
  }

  if (typeof parsed !== 'object' || parsed === null) {
    return { ok: false, kind: 'not-an-envelope', message: 'yidam returned JSON that is not a report envelope.' }
  }
  const obj = parsed as Record<string, unknown>
  const format = obj.format_version
  const yidam = obj.yidam as YidamBlock | undefined
  if (typeof format !== 'string' || typeof yidam !== 'object' || yidam === null) {
    return {
      ok: false,
      kind: 'not-an-envelope',
      message: 'yidam returned JSON without a `format_version` and `yidam` block.',
    }
  }

  // Compare the MAJOR only. Adding a field is not a breaking change and consumers are
  // required to ignore what they do not know, so a future 1.x must keep working here —
  // otherwise every additive change to the contract would strand every older editor.
  if (major(format) !== major(SUPPORTED_FORMAT_VERSION)) {
    return {
      ok: false,
      kind: 'unsupported-version',
      message:
        `This yidam speaks report contract ${format}; this extension understands ` +
        `${SUPPORTED_FORMAT_VERSION}. Verdict features are disabled rather than guessed at. ` +
        'Update the extension, or re-pin the binary with `mise run yidam-vendor-update`.',
    }
  }

  return { ok: true, format, yidam }
}

function major(v: string): string {
  return v.split('.')[0]
}

/** Whether a feature-gated command is available in the binary that answered. */
export function hasFeature(h: Handshake, feature: string): boolean {
  return h.ok && h.yidam.features.includes(feature)
}

/** One line for the status bar. Says which binary, so the user can reproduce it. */
export function describe(h: Handshake): string {
  if (!h.ok) return 'yidam: unavailable'
  return `yidam ${h.yidam.version} (${h.yidam.commit})`
}
