/**
 * Pack the tarball npm would publish, install it somewhere else, and start it.
 *
 * `npm publish` copies only what lives under the package root. A module imported from
 * outside it — `../../../design/lib/whatever.js` from `src/` — resolves in the working tree
 * and in every CI job, because the tree is right there, and is simply **absent from the
 * tarball**. Nothing runs that tarball until a stranger types `npx`.
 *
 * That is not a hypothetical here. It is `cargo package`'s failure with the extension
 * changed, and this repository has paid for it twice: `cli/v0.3.0` was one command from
 * shipping five platform binaries, a GitHub release and a Homebrew formula all naming a
 * version `cargo install` could not install, and `cli/v0.7.0` would have done the same over
 * seven `.rego` files. [`packaging.rs`](../../../cli/tests/packaging.rs) is where that rule
 * lives for the crate. This is the same rule one ecosystem over.
 *
 * ── why it installs rather than reading the file list ────────────────────────
 *
 * A list of entries answers "is the file in the tarball". It does not answer "does the thing
 * start", and the two come apart in a second way this ecosystem has that cargo does not: a
 * module that is genuinely inside the package root can still import a package that is only a
 * **devDependency**. The tarball carries no `node_modules`, so what a consumer gets is
 * whatever `dependencies` resolves — and the failure is an `ERR_MODULE_NOT_FOUND` naming a
 * package that is right there on the machine it was tested on.
 *
 * So the tarball is unpacked into a directory outside this repository, its *production*
 * dependencies are installed there, and the server is started from it. Both failures are the
 * same failure at that point: the process does not come up.
 *
 * `npm ci` against a copy of this package's lockfile, rather than `npm install` against the
 * ranges. The lockfile is not in `files` and a consumer never sees it, so this is the test
 * arranging the install — deliberately, because the alternative is resolving fresh ranges on
 * every run and a gate that goes red for a reason that is not about this repository is a gate
 * people learn to re-run rather than read.
 *
 * ── why it runs on pull requests, not first at the tag ───────────────────────
 *
 * #871. `ci (vscode)` never packaged, so two `vsce` refusals stayed green for weeks and both
 * arrived during a release, which is the one moment at which finding out is no longer free.
 * `ci-editor-web` runs this; `release.sh` runs it again before it will cut `edit/v*`.
 */

import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { spawn } from 'node:child_process'
import * as fs from 'node:fs'
import net from 'node:net'
import * as os from 'node:os'
import * as path from 'node:path'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const ROOT = path.resolve(HERE, '..')
const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'))

/**
 * Files the tarball must contain, because each one's absence is invisible until `npx` has
 * already fetched it. `bin/` ships unbundled — it is the code that runs *before* anything is
 * bundled — so each of its three modules is named rather than covered by a pattern.
 */
const REQUIRED = [
  'package.json',
  'README.md',
  'bin/yidam-edit.mjs',
  'bin/args.mjs',
  'bin/listen.mjs',
  'dist/server/entry.mjs',
]

