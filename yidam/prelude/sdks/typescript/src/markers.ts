export interface TemplateMarker {
  kind: 'Template'
  instruction: string
}

export interface RegenMarker {
  kind: 'Regen'
  command: string
  content: string
}

export type Marker = TemplateMarker | RegenMarker

/** What is wrong with a REGEN block the scan crossed. */
export type Fault = 'OpenArrowMissing' | 'CloseTagMissing' | 'ClosedOnAnothersTag'

/**
 * A REGEN block whose extent the scan could not read the way it was meant.
 *
 * In every case the block has taken lines that were not its content, and every marker among
 * them is a marker the caller never sees — which is what `swallowedMarkers` counts.
 */
export interface MalformedBlock {
  command: string
  /** 1-indexed line the open tag sits on. */
  line: number
  fault: Fault
  /** Lines after the open tag that this block took as its own. */
  swallowedLines: number
  /** How many of those lines open a marker — markers that are now content. */
  swallowedMarkers: number
}

/**
 * Where a well-formed REGEN block sits in the text, so that a writer does not have to look
 * for it a second time.
 *
 * The offsets are **UTF-16 code units**, because this is JavaScript. They are not a parity
 * contract and no fixture asserts one: the Rust SDK counts bytes and the Python SDK code
 * points, so a single number here is three different claims about any document holding a
 * character outside ASCII — the trap `findReachable`'s sort order already fell into. What the
 * fixtures grade is what the scan and the writer *do*. See RFC-0043.
 *
 * Only blocks the scan read to the end get one. A block whose open tag never closed, or whose
 * `<!-- /REGEN -->` never arrived, has no extent to write into, and `updateRegen` left such a
 * block alone before this type existed.
 */
export interface RegenSpan {
  /** The command on the open tag, as the scan read it — trimmed. */
  command: string
  /** Offset of the `<` that opens the block. */
  open: number
  /** Offset just past the `-->` that ends the open tag. The body starts here. */
  body: number
  /** Offset of the `<` in this block's `<!-- /REGEN -->`. The body ends here. */
  close: number
  /**
   * Whether the body holds no newline — the whole block sits on one line.
   *
   * The rule RFC-0043 states, applied to the text rather than to how the block was found:
   * `-->44<!-- /REGEN -->` and `--><!-- /REGEN -->` are inline, and a body of `\n` — the shape
   * a cleared section has — is not.
   */
  inline: boolean
}

/**
 * What one pass over the text found: the markers, the blocks that are malformed, and where
 * the well-formed ones are.
 */
export interface Scan {
  markers: Marker[]
  malformed: MalformedBlock[]
  /**
   * One per well-formed REGEN block, in document order. `updateRegen` is defined over this
   * and nothing else, which is what makes the reader and the writer agree about what a block
   * is (#1094).
   */
  regen: RegenSpan[]
}

/**
 * `str::lines()`, which is not `split('\n')`.
 *
 * The difference is one trailing empty element on any text ending in a newline — invisible
 * while the only output was markers, because a trailing blank is not a marker and is trimmed
 * out of any content it lands in. It stops being invisible the moment a line is *counted*:
 * the same file would report one more swallowed line here than in the Rust and Python SDKs,
 * and the parity fixture for a malformed block is what would have caught it.
 */
function toLines(text: string): string[] {
  const out = text.split('\n')
  if (out.length > 0 && out[out.length - 1] === '') out.pop()
  return out.map((l) => (l.endsWith('\r') ? l.slice(0, -1) : l))
}

const REGEN_OPEN = '<!-- REGEN:'
const REGEN_CLOSE = '<!-- /REGEN -->'
const ARROW = '-->'

/**
 * `toLines`, keeping each line's offset into the original text.
 *
 * The offset is the whole point — recovering it afterwards means re-searching — and `split`
 * throws it away, so the lengths are added back up. A `\r` is not part of the line but is
 * part of what separates it from the next, which is why the running offset uses the raw
 * split and the yielded text uses the stripped one.
 */
function linesWithOffsets(text: string): Array<[number, string]> {
  const raw = text.split('\n')
  if (raw.length > 0 && raw[raw.length - 1] === '') raw.pop()
  const out: Array<[number, string]> = []
  let at = 0
  for (const line of raw) {
    out.push([at, line.endsWith('\r') ? line.slice(0, -1) : line])
    at += line.length + 1
  }
  return out
}

