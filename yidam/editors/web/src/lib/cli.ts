/**
 * The only place a verdict enters this process.
 *
 * RFC-0016's rule is **TypeScript computes affordances; the CLI computes verdicts.** Under
 * the shape RFC-0030 originally proposed that rule was unreachable rather than merely
 * forbidden: with no Node process anywhere, the JavaScript that shipped had nothing to
 * compute a verdict *from*. The reversal recorded in RFC-0030's 2026-09-05 amendment took
 * that property away — there is now a process that can hold a parsed corpus, and every check
 * in this repository is a pure function over exactly that.
 *
 * So the rule becomes a gate, and this file is where the gate points. `test/boundary.mjs`
 * asserts that a `violations` array reaches the rest of the app through `spawnReport` and
 * through nothing else, and that nothing under `src/` imports a corpus-evaluating module.
 * If a second route to a finding is ever added, that test is the thing that should go red.
 *
 * Never mis-parse, never guess: every run goes through `readHandshake` before its payload is
 * read, and an envelope this build does not understand disables the feature rather than
 * being best-effort parsed. That is `handshake.ts`'s contract, and this surface is held to it
 * for the same reason the extension is — the client is versioned independently of the binary
 * a repository pins, so skew is normal rather than exceptional.
 */

import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { readHandshake, type Handshake } from './handshake.ts'

const run = promisify(execFile)

/** Reports this surface reads. Each is a `yidam <command> --format json` run and nothing more. */
export type ReportCommand =
  | 'lint'
  | 'graph'
  | 'graph-check'
  | 'status'
  | 'open-questions'

export type ReportResult<T> =
  | {
      ok: true
      handshake: Handshake
      report: T
      /**
       * The root the binary actually resolved, off the envelope.
       *
       * Not the same question as the root that was asked for. Handed `--root`, the binary
       * walks up to the nearest `.yidam/` (`paths::resolve_root`), so a directory *inside* a
       * corpus answers for the corpus around it. Without the flag, it resolves the working
       * directory with `git rev-parse --show-toplevel`, which overshoots a corpus nested
       * inside another git repository — `examples/streamflow` is exactly that case — and
       * answers about the outer one with zero nodes.
       *
       * A zero-node page is indistinguishable from an empty corpus, so the difference is
       * carried rather than dropped, and the shell says it out loud.
       */
      resolvedRoot: string | null
      /**
       * How the binary was told which corpus: `flag` when it took `--root`, `working-directory`
       * when it predates the flag on this command (cli/v0.16.0, #918) and was asked again
       * without it. Only the second can overshoot a nested corpus, and the shell's wording
       * depends on which one answered.
       */
      resolvedBy: 'flag' | 'working-directory'
    }
  | {
      ok: false
      /**
       * Narrowed, because `ok: false` is *only* ever returned from the `!handshake.ok`
       * branch below — the failure arm carries a failed handshake by construction.
       *
       * Typing it as the whole `Handshake` union threw that away, and three call sites
       * reading `handshake.kind` inside `if (!result.ok)` were type errors that nothing
       * ran a type-checker over. They work at runtime; the type was the thing that was
       * wrong (#688).
       */
      handshake: Extract<Handshake, { ok: false }>
      /**
       * The binary's own refusal, when it printed one instead of an envelope.
       *
       * A named `--root` holding no corpus is refused (#1000): exit 1, nothing on stdout, and
       * `Error: not a yidam repository: …` on stderr. The handshake reads empty stdout as
       * `not-json` — a binary predating `--format json` — and its message would send the
       * reader to re-pin a binary that is current. The refusal names the actual repair, so it
       * is carried and shown instead. Null when stderr is anything else.
       */
      refusal: string | null
    }

