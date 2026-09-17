/**
 * Turning the reports fixture into the repository it describes.
 *
 * The reports cannot run against a bare directory — `repo_root()` shells out to
 * `git rev-parse --show-toplevel` — so every test that uses the fixture has to build a git
 * repository out of it first. Six files here did, in six copies, and the Rust golden harness
 * did too, in a seventh. They did not agree: the goldens staged three commits and two
 * branches, five files here staged one commit and no branch, and `tree.test.ts` staged one
 * commit and two branches because the phases view needed them.
 *
 * So `expected/` described a repository the extension was never exercised on, while both
 * sides read it as though it described the same one. The recipe now lives beside the fixture
 * in `stage.toml` and this reads it, as does `report_goldens.rs`.
 */

import { execFileSync } from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

import { parse } from 'smol-toml'

import { resolveBinary } from '../src/binary.ts'
import { readHandshake } from '../src/handshake.ts'

const HERE = path.dirname(new URL(import.meta.url).pathname)

/** The fixture root — `repo/` beside `stage.toml`. */
export const FIXTURE_DIR = path.resolve(
  HERE,
  '../../../prelude/sdks/parity/fixtures/reports/basic',
)

interface Edit {
  file: string
  from: string
  to: string
}

interface Write {
  file: string
  content: string
}

interface Recipe {
  branches: string[]
  commits: { message: string; replace?: Edit[]; write?: Write[] }[]
}

/**
 * A throwaway repository staged from the fixture, in a fresh tempdir.
 *
 * `prefix` names the tempdir only, so a failure is traceable to the file that staged it.
 */
export function stageFixture(prefix = 'yidam-ext-'): string {
  return stageInto(fs.mkdtempSync(path.join(os.tmpdir(), prefix)))
}

/**
 * Resolve `{{commit:N}}` in a written file to the sha of this recipe's Nth commit, 1-indexed.
 *
 * One file in the fixture has to name a commit: `.yidam/lint-baseline.yml`, whose `since:` is
 * read against the corpus history and decides whether an entry has expired. A sha cannot be a
 * literal in `repo/`, because a commit's sha is a function of the tree it holds — a baseline
 * shipped inside that tree would have to name itself. So the recipe writes it at a later
 * commit and refers to an earlier one by position.
 *
 * **Unresolvable throws rather than substituting nothing.** A `since` naming no commit reads
 * as an entry whose clock cannot be established, which the CLI reports as *not expired* — so
 * a silent failure here would take the arm the fixture exists to reach with it, and every
 * assertion downstream would go on passing.
 *
 * `report_goldens.rs::resolve_commits` is the other half of this; the two must agree, and the
 * fixture's expired entry is what says so — it is absent from this side's report the moment
 * they do not.
 */
function resolveCommits(content: string, shas: string[], file: string): string {
  return content.replace(/\{\{commit:(\d+)\}\}/g, (_match, n: string) => {
    const sha = shas[Number(n) - 1]
    if (sha === undefined) {
      throw new Error(
        `stage.toml: ${file} names commit ${n} and only ${shas.length} have been made — ` +
          'a recipe can only refer to a commit that already exists',
      )
    }
    return sha
  })
}

/**
 * The same repository, at a path you choose.
 *
 * For running the extension by hand: the launch configuration has to name a workspace, and
 * a `mkdtemp` path is different every time. Staging the *same* corpus the tests assert
 * against means what a person sees in the editor and what CI checks are one repository
 * rather than two that drift.
 *
 * `dir` is emptied first, so re-staging after an edit to the fixture is one command.
 */
export function stageInto(dir: string): string {
  fs.rmSync(dir, { recursive: true, force: true })
  fs.mkdirSync(dir, { recursive: true })
  fs.cpSync(path.join(FIXTURE_DIR, 'repo'), dir, { recursive: true })

  const recipe = parse(
    fs.readFileSync(path.join(FIXTURE_DIR, 'stage.toml'), 'utf8'),
  ) as unknown as Recipe

  const git = (...args: string[]) => execFileSync('git', args, { cwd: dir, stdio: 'pipe' })
  git('init', '-q', '-b', 'main')
  git('config', 'user.email', 'fixture@yidam.test')
  git('config', 'user.name', 'Fixture')

  // The shas this recipe has made so far, oldest first — what `{{commit:N}}` resolves
  // against. See `resolveCommits`.
  const shas: string[] = []

  for (const commit of recipe.commits) {
    // Edits first, then stage everything: a commit's `replace` and `write` describe the tree
    // as of that commit, not a change made after it.
    for (const edit of commit.replace ?? []) {
      const file = path.join(dir, edit.file)
      const text = fs.readFileSync(file, 'utf8')
      if (!text.includes(edit.from)) {
        throw new Error(`stage.toml: ${edit.file} does not contain ${JSON.stringify(edit.from)}`)
      }
      fs.writeFileSync(file, text.replaceAll(edit.from, edit.to))
    }
    for (const write of commit.write ?? []) {
      const file = path.join(dir, write.file)
      fs.mkdirSync(path.dirname(file), { recursive: true })
      fs.writeFileSync(file, resolveCommits(write.content, shas, write.file))
    }
    git('add', '-A')
    // Fixed dates keep `status`'s genesis field stable across runs.
    execFileSync('git', ['commit', '-q', '-m', commit.message], {
      cwd: dir,
      stdio: 'pipe',
      env: {
        ...process.env,
        GIT_AUTHOR_DATE: '2026-01-01T00:00:00Z',
        GIT_COMMITTER_DATE: '2026-01-01T00:00:00Z',
      },
    })
    const sha = git('rev-parse', 'HEAD').toString().trim()
    // Empty would substitute as an empty `since`, which reads as "never expires" — the one
    // failure this whole mechanism exists to make impossible.
    if (!sha) throw new Error(`stage.toml: no HEAD after committing ${commit.message}`)
    shas.push(sha)
  }

  for (const branch of recipe.branches) git('branch', branch)

  return dir
}