/**
 * Whether a line opens a REGEN block. A body containing one means a close tag is missing
 * above it, which is what separates `ClosedOnAnothersTag` from a block that is merely long.
 */
function opensARegen(line: string): boolean {
  return line.trim().startsWith('<!-- REGEN:')
}

/** Whether a line opens a marker of either kind. */
function opensAMarker(line: string): boolean {
  const t = line.trim()
  return t.startsWith('<!-- REGEN:') || t.startsWith('<!-- TEMPLATE:')
}

/**
 * The markers, and the blocks that took lines which were not theirs.
 *
 * One pass, two outputs. `parseMarkers` is this without the second, and keeps its signature:
 * the marker sequence is a frozen parity contract and does not change here.
 */
export function scanMarkers(text: string): Scan {
  const markers: Marker[] = []
  const malformed: MalformedBlock[] = []
  const regen: RegenSpan[] = []
  const lines = linesWithOffsets(text)
  let i = 0

  while (i < lines.length) {
    const [lineAt, line] = lines[i]
    const stripped = line.trim()

    if (stripped.startsWith('<!-- TEMPLATE:')) {
      const rest = stripped.slice('<!-- TEMPLATE:'.length)
      if (rest.endsWith(ARROW)) {
        const instruction = rest.slice(0, -ARROW.length).trim()
        markers.push({ kind: 'Template', instruction })
      }
      i++
      continue
    }

    // Every block this line opens *and closes*, left to right — the inline form. A line may
    // carry more than one, and a block found here is finished: nothing below runs for it.
    // `col` walks past each one so the next search starts after its close tag rather than
    // inside its body.
    let col = 0
    let blockOpen: number | null = null
    for (;;) {
      const rel = line.indexOf(REGEN_OPEN, col)
      if (rel === -1) break
      const afterOpen = rel + REGEN_OPEN.length
      const arrowAt = line.indexOf(ARROW, afterOpen)
      if (arrowAt === -1) {
        blockOpen = rel
        break
      }
      const bodyCol = arrowAt + ARROW.length
      const closeCol = line.indexOf(REGEN_CLOSE, bodyCol)
      if (closeCol === -1) {
        blockOpen = rel
        break
      }
      const command = line.slice(afterOpen, arrowAt).trim()
      regen.push({
        command,
        open: lineAt + rel,
        body: lineAt + bodyCol,
        close: lineAt + closeCol,
        inline: true,
      })
      markers.push({ kind: 'Regen', command, content: line.slice(bodyCol, closeCol).trim() })
      col = closeCol + REGEN_CLOSE.length
    }

    // What is left is an open tag with no close beside it — the block form, which is only a
    // block when it starts its line. A `<!-- REGEN:` in the middle of a sentence with no
    // close tag after it is prose, and was prose before this function learned the inline
    // form; reading it as a block would make it swallow the rest of the file.
    if (blockOpen === null || line.slice(0, blockOpen).trim() !== '') {
      i++
      continue
    }
    const openCol = blockOpen
    const rest = line.slice(openCol + REGEN_OPEN.length)
    const restTrimmed = rest.trim()
    const openLine = i
    let fault: Fault | null = null
    let bodyAt: number | null = null
    let command: string

    if (restTrimmed.endsWith(ARROW)) {
      // Single-line open tag. The arrow is the last thing on the line by that test, so the
      // body starts where the line's own trailing whitespace does.
      command = restTrimmed.slice(0, -ARROW.length).trim()
      bodyAt = lineAt + line.trimEnd().length
      i++
    } else {
      command = rest.trim()
      i++
      let arrowFound = false
      while (i < lines.length) {
        const [at, l] = lines[i]
        const t = l.trim()
        i++
        if (t === ARROW || t.endsWith(ARROW)) {
          bodyAt = at + l.trimEnd().length
          arrowFound = true
          break
        }
      }
      if (!arrowFound) fault = 'OpenArrowMissing'
    }

    const contentStart = i
    let contentEnd = lines.length
    let closeAt: number | null = null
    let closed = false
    while (i < lines.length) {
      const [at, l] = lines[i]
      if (l.trim() === REGEN_CLOSE) {
        contentEnd = i
        closeAt = at + (l.length - l.trimStart().length)
        i++
        closed = true
        break
      }
      i++
    }
    if (fault === null) {
      if (!closed) fault = 'CloseTagMissing'
      else if (lines.slice(contentStart, contentEnd).some(([, l]) => opensARegen(l)))
        fault = 'ClosedOnAnothersTag'
    }

    if (fault !== null) {
      // From the open tag to wherever the content stopped, which in the `OpenArrow` case is
      // the end of the input: the body is empty there and everything was consumed looking
      // for the arrow, so a count over the body alone reports nothing.
      const swallowed = lines.slice(openLine + 1, contentEnd)
      malformed.push({
        command,
        line: openLine + 1,
        fault,
        swallowedLines: swallowed.length,
        swallowedMarkers: swallowed.filter(([, l]) => opensAMarker(l)).length,
      })
    }

    // An extent, but only for a block with two ends. `OpenArrowMissing` has no body to
    // start, and `CloseTagMissing` none to end; `updateRegen` returned such a file unchanged
    // when it did its own searching, and it returns it unchanged now.
    if (bodyAt !== null && closeAt !== null) {
      regen.push({
        command,
        open: lineAt + openCol,
        body: bodyAt,
        close: closeAt,
        inline: !text.slice(bodyAt, closeAt).includes('\n'),
      })
    }

    const content = lines
      .slice(contentStart, contentEnd)
      .map(([, l]) => l)
      .join('\n')
      .trim()
    markers.push({ kind: 'Regen', command, content })
  }

  return { markers, malformed, regen }
}