export interface SpawnInput {
  /** Absolute path to the binary `resolveBinary` found. */
  command: string
  /**
   * The corpus root, already resolved. Passed as `--root` *and* as the working directory:
   * the flag is what a current binary reads, the working directory is what one predating it
   * reads, and the two cannot disagree when they are the same value.
   */
  root: string
  /** Injected so tests need no binary. */
  exec?: (
    command: string,
    args: string[],
    options: { cwd: string },
  ) => Promise<{ stdout: string; stderr: string }>
}

/**
 * One report, as the envelope the binary printed.
 *
 * Errors are values rather than throws: a page whose report is missing renders as
 * unavailable, and one failed report must not take the others with it. That is
 * `report-run.ts`'s discipline in the extension, and it is right for the same reason here.
 */
export async function spawnReport<T>(
  input: SpawnInput,
  command: ReportCommand,
): Promise<ReportResult<T>> {
  const exec = input.exec ?? defaultExec
  const args = [command, '--format', 'json']
  let resolvedBy: 'flag' | 'working-directory' = 'flag'
  // `--root` after `--format`: clap names the first argument it does not know, so a binary
  // predating both is still reported as predating `--format`, which is the older and the
  // more useful of the two answers.
  let { stdout, stderr } = await capture(exec, input.command, [...args, '--root', input.root], input.root)
  if (rejectsRootFlag(stderr)) {
    // A binary before cli/v0.16.0 takes `--root` on `serve` and nothing else. Skew is normal
    // here (`handshake.ts`), so the run is repeated the way every earlier version of this
    // surface made it, and `resolvedBy` says so rather than the page failing outright.
    ;({ stdout, stderr } = await capture(exec, input.command, args, input.root))
    resolvedBy = 'working-directory'
  }

  const handshake = readHandshake(stdout, stderr)
  if (!handshake.ok) {
    return { ok: false, handshake, refusal: stdout.trim() === '' ? refusalOf(stderr) : null }
  }
  const report = JSON.parse(stdout) as T
  const envelopeRoot = (report as { root?: unknown }).root
  return {
    ok: true,
    handshake,
    report,
    resolvedRoot: typeof envelopeRoot === 'string' ? envelopeRoot : null,
    resolvedBy,
  }
}

async function capture(
  exec: NonNullable<SpawnInput['exec']>,
  command: string,
  args: string[],
  cwd: string,
): Promise<{ stdout: string; stderr: string }> {
  try {
    return await exec(command, args, { cwd })
  } catch (e) {
    // A nonzero exit is normal: `lint` exits nonzero when the gate fails and still prints a
    // perfectly good envelope on stdout. Only a run that produced no readable envelope is a
    // failure here, and `readHandshake` is what decides that — including the case clap
    // rejects `--format` outright, which is a stale binary rather than a broken one.
    const err = e as { stdout?: string; stderr?: string }
    return { stdout: err.stdout ?? '', stderr: err.stderr ?? '' }
  }
}

/** Clap's rejection of `--root` by a binary that has the command and not the flag. */
function rejectsRootFlag(stderr: string): boolean {
  return /unexpected argument '--root'/.test(stderr)
}

/**
 * The binary's `Error: …` text, without the prefix, or null when stderr is not one.
 *
 * `anyhow` prints a failed `main` as `Error: ` and the chain beneath it, and that is the only
 * shape read here. A panic or a usage message is not a refusal, and passing it through as
 * one would put a stack trace where a sentence about the corpus belongs.
 */
export function refusalOf(stderr: string): string | null {
  const text = stderr.trim()
  return text.startsWith('Error: ') ? text.slice('Error: '.length) : null
}

const defaultExec = async (
  command: string,
  args: string[],
  options: { cwd: string },
): Promise<{ stdout: string; stderr: string }> => {
  // `maxBuffer` raised because a graph report on a real corpus is larger than the 1MB
  // default, and exceeding it fails the run with a message about buffers rather than about
  // the corpus — a failure mode that reads as a bug in yidam.
  const { stdout, stderr } = await run(command, args, { ...options, maxBuffer: 64 * 1024 * 1024 })
  return { stdout, stderr }
}
