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
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix))
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
