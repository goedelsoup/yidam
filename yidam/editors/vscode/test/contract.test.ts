/**
 * The extension's reader, driven against the corpora that certify the reports.
 *
 * RFC-0016 asks for this explicitly: the `reports/` golden trees become the repositories
 * the extension is exercised on, so a fixture whose output changes fails the parity run
 * *and* these tests. Two things then cannot drift apart quietly — the contract the CLI
 * emits and the contract this extension believes it emits.
 *
 * No editor is involved. This is the reader, the real binary, and a real corpus.
 */

import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { test } from 'node:test'

import { resolveBinary } from '../src/binary.ts'
import { hasFeature, readHandshake } from '../src/handshake.ts'
import { stageFixture } from './stage.ts'



/**
 * Run and keep both streams whatever the exit code.
 *
 * `lint` and `graph-check` gate — a nonzero exit is a verdict, not a failure to produce
 * one, and the envelope is on stdout regardless. An extension that treated exit != 0 as
 * "binary unusable" would go blind exactly when the corpus needs attention.
 */
function capture(bin: string, args: string[], cwd: string): { stdout: string; stderr: string } {
  try {
    return { stdout: execFileSync(bin, args, { cwd, encoding: 'utf8' }), stderr: '' }
  } catch (err) {
    const e = err as { stdout?: string; stderr?: string }
    return { stdout: e.stdout ?? '', stderr: e.stderr ?? '' }
  }
}

/**
 * A binary that actually speaks the contract, or `null`.
 *
 * Resolution is not enough. A *stale* yidam resolves perfectly well and then rejects
 * `--format`, which turned these tests red for a contributor whose `~/.cargo/bin` copy
 * predated the flag — a confusing failure about someone else's work.
 *
 * So a stale binary skips like an absent one. `YIDAM_REQUIRE_CONTRACT=1` turns both into
 * a hard failure, and CI sets it: skipping is a courtesy to contributors, never a way for
 * the gate to pass without having checked anything.
 */
async function contractBinary(cwd: string): Promise<string | null> {
  const r = await resolveBinary({ configured: process.env.YIDAM_BIN ?? '', workspace: cwd })
  if (!r.command) {
    if (required()) throw new Error(`YIDAM_REQUIRE_CONTRACT is set and no yidam resolved: ${r.reason}`)
    return null
  }
  const { stdout, stderr } = capture(r.command, ['status', '--format', 'json'], cwd)
  const h = readHandshake(stdout, stderr)
  if (!h.ok) {
    if (required()) {
      throw new Error(`YIDAM_REQUIRE_CONTRACT is set and ${r.command} does not speak it: ${h.message}`)
    }
    return null
  }
  return r.command
}

function required(): boolean {
  return (process.env.YIDAM_REQUIRE_CONTRACT ?? '') !== ''
}

const SKIP = 'no yidam speaking the report contract — set YIDAM_BIN, or `cargo install --path yidam/cli`'


test('the handshake accepts what a real yidam actually emits', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    // Skipped rather than failed: a contributor without the CLI on PATH should still be
    // able to run the unit tests. CI installs it, so CI does not take this branch.
    t.skip(SKIP)
    return
  }

  const { stdout, stderr } = capture(bin, ['status', '--format', 'json'], dir)
  const h = readHandshake(stdout, stderr)
  assert.equal(h.ok, true, `handshake failed: ${h.ok === false ? h.message : ''}`)
  assert.equal(hasFeature(h, 'reports'), true, 'the light build always reports `reports`')
})

test('a binary that rejects --format reads as contract skew, not a broken install', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // Simulated rather than requiring a stale binary on hand — but this is the real shape,
  // found by running these tests against a yidam that predated the flag: clap prints a
  // usage message to stderr, writes nothing to stdout, and exits nonzero.
  const h = readHandshake('', "error: unexpected argument '--format' found\n\nUsage: yidam status\n")
  assert.equal(h.ok, false)
  assert.equal(h.ok === false && h.kind, 'not-json')
  assert.match(h.ok === false ? h.message : '', /predates `--format json`/)
})

test('prose from the same binary degrades to a named state', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // Without --format json the very same command prints prose. This is what a binary
  // predating the contract looks like from the extension's side, and it must be a state
  // rather than a stack trace.
  const { stdout: prose } = capture(bin, ['status'], dir)
  const h = readHandshake(prose)
  assert.equal(h.ok, false)
  assert.equal(h.ok === false && h.kind, 'not-json')
})

test('a gating command still yields a readable envelope', async (t) => {
  const dir = stageFixture('yidam-ext-')
  const bin = await contractBinary(dir)
  if (!bin) {
    t.skip(SKIP)
    return
  }
  // `lint` exits nonzero on this fixture by design — it carries one deliberate broken
  // edge. The envelope is on stdout regardless, and the extension must read it rather
  // than treating a failing gate as an unusable binary.
  const { stdout, stderr } = capture(bin, ['lint', '--format', 'json'], dir)
  const h = readHandshake(stdout, stderr)
  assert.equal(h.ok, true)
  const doc = JSON.parse(stdout)
  assert.equal(doc.gate.passed, false, 'the fixture carries a deliberate error')
  assert.equal(typeof doc.gate.new_violations, 'number')
})
