import assert from 'node:assert/strict'
import { test } from 'node:test'

import { describe, hasFeature, readHandshake, SUPPORTED_FORMAT_VERSION } from '../src/handshake.ts'

const envelope = (over: Record<string, unknown> = {}) =>
  JSON.stringify({
    format_version: SUPPORTED_FORMAT_VERSION,
    yidam: { version: '0.1.0', commit: 'bf7d203', features: ['reports'] },
    root: '/r',
    ...over,
  })

test('a current envelope is accepted and described', () => {
  const h = readHandshake(envelope())
  assert.equal(h.ok, true)
  assert.equal(describe(h), 'yidam 0.1.0 (bf7d203)')
})

test('a binary predating --format json is a named state, not a stack trace', () => {
  // The single most likely failure: prose on stdout where JSON was asked for.
  const h = readHandshake('**2 nodes** · 0 open · 1 sources\n')
  assert.equal(h.ok, false)
  assert.equal(h.ok === false && h.kind, 'not-json')
  assert.match(h.ok === false ? h.message : '', /predates `--format json`/)
})

test('an unknown major disables verdicts rather than guessing', () => {
  const h = readHandshake(envelope({ format_version: '2' }))
  assert.equal(h.ok, false)
  assert.equal(h.ok === false && h.kind, 'unsupported-version')
  assert.match(h.ok === false ? h.message : '', /disabled rather than guessed/)
})

test('a future minor of the same major keeps working', () => {
  // Adding a field is not a breaking change and consumers must ignore what they do not
  // know — otherwise every additive change strands every older editor.
  const h = readHandshake(envelope({ format_version: '1.4' }))
  assert.equal(h.ok, true)
})

test('JSON that is not an envelope is rejected', () => {
  assert.equal(readHandshake('{"hello":1}').ok, false)
  assert.equal(readHandshake('[]').ok, false)
  assert.equal(readHandshake('null').ok, false)
})

test('feature gating reads the block rather than assuming', () => {
  const h = readHandshake(envelope())
  assert.equal(hasFeature(h, 'reports'), true)
  assert.equal(hasFeature(h, 'index'), false)
  // A failed handshake advertises nothing.
  assert.equal(hasFeature(readHandshake('nope'), 'reports'), false)
})

test('a binary that rejects the flag is skew, not a broken install', () => {
  // Found by running the contract tests against a stale yidam on PATH: clap writes a
  // usage message to stderr and nothing to stdout. Reporting that as "could not run
  // yidam" would send a user hunting a broken install instead of a stale pin.
  const h = readHandshake('', "error: unexpected argument '--format' found\n\nUsage: yidam status\n")
  assert.equal(h.ok, false)
  assert.equal(h.ok === false && h.kind, 'not-json')
  assert.match(h.ok === false ? h.message : '', /predates `--format json`/)
})

test('stderr noise from a working binary does not derail a good envelope', () => {
  const h = readHandshake(envelope(), 'warning: something unrelated\n')
  assert.equal(h.ok, true)
})
