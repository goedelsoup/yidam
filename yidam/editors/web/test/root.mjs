/**
 * The corpus that was asked for, and the one that answered.
 *
 * **No yidam subcommand takes a `--root` flag.** The corpus is resolved by
 * `git rev-parse --show-toplevel` from the working directory, so `--root` on this surface
 * sets a working directory and the binary decides from there. A corpus nested inside another
 * git repository therefore answers about the outer one, and that renders as a corpus with no
 * nodes in it — indistinguishable from an empty corpus, which is why the difference is stated
 * rather than left to be inferred from a blank page.
 *
 * Found by running the thing: pointing it at `examples/streamflow` inside this checkout gave
 * a page with zero nodes and no indication that anything was wrong.
 *
 * The second test is the one that keeps the warning usable. `/tmp` is a symlink to
 * `/private/tmp` on macOS, so a naive string compare warns about a directory that is
 * perfectly correct — and a warning that cries wolf is a warning people learn to scroll past.
 * `session.ts` canonicalises before comparing; this asserts the message itself is a plain
 * comparison, so the canonicalisation cannot be quietly dropped and made up for here.
 */

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { describeRootMismatch } from '../src/lib/messages.ts'

test('agreement says nothing', () => {
  assert.equal(describeRootMismatch('/srv/corpus', '/srv/corpus'), null)
})

test('an envelope with no root says nothing', () => {
  // Every report carries `root`, but a future one that does not must not produce a warning
  // about a mismatch nobody can see.
  assert.equal(describeRootMismatch('/srv/corpus', null), null)
})

test('a different root is stated, with both paths in it', () => {
  const message = describeRootMismatch('/repo/examples/streamflow', '/repo')
  assert.ok(message)
  assert.match(message, /\/repo\/examples\/streamflow/)
  assert.match(message, /\/repo\b/)
  // The reason, not only the fact. A person who reads "these differ" and nothing else has to
  // go and find out why on their own.
  assert.match(message, /--root/)
  assert.match(message, /show-toplevel/)
})

test('it is a plain comparison, so canonicalisation stays session.ts’s job', () => {
  // If this ever returns null, somebody has papered over the symlink case here instead of in
  // `session.ts`, and the real-path handling has two homes.
  assert.ok(describeRootMismatch('/tmp/corpus', '/private/tmp/corpus'))
})
