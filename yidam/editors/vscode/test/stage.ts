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
      fs.writeFileSync(file, write.content)
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
 */
export function captureStreams(
  bin: string,
  args: string[],
  cwd: string,
): { stdout: string; stderr: string } {
  try {
    return { stdout: execFileSync(bin, args, { cwd, encoding: 'utf8' }), stderr: '' }
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
 * So the probe is compared against what the fixture says it should produce. `status` is
 * already being run for the handshake and its answer thrown away; `expected/status.json` is
 * already committed. Comparison is structural rather than textual, because a textual one
 * would need `redact()`'s rules and a third transcription of those is the thing this
 * repository keeps having to undo.
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

  const { stdout, stderr } = captureStreams(r.command, ['status', '--format', 'json'], cwd)
  const h = readHandshake(stdout, stderr)
  if (!h.ok) {
    return refuse(
      `${r.command} does not speak the report contract: ${h.ok === false ? h.message : ''}`,
    )
  }

  const golden = fs.readFileSync(path.join(FIXTURE_DIR, 'expected/status.json'), 'utf8')
  const got = payload(stdout)
  const want = payload(golden)
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    return refuse(
      `${r.command} does not reproduce this fixture's committed goldens — it is stale.\n` +
        `  expected ${JSON.stringify(want)}\n` +
        `  got      ${JSON.stringify(got)}\n` +
        '  rebuild it: cargo install --path yidam/cli',
    )
  }
  return r.command
}
