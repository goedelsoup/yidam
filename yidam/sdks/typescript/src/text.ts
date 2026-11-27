/**
 * Where a line ends and what whitespace is — the Rust SDK's rule, written out.
 *
 * One copy for every parity function that reads lines or trims text, so that `parse_markers`
 * and `classify_commit` cannot drift apart again: each called JavaScript's built-ins on its
 * own, and `trim` draws whitespace differently from Rust (#1620, #1666). Not exported from the
 * package; it is how the parity functions agree, not part of what they promise.
 */

/**
 * The lines of `text`, each with its offset into the original — the Rust SDK's rule, written
 * out (#1620).
 *
 * A line ends at `\n` and nowhere else, and one `\r` before that `\n` is not part of it. A
 * last line with no `\n` after it is kept as it is, and a text ending in `\n` has no empty
 * line after it. `split('\n')` alone leaves that empty element, which counted one more
 * swallowed line here than in the other SDKs. Python's `splitlines` breaks on `\f`, `\v`, a
 * lone `\r` and U+2028 as well. Either is a second rule for one contract.
 *
 * The offset is the whole point — recovering it afterwards means re-searching — and `split`
 * throws it away, so the lengths are added back up.
 */
export function linesWithOffsets(text: string): Array<[number, string]> {
  const raw = text.split('\n')
  const last = raw.length - 1
  const out: Array<[number, string]> = []
  let at = 0
  for (let n = 0; n < last; n++) {
    const line = raw[n]
    out.push([at, line.endsWith('\r') ? line.slice(0, -1) : line])
    at += line.length + 1
  }
  if (raw[last] !== '') out.push([at, raw[last]])
  return out
}

/**
 * The first line of `text` by `linesWithOffsets`' rule, or `''` if it has none — Rust's
 * `text.lines().next().unwrap_or("")` (#1666).
 */
export function firstLine(text: string): string {
  const lines = linesWithOffsets(text)
  return lines.length > 0 ? lines[0][1] : ''
}

/**
 * Rust's `char::is_whitespace`: the Unicode `White_Space` property, and nothing else.
 *
 * Not `String.prototype.trim`, which adds U+FEFF and leaves U+0085 — so a BOM before a marker
 * made it a marker here and prose in the other two SDKs (#1620), and a BOM before a commit
 * verb made the commit Operational here and Epistemic in the other two (#1666).
 */
export function isWhiteSpace(c: number): boolean {
  return (
    (c >= 0x09 && c <= 0x0d) ||
    c === 0x20 ||
    c === 0x85 ||
    c === 0xa0 ||
    c === 0x1680 ||
    (c >= 0x2000 && c <= 0x200a) ||
    c === 0x2028 ||
    c === 0x2029 ||
    c === 0x202f ||
    c === 0x205f ||
    c === 0x3000
  )
}

/** `trimStart` over `isWhiteSpace` — Rust's `str::trim_start`. */
export function trimStart(s: string): string {
  let a = 0
  while (a < s.length && isWhiteSpace(s.charCodeAt(a))) a++
  return s.slice(a)
}

/** `trimEnd` over `isWhiteSpace` — Rust's `str::trim_end`. */
export function trimEnd(s: string): string {
  let b = s.length
  while (b > 0 && isWhiteSpace(s.charCodeAt(b - 1))) b--
  return s.slice(0, b)
}

/** `trim` over `isWhiteSpace` — Rust's `str::trim`. */
export function trim(s: string): string {
  return trimEnd(trimStart(s))
}