// ── Is this binary one whose answers mean anything here? ─────────────────────

export const SKIP =
  'no yidam speaking the report contract — set YIDAM_BIN, or `cargo install --path yidam/cli`'

/**
 * Run and keep both streams whatever the exit code.
 *
 * `lint` and `graph-check` gate — a nonzero exit is a verdict, not a failure to produce one,
 * and the envelope is on stdout regardless. A caller that treated exit != 0 as "binary
 * unusable" would go blind exactly when the corpus needs attention.
 *
 * `stdio` is given explicitly because `execFileSync` inherits stderr by default: a gating
 * command's summary line went to the test runner's own output and arrived here as `''`, so
 * the one case that needs stderr — a binary predating `--format json`, which writes clap's
 * usage there and nothing to stdout — reached `readHandshake` with the evidence missing.
 */
export function captureStreams(
  bin: string,
  args: string[],
  cwd: string,
): { stdout: string; stderr: string } {
  try {
    const stdio: ('ignore' | 'pipe')[] = ['ignore', 'pipe', 'pipe']
    return { stdout: execFileSync(bin, args, { cwd, encoding: 'utf8', stdio }), stderr: '' }
  } catch (err) {
    const e = err as { stdout?: string; stderr?: string }
    return { stdout: e.stdout ?? '', stderr: e.stderr ?? '' }
  }
}

/** Stdout alone, which is all a report's reader needs. */
export function capture(bin: string, args: string[], cwd: string): string {
  return captureStreams(bin, args, cwd).stdout
}

/** The report minus the fields that belong to the run rather than to the corpus. */
function payload(report: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(report) as Record<string, unknown>
    // `yidam` is version, build commit and feature set; `root` is an absolute path. Both
    // vary by machine. Everything else — `format_version` included, which is a contract
    // check worth keeping — is a statement about the fixture.
    delete parsed.yidam
    delete parsed.root
    return parsed
  } catch {
    return null
  }
}

/** What of a report is compared: one member of the payload, or the whole of it. */
function compared(report: string, field: string | undefined): unknown {
  const p = payload(report)
  if (p === null || field === undefined) return p
  return p[field]
}

/** One report the probe certifies a binary against. */
export interface ProbeSpec {
  /** Named in the refusal, so a reader knows which report disagreed. */
  report: string
  args: string[]
  /** Path under [`FIXTURE_DIR`]. */
  golden: string
  /**
   * The member of the payload compared, or the whole payload when absent.
   *
   * `lint`'s golden carries an `age.first_commit` of `<FIRST_COMMIT>`, so comparing all of
   * it would need `redact()`'s rules and a third transcription of those is the thing this
   * repository keeps having to undo. Its `gate` needs no redaction, and the gate is what
   * the tests downstream read.
   */
  field?: string
}

/**
 * The reports a binary has to agree about before its answers here mean anything.
 *
 * `status` alone was the whole probe until #752. It reports node, question, catalog and
 * claim counts and says nothing about which *checks* the binary carries, so a yidam
 * predating a lint check resolved, passed, and then answered wrongly about `lint` — which
 * is what nine of the seventeen `contractBinary` call sites go on to read. 0.9.0 on `PATH`
 * did exactly that: it does not carry `resolution-elector-unregistered` at all, so the
 * baseline entry for it read as stale rather than as inherited debt, and the failure
 * surfaced as `1 !== 2` three assertions downstream (#657) — the confusing failure about
 * someone else's work this probe exists to prevent, one report over.
 *
 * **The cost is one extra `lint` run**, measured on this fixture at 268–309 ms for six of
 * seven runs of a debug build, with a cold outlier at 977 ms — and there are seventeen call
 * sites, each staging a fresh repository. That is why the verdict is memoized per binary path in
 * [`contractBinary`]: a verdict is a property of the binary rather than of the tempdir it
 * was reached in, and `node --test` gives each file its own process, so the seventeen calls
 * across six files cost six runs rather than seventeen. Checking *less* is not the
 * alternative — that is what this issue was.
 */
