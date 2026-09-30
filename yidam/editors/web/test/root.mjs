/**
 * The corpus that was asked for, and the one that answered.
 *
 * Every report command this surface runs takes `--root` since cli/v0.16.0 (#918), and
 * `spawnReport` hands it over. A binary given the flag walks up to the nearest `.yidam/`, so
 * a corpus nested inside another git repository — `examples/streamflow` in this one — is the
 * corpus that answers. Before #1012 the flag was not passed: the working directory was
 * resolved with `git rev-parse --show-toplevel`, which answers about the *outer* repository,
 * and that rendered as a corpus with no nodes and no indication anything was wrong.
 *
 * A pinned binary older than the flag still does exactly that, and pins are routinely older
 * than this package. So the run is repeated without the flag when clap rejects it, the result
 * says which way the corpus was resolved, and the mismatch message names the right repair.
 *
 * The last message test keeps the warning usable. `/tmp` is a symlink to `/private/tmp` on
 * macOS, so a naive string compare warns about a directory that is perfectly correct — and a
 * warning that cries wolf is a warning people learn to scroll past. `session.ts` canonicalises
 * before comparing; this asserts the message itself is a plain comparison, so the
 * canonicalisation cannot be quietly dropped and made up for here.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { spawnReport, refusalOf } from '../src/lib/cli.ts'
import { describeRootMismatch, describeUnavailable } from '../src/lib/messages.ts'

const ENVELOPE = (root) =>
  JSON.stringify({
    format_version: '1',
    yidam: { version: '0.16.0', commit: 'abc1234', features: [] },
    root,
  })

/** A clap rejection: nonzero exit, nothing on stdout, a usage message on stderr. */
function exited(code, stderr) {
  return Object.assign(new Error(`exit ${code}`), { code, stdout: '', stderr })
}

/** A fake binary. `answer` is called with each argv and returns stdout or throws. */
function binary(answer) {
  const calls = []
  const exec = async (command, args, options) => {
    calls.push({ args, cwd: options.cwd })
    return { stdout: answer(args), stderr: '' }
  }
  return { exec, calls }
}

const OLD_CLAP =
  "error: unexpected argument '--root' found\n\nUsage: yidam status --format <FORMAT>\n\n" +
  "For more information, try '--help'.\n"

test('a report is run with --root and in the root, both', async () => {
  const { exec, calls } = binary(() => ENVELOPE('/srv/corpus'))
  const result = await spawnReport({ command: 'y', root: '/srv/corpus', exec }, 'status')
  assert.equal(calls.length, 1)
  assert.deepEqual(calls[0].args, ['status', '--format', 'json', '--root', '/srv/corpus'])
  // The working directory stays: it is what a binary before the flag reads, and with both set
  // to one value they cannot disagree.
  assert.equal(calls[0].cwd, '/srv/corpus')
  assert.equal(result.ok, true)
  assert.equal(result.resolvedBy, 'flag')
  assert.equal(result.resolvedRoot, '/srv/corpus')
})

test('a binary predating --root is asked again without it, and says so', async () => {
  const { exec, calls } = binary((args) => {
    if (args.includes('--root')) throw exited(2, OLD_CLAP)
    return ENVELOPE('/repo')
  })
  const result = await spawnReport({ command: 'y', root: '/repo/examples/streamflow', exec }, 'lint')
  assert.deepEqual(
    calls.map((c) => c.args),
    [
      ['lint', '--format', 'json', '--root', '/repo/examples/streamflow'],
      ['lint', '--format', 'json'],
    ],
  )
  assert.equal(calls[1].cwd, '/repo/examples/streamflow')
  assert.equal(result.ok, true)
  assert.equal(result.resolvedBy, 'working-directory')
  assert.equal(result.resolvedRoot, '/repo')
})

