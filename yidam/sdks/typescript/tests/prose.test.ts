/**
 * `proseOf` and `proseText` trim by Unicode `White_Space`, as the Rust SDK's `str::trim` does.
 *
 * Held here rather than as a parity fixture: `prose` is not on the parity function list.
 * `composeEmbedText` reads prose through `proseOf`, so its fixtures pin the emptiness test
 * (#1699). `proseText` is the one call no fixture reaches, and this pins both.
 *
 * JavaScript's `trim` also removes U+FEFF and leaves U+0085, so those are the cases the
 * built-in gets wrong.
 */
import { describe, expect, it } from 'vitest'

import { parseInstance } from '../src/corpus.ts'
import { proseOf, proseText } from '../src/prose.ts'

function node(value: string) {
  return parseInstance(`class: measure\nlabel: L\ndescription: "${value}"\n`)
}

describe('prose: White_Space', () => {
  const kept: [string, string, boolean][] = [
    ['U+001F', '\\x1F', true],
    ['U+FEFF', '\\uFEFF', true],
    ['U+0085', '\\x85', false],
    ['a space', ' ', false],
  ]
  for (const [name, value, isProse] of kept) {
    it(`a value of only ${name} ${isProse ? 'is' : 'is not'} prose`, () => {
      expect(proseOf(node(value), ['description'], [])).toHaveLength(isProse ? 1 : 0)
    })
  }

  const trimmed: [string, string, string][] = [
    ['U+001F', 'x\\x1F', 'x\u001f'],
    ['U+FEFF', 'x\\uFEFF', 'x\ufeff'],
    ['U+0085', 'x\\x85', 'x'],
    ['a space', 'x ', 'x'],
  ]
  for (const [name, value, text] of trimmed) {
    it(`proseText ${text === 'x' ? 'trims' : 'keeps'} a trailing ${name}`, () => {
      expect(proseText(node(value), ['description'], [])).toBe(text)
    })
  }
})
