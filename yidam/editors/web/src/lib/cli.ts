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
       * Not the same question as the root that was asked for, and the difference is not
       * hypothetical. **No subcommand takes a `--root` flag**: the corpus is resolved by
       * `git rev-parse --show-toplevel` from the working directory (`paths.rs`), so pointing
       * this surface at a corpus nested inside another git repository silently answers about
       * the outer one. `examples/streamflow` is exactly that case, and `main.rs` says so.
       *
       * A zero-node page is what that looks like from the browser, and a zero-node page is
       * indistinguishable from an empty corpus. So the difference is carried rather than
       * dropped, and the shell says it out loud.
       */
      resolvedRoot: string | null
    }
  | { ok: false; handshake: Handshake }

export interface SpawnInput {
  /** Absolute path to the binary `resolveBinary` found. */
  command: string
  /** The corpus root, already resolved. */
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
  let stdout = ''
  let stderr = ''
  try {
    const out = await exec(input.command, [command, '--format', 'json'], { cwd: input.root })
    stdout = out.stdout
    stderr = out.stderr
  } catch (e) {
    // A nonzero exit is normal: `lint` exits nonzero when the gate fails and still prints a
    // perfectly good envelope on stdout. Only a run that produced no readable envelope is a
    // failure here, and `readHandshake` is what decides that — including the case clap
    // rejects `--format` outright, which is a stale binary rather than a broken one.
    const err = e as { stdout?: string; stderr?: string }
    stdout = err.stdout ?? ''
    stderr = err.stderr ?? ''
  }

  const handshake = readHandshake(stdout, stderr)
  if (!handshake.ok) {
    return { ok: false, handshake }
  }
  const report = JSON.parse(stdout) as T
  const envelopeRoot = (report as { root?: unknown }).root
  return {
    ok: true,
    handshake,
    report,
    resolvedRoot: typeof envelopeRoot === 'string' ? envelopeRoot : null,
  }
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