/** Everything else that is allowed: the build output, and nothing that is not it. */
const ALLOWED = [/^dist\//]

/** Named, so the failure says what went wrong rather than only that something did. */
const FORBIDDEN = [
  [/^src\//, 'TypeScript and Astro sources — the build already emitted these to dist/'],
  [/^test\//, 'the test suite, which no installer runs'],
  [/^scripts\//, 'development scripts, including this one'],
  [/^node_modules\//, 'dependencies — npm installs these from package.json'],
  [/^\.astro\//, "Astro's build cache"],
  [/^package-lock\.json$/, 'the lockfile, which a consumer never resolves against'],
  [/^junit\.xml$/, 'a test report'],
  [
    /^(astro\.config|tsconfig|\.oxlintrc)/,
    'build configuration that runs nowhere after packaging',
  ],
]

const problems = []

// ── the build has to have happened ──────────────────────────────────────────
//
// `files` names `dist`, and npm packs a named directory that does not exist by saying
// nothing at all. Failing here rather than at the REQUIRED check below is the difference
// between "run npm run build" and "dist/server/entry.mjs is missing", which reads as a bug.
if (!fs.existsSync(path.join(ROOT, 'dist', 'server', 'entry.mjs'))) {
  console.error('\nno server build at dist/server/entry.mjs — run `npm run build` first.\n')
  process.exit(1)
}

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'yidam-edit-pack-'))
const packed = JSON.parse(
  execFileSync('npm', ['pack', '--json', '--pack-destination', work], {
    cwd: ROOT,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  }),
)
const tarball = path.join(work, packed[0].filename)

// npm writes every entry under a `package/` prefix, which is the tar convention and not a
// path in this package.
const entries = execFileSync('tar', ['-tzf', tarball], { encoding: 'utf8' })
  .split('\n')
  .filter(Boolean)
  .filter((e) => !e.endsWith('/'))
  .map((e) => e.replace(/^package\//, ''))

for (const entry of entries) {
  const forbidden = FORBIDDEN.find(([re]) => re.test(entry))
  if (forbidden) {
    problems.push(`  ${entry}\n      ${forbidden[1]}`)
    continue
  }
  if (REQUIRED.includes(entry)) continue
  if (ALLOWED.some((re) => re.test(entry))) continue
  problems.push(
    `  ${entry}\n      not in the runtime set. Narrow \`files\` in package.json, or — if it ` +
      `genuinely ships — add it to REQUIRED or ALLOWED in this script.`,
  )
}

for (const required of REQUIRED) {
  if (!entries.includes(required)) problems.push(`  ${required}\n      required, and absent`)
}

if (problems.length > 0) {
  console.error(`\nthe packed tarball is not the runtime set:\n\n${problems.join('\n')}\n`)
  console.error(`${entries.length} entries in ${tarball}\n`)
  process.exit(1)
}

console.log(`packed: ${entries.length} files, ${(fs.statSync(tarball).size / 1024).toFixed(1)} KB`)

// ── install it somewhere this repository is not ─────────────────────────────
const install = path.join(work, 'install')
fs.mkdirSync(install)
execFileSync('tar', ['-xzf', tarball, '-C', install, '--strip-components=1'], {
  stdio: 'inherit',
})
fs.copyFileSync(path.join(ROOT, 'package-lock.json'), path.join(install, 'package-lock.json'))
execFileSync(
  'npm',
  ['ci', '--omit=dev', '--ignore-scripts', '--no-audit', '--no-fund'],
  { cwd: install, stdio: 'inherit' },
)

/**
 * A port nothing is on, taken and released.
 *
 * Narrows a window rather than closing one, for the reason `bin/listen.mjs` documents at
 * length: the adapter binds during module evaluation and accepts no already-listening
 * handle, so the only way to hand it a port is to let go of one first.
 */
const port = await new Promise((resolve, reject) => {
  const probe = net.createServer()
  probe.once('error', reject)
  probe.listen(0, '127.0.0.1', () => {
    const { port: got } = probe.address()
    probe.close(() => resolve(got))
  })
})

// `--root work`, which is a directory with no corpus and no `yidam` on any path under it.
// That is the point: this asserts the server *starts and answers*, which is the thing a
// missing module takes away. What it renders is `index.astro`'s "no status to show" arm,
// and the pages that need a binary are covered by the unit tests against a real one.
const server = spawn(process.execPath, ['bin/yidam-edit.mjs', '--root', work, '--port', String(port), '--no-open'], {
  cwd: install,
  stdio: ['ignore', 'pipe', 'pipe'],
})
let log = ''
server.stdout.on('data', (d) => {
  log += d
})
server.stderr.on('data', (d) => {
  log += d
})

const died = new Promise((resolve) => server.once('exit', (code) => resolve(code ?? 'signal')))

/** Poll rather than parse the banner: the URL is printed before the adapter has bound. */
async function reach(url) {
  const deadline = Date.now() + 20_000
  for (;;) {
    if (server.exitCode !== null) return null
    try {
      return await fetch(url)
    } catch {
      if (Date.now() > deadline) return null
      await new Promise((r) => setTimeout(r, 200))
    }
  }
}

const response = await reach(`http://127.0.0.1:${port}/`)
if (response === null) {
  const how = server.exitCode === null ? 'never answered' : `exited ${await died}`
  console.error(
    `\nthe packed tarball does not run: the server ${how}.\n\n` +
      'This is the failure this check exists for. A module imported from outside the package\n' +
      'root, or a runtime import that is only a devDependency, resolves in the working tree\n' +
      'and is absent from what a consumer installs.\n\n' +
      `${log}\n`,
  )
  server.kill('SIGKILL')
  process.exit(1)
}

const body = await response.text()
server.kill('SIGTERM')

assert.equal(
  response.status,
  200,
  `the packed tarball answered ${response.status} on / — it starts, and does not serve.\n${log}`,
)
assert.match(
  body,
  /<\/html>/,
  `the packed tarball served no page on /.\n${body.slice(0, 400)}`,
)

console.log(`ran: ${install} answered 200 on / with no corpus and no binary`)

// ── the bytes that were graded are the bytes that publish ────────────────────
//
// `edit.yml` sets this, and its publish job runs `npm publish <that file>` rather than
// packing again from source. editor.yml passes `--packagePath` to both registry tools for
// the same reason: a tool that repackages publishes an artifact nothing inspected, and the
// gate above then describes a tarball nobody shipped.
const keep = process.env.YIDAM_PACK_OUT
if (keep) {
  fs.mkdirSync(path.dirname(keep), { recursive: true })
  fs.copyFileSync(tarball, keep)
  console.log(`kept: ${keep}`)
}

// The tag and the manifest must agree, and this script is what the publish workflow runs
// before it reaches npm. `release.sh` checks it too; the two entrances into a release are
// exactly where a version drifts.
const tag = process.env.GITHUB_REF_NAME ?? ''
if (tag.startsWith('edit/v')) {
  assert.equal(
    manifest.version,
    tag.slice('edit/v'.length),
    `tag ${tag} does not match package.json version ${manifest.version}`,
  )
  console.log(`tag ${tag} matches the manifest`)
}

fs.rmSync(work, { recursive: true, force: true })