test('any other failure is not retried', async () => {
  // A retry keyed on "it failed" would re-run a refused root without the flag and render the
  // outer repository's empty answer — the very defect the flag is there to end.
  const { exec, calls } = binary(() => {
    throw exited(1, 'Error: not a yidam repository: /srv/typo has no .yidam/ directory\n')
  })
  const result = await spawnReport({ command: 'y', root: '/srv/typo', exec }, 'status')
  assert.equal(calls.length, 1)
  assert.equal(result.ok, false)
})

test('a refused root reaches the reader in the binary’s words, not as a stale binary', async () => {
  const stderr =
    'Error: not a yidam repository: /srv/typo has no .yidam/ directory\n' +
    '  This is a git repository, but not one yidam bootstrapped.\n'
  const { exec } = binary(() => {
    throw exited(1, stderr)
  })
  const result = await spawnReport({ command: 'y', root: '/srv/typo', exec }, 'status')
  assert.equal(result.ok, false)
  // The handshake reads empty stdout as a binary predating `--format json`. That is the wrong
  // repair for a current binary pointed at the wrong directory.
  assert.equal(result.handshake.kind, 'not-json')
  const said = describeUnavailable(result)
  assert.match(said, /^not a yidam repository: \/srv\/typo/)
  assert.doesNotMatch(said, /predates/)
})

test('a failure with no refusal keeps the handshake’s message', async () => {
  const { exec } = binary(() => 'yidam 0.3.0\nsome prose report\n')
  const result = await spawnReport({ command: 'y', root: '/srv/corpus', exec }, 'status')
  assert.equal(result.ok, false)
  assert.equal(result.refusal, null)
  assert.match(describeUnavailable(result), /predates `--format json`/)
})

test('only anyhow’s Error: shape is a refusal', () => {
  assert.equal(refusalOf('Error: no corpus\n  because\n'), 'no corpus\n  because')
  assert.equal(refusalOf("thread 'main' panicked at src/main.rs:1:1"), null)
  assert.equal(refusalOf(OLD_CLAP), null)
  assert.equal(refusalOf(''), null)
})

test('agreement says nothing', () => {
  assert.equal(describeRootMismatch('/srv/corpus', '/srv/corpus', 'flag'), null)
  assert.equal(describeRootMismatch('/srv/corpus', '/srv/corpus', 'working-directory'), null)
})

test('an envelope with no root says nothing', () => {
  // Every report carries `root`, but a future one that does not must not produce a warning
  // about a mismatch nobody can see.
  assert.equal(describeRootMismatch('/srv/corpus', null, 'flag'), null)
})

test('an old binary overshooting a nested corpus is stated, with the reason and the repair', () => {
  const message = describeRootMismatch('/repo/examples/streamflow', '/repo', 'working-directory')
  assert.ok(message)
  assert.match(message, /\/repo\/examples\/streamflow/)
  assert.match(message, /\/repo\b/)
  // The reason, not only the fact. A person who reads "these differ" and nothing else has to
  // go and find out why on their own.
  assert.match(message, /predates --root/)
  assert.match(message, /show-toplevel/)
  assert.match(message, /yidam-vendor-update/)
})

test('a flag resolved to an enclosing corpus is stated, without a repair', () => {
  // Handed `--root /srv/corpus/.yidam/corpus`, the binary walks up to `/srv/corpus`. Correct,
  // and worth one line; blaming the binary for it would send someone to re-pin for nothing.
  const message = describeRootMismatch('/srv/corpus/.yidam/corpus', '/srv/corpus', 'flag')
  assert.ok(message)
  assert.match(message, /nearest directory/)
  assert.doesNotMatch(message, /predates|show-toplevel|re-pin/i)
})

test('it is a plain comparison, so canonicalisation stays session.ts’s job', () => {
  // If this ever returns null, somebody has papered over the symlink case here instead of in
  // `session.ts`, and the real-path handling has two homes.
  assert.ok(describeRootMismatch('/tmp/corpus', '/private/tmp/corpus', 'flag'))
})
