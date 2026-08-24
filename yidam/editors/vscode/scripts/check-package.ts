/**
 * Package the extension, and assert that what came out is the runtime set and nothing else.
 *
 * `.vscodeignore` is an exclusion list, and an exclusion list has the failure mode every
 * exclusion list has: the thing it forgot looks exactly like the thing it excluded. Both
 * are "a file that is not mentioned". Before this existed, the default — no `.vscodeignore`
 * at all — packaged 66 files including `src/`, `test/`, `eslint.config.mjs` and `mise.toml`,
 * and `vsce package` reported success each time.
 *
 * So the rule is inverted here into an allowlist. A new file in the package fails this
 * check whether or not anyone thought to exclude it, which is the only ordering that
 * survives someone adding a directory a year from now.
 *
 * Running `vsce package` rather than `vsce ls` is deliberate. `ls` answers a question about
 * intent; packaging is the operation that actually happens on the tag, and it is the one
 * that runs `vscode:prepublish` and can fail on a manifest vsce will not accept. The
 * repository already learned this from the CLI: `cargo publish --dry-run` is in `release.sh`
 * because a green CI run is not evidence that the shipped artifact builds.
 */

import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const ROOT = path.resolve(HERE, '..')

/**
 * Files the package must contain. Each is here because its absence is invisible until a
 * stranger has already installed: a missing LICENSE is a legal question, a missing icon is
 * a grey square, a missing CHANGELOG is a tab that is not there.
 *
 * The names are vsce's, not the tree's. It lowercases `README.md` and `CHANGELOG.md` and
 * appends `.txt` to `LICENSE` on the way in — the Marketplace looks for those exact spellings
 * and would render none of the three otherwise. Asserting the tree's spellings instead would
 * be asserting a file this check never sees.
 */
const REQUIRED = [
  'package.json',
  'readme.md',
  'LICENSE.txt',
  'changelog.md',
  'resources/icon.png',
  'resources/yidam.svg', // the activity-bar icon the manifest names
  'out/extension.js', // what `main` points at
]

/**
 * Everything else that is allowed. `out/**` is the compiled extension; the pattern is open
 * because adding a module is ordinary and adding a directory is not.
 */
const ALLOWED = [/^out\/[\w./-]+\.js$/]

/** Named so the failure says what went wrong rather than only that something did. */
const FORBIDDEN: [RegExp, string][] = [
  [/^src\//, 'TypeScript sources — the build already emitted these to out/'],
  [/^test\//, 'the test suite, which no installer runs'],
  [/^scripts\//, 'development scripts, including this one'],
  [/\.js\.map$/, 'a source map whose sources are not shipped'],
  [/^tsconfig|^eslint|^mise\.toml$/, 'build configuration that runs nowhere after packaging'],
  [/^node_modules\//, 'dependencies — this extension bundles none at runtime'],
  [/^\.git|^\.vscodeignore$/, 'repository metadata'],
]

/**
 * Where to write it. The publish workflow passes a path because it uploads the exact bytes
 * this script checked — packaging again downstream would publish an artifact nothing
 * inspected, which is the whole failure mode being closed here. A person running it by hand
 * passes nothing and gets a temp file, because the answer they want is pass or fail.
 */
const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'))
const out = process.argv[2]
  ? path.resolve(process.argv[2])
  : path.join(
      fs.mkdtempSync(path.join(os.tmpdir(), 'yidam-vsix-')),
      `yidam-vscode-${manifest.version}.vsix`,
    )
fs.mkdirSync(path.dirname(out), { recursive: true })

// `--allow-missing-repository` is deliberately NOT passed: the manifest declares one, and
// if that ever stops being true this should fail rather than be waved through.
execFileSync('npx', ['vsce', 'package', '--out', out], { cwd: ROOT, stdio: 'inherit' })

// A .vsix is a zip. `unzip -Z1` lists it without unpacking, and is present on every runner
// this workflow uses; a zip reader in node would be a dependency for one line.
const entries = execFileSync('unzip', ['-Z1', out], { encoding: 'utf8' })
  .split('\n')
  .filter(Boolean)
  // The OPC wrapper, which vsce writes and neither of us chose.
  .filter((e) => e !== '[Content_Types].xml' && e !== 'extension.vsixmanifest')
  .map((e) => e.replace(/^extension\//, ''))

const problems: string[] = []

for (const entry of entries) {
  const forbidden = FORBIDDEN.find(([re]) => re.test(entry))
  if (forbidden) {
    problems.push(`  ${entry}\n      ${forbidden[1]}`)
    continue
  }
  if (REQUIRED.includes(entry)) continue
  if (ALLOWED.some((re) => re.test(entry))) continue
  problems.push(
    `  ${entry}\n      not in the runtime set. Exclude it in .vscodeignore, or — if it ` +
      `genuinely ships — add it to REQUIRED or ALLOWED in this script.`,
  )
}

for (const required of REQUIRED) {
  if (!entries.includes(required)) problems.push(`  ${required}\n      required, and absent`)
}

if (problems.length > 0) {
  console.error(`\nthe packaged extension is not the runtime set:\n\n${problems.join('\n')}\n`)
  console.error(`${entries.length} entries in ${out}\n`)
  process.exit(1)
}

const bytes = fs.statSync(out).size
console.log(`\npackaged: ${entries.length} files, ${(bytes / 1024).toFixed(1)} KB — the runtime set`)

// The tag and the manifest must agree, and this script is what the publish workflow runs
// before it reaches the registries. `release.sh` checks this too; the check is cheap and
// the two entrances into a release are exactly where a version drifts.
const tag = process.env.GITHUB_REF_NAME ?? ''
if (tag.startsWith('editor/v')) {
  assert.equal(
    manifest.version,
    tag.slice('editor/v'.length),
    `tag ${tag} does not match package.json version ${manifest.version}`,
  )
  console.log(`tag ${tag} matches the manifest`)
}

console.log(out)