export function parseMarkers(text: string): Marker[] {
  return scanMarkers(text).markers
}

/**
 * The body to write between a block's markers.
 *
 * An inline block keeps its line: the author wrote it that way and no regeneration
 * second-guesses them. Every other block is bracketed by newlines, and the empty case is
 * collapsed so that clearing a section does not leave a blank line between the markers.
 */
function regenBody(newContent: string, inline: boolean, standsAlone: boolean): string | null {
  // Both conditions: `inline` is the form the block was written in, and the newline test is
  // whether the new content can still be written that way. See the Rust note. `null` is an
  // inline block that shares its line, which block form would break (#1137).
  if (inline && !newContent.includes('\n')) return newContent
  if (inline && !standsAlone) return null
  if (newContent === '') return '\n'
  return `\n${newContent}\n`
}

/** Whether nothing but whitespace shares the block's line, before its open tag or after its close tag. */
function standsAlone(text: string, span: RegenSpan): boolean {
  const start = text.lastIndexOf('\n', span.open - 1) + 1
  const end = span.close + REGEN_CLOSE.length
  const nl = text.indexOf('\n', end)
  const stop = nl === -1 ? text.length : nl
  return text.slice(start, span.open).trim() === '' && text.slice(end, stop).trim() === ''
}

/**
 * Replace the body of every REGEN block named `command`.
 *
 * Defined over `scanMarkers`, which is the point: before #1094 this searched the text itself,
 * and the two searches disagreed. A block the scan cannot read is a block this leaves alone,
 * and a block it reads inline stays inline — or, given a value that needs more than one line,
 * takes the block form if the line is its own and is left alone if not.
 *
 * Two things changed with the second search's removal, and both close a defect rather than
 * open a feature. The command matches **exactly**, where the old prefix search let
 * `yidam status` claim a `yidam status-quo` block (#1058). And **every** block with the
 * command is rewritten, where the old search rewrote the first and left a second copy of the
 * same figure stale with nothing to report it.
 */
export function updateRegen(text: string, command: string, newContent: string): string {
  const spans = scanMarkers(text).regen.filter((s) => s.command === command)
  if (spans.length === 0) return text

  let out = ''
  let cursor = 0
  for (const span of spans) {
    const body = regenBody(newContent, span.inline, standsAlone(text, span))
    if (body === null) continue
    out += text.slice(cursor, span.body)
    out += body
    cursor = span.close
  }
  return out + text.slice(cursor)
}
