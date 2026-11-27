/**
 * Typing the resolved binary into a terminal.
 *
 * `blessBaseline` used to type a bare `yidam`, so the terminal's `PATH` chose the build and
 * the extension's own resolution (the pin, `yidam.path`) was ignored.
 */

import assert from 'node:assert/strict'
import { test } from 'node:test'

import { terminalCommand } from '../src/terminal.ts'

const bless = ['lint', '--bless']

test('the resolved binary is what gets typed, not a bare yidam', () => {
  assert.equal(
    terminalCommand('/work/repo/.mise/bin/yidam', bless, '/bin/zsh'),
    '/work/repo/.mise/bin/yidam lint --bless',
  )
})

test('a path with a space is double-quoted', () => {
  assert.equal(
    terminalCommand('/Users/a b/bin/yidam', bless, '/bin/bash'),
    '"/Users/a b/bin/yidam" lint --bless',
  )
  assert.equal(
    terminalCommand('C:\\Program Files\\yidam\\yidam.exe', bless, 'C:\\Windows\\System32\\cmd.exe'),
    '"C:\\Program Files\\yidam\\yidam.exe" lint --bless',
  )
})

test('PowerShell needs the call operator before a quoted path', () => {
  assert.equal(
    terminalCommand('C:\\Program Files\\yidam\\yidam.exe', bless, 'C:\\...\\pwsh.exe'),
    '& "C:\\Program Files\\yidam\\yidam.exe" lint --bless',
  )
})

test('a path with shell metacharacters is refused rather than quoted', () => {
  for (const bad of [
    '/x/$(id)/yidam',
    '/x/a`id`/yidam',
    '/x/a;b/yidam',
    '/x/a"b/yidam',
    "/x/a'b/yidam",
    '/x/a\nb/yidam',
    '/x/a&b/yidam',
    '/x/a%PATH%/yidam',
    '',
  ]) {
    assert.equal(terminalCommand(bad, bless, '/bin/zsh'), null, JSON.stringify(bad))
  }
})