export const PROBES: ProbeSpec[] = [
  { report: 'status', args: ['status', '--format', 'json'], golden: 'expected/status.json' },
  {
    report: 'lint',
    args: ['lint', '--format', 'json'],
    golden: 'expected/lint.json',
    field: 'gate',
  },
]

/**
 * Why this binary's reports disagree with the fixture, or `null` when they do not.
 *
 * Pure over the captured stdout, against the committed goldens, so the refusal arm can be
 * exercised without a stale yidam on hand — see `contract.test.ts`. A guard whose refusal
 * has never been reached is a guard that may already have stopped guarding.
 *
 * Comparison is structural rather than textual, because a textual one would need
 * `redact()`'s rules. See [`ProbeSpec.field`].
 */
export function contractRefusal(bin: string, outputs: Record<string, string>): string | null {
  for (const spec of PROBES) {
    const golden = fs.readFileSync(path.join(FIXTURE_DIR, spec.golden), 'utf8')
    const got = compared(outputs[spec.report] ?? '', spec.field)
    const want = compared(golden, spec.field)
    if (JSON.stringify(got) === JSON.stringify(want)) continue
    return (
      `${bin} does not reproduce this fixture's committed \`${spec.report}\` golden — it is ` +
      'stale relative to the fixture.\n' +
      `  report:   yidam ${spec.args.join(' ')}\n` +
      `  compared: ${spec.field === undefined ? 'the whole payload' : `\`${spec.field}\``}\n` +
      `  expected ${JSON.stringify(want)}\n` +
      `  got      ${JSON.stringify(got)}\n` +
      '  rebuild it: cargo install --path yidam/cli'
    )
  }
  return null
}

/**
 * One verdict per binary path: the reason it was refused, or `null` for accepted.
 *
 * Keyed on the *resolved* path rather than on `YIDAM_BIN`, so two workspaces resolving to
 * the same binary share the answer and two resolving differently do not. See [`PROBES`]
 * for why this is worth memoizing at all.
 */
const verdicts = new Map<string, string | null>()

/**
 * A binary whose answers about this fixture mean something, or `null`.
 *
 * Resolution is not enough, and neither is the handshake. A yidam that resolves and emits a
 * well-formed envelope can still predate a *corpus* feature and answer wrongly: one that
 * predated the structural claim tag reported one open question where the fixture has two,
 * and the failure surfaced three assertions downstream as `1 !== 2` — a confusing failure
 * about someone else's work, which is exactly what the handshake check was written to
 * prevent one layer further out.
 *
 * So the probe runs each of [`PROBES`] and compares it against what the fixture says it
 * should produce, and the refusal names the report that disagreed — `status` alone was not
 * enough, because the reports these tests read are mostly `lint` (#752).
 *
 * **What this cannot detect** is a binary *newer* than the fixture in a way that changes
 * output. That fails the Rust goldens first, which is where it belongs.
 */
export async function contractBinary(cwd: string): Promise<string | null> {
  const required = (process.env.YIDAM_REQUIRE_CONTRACT ?? '') !== ''
  const refuse = (why: string): null => {
    if (required) throw new Error(`YIDAM_REQUIRE_CONTRACT is set and ${why}`)
    return null
  }

  const r = await resolveBinary({ configured: process.env.YIDAM_BIN ?? '', workspace: cwd })
  if (!r.command) return refuse(`no yidam resolved: ${r.reason}`)
  const bin = r.command

  // A remembered refusal is re-raised rather than re-derived, so `YIDAM_REQUIRE_CONTRACT`
  // still throws at every call site and a run under it fails where it would have before.
  const remembered = verdicts.get(bin)
  if (remembered !== undefined) return remembered === null ? bin : refuse(remembered)

  const decide = (why: string | null): string | null => {
    verdicts.set(bin, why)
    return why === null ? bin : refuse(why)
  }

  const outputs: Record<string, string> = {}
  for (const spec of PROBES) {
    const { stdout, stderr } = captureStreams(bin, spec.args, cwd)
    // Every probed report has to be a readable envelope, not just the first: a binary
    // predating `lint --format json` fails here rather than parsing as nothing and
    // comparing unequal, which would send a reader to rebuild over a usage message.
    const h = readHandshake(stdout, stderr)
    if (!h.ok) {
      return decide(
        `${bin} does not speak the report contract in \`${spec.report}\`: ` +
          `${h.ok === false ? h.message : ''}`,
      )
    }
    outputs[spec.report] = stdout
  }
  return decide(contractRefusal(bin, outputs))
}
