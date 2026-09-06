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

/** What one pass over the text found: the markers, and the blocks that are malformed. */
export interface Scan {
  markers: Marker[]
  malformed: MalformedBlock[]
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
  const lines = toLines(text)
  let i = 0

  while (i < lines.length) {
    const stripped = lines[i].trim()

    if (stripped.startsWith('<!-- TEMPLATE:')) {
      const rest = stripped.slice('<!-- TEMPLATE:'.length)
      if (rest.endsWith('-->')) {
        const instruction = rest.slice(0, -'-->'.length).trim()
        markers.push({ kind: 'Template', instruction })
      }
      i++
      continue
    }

    if (!stripped.startsWith('<!-- REGEN:')) {
      i++
      continue
    }

    const rest = stripped.slice('<!-- REGEN:'.length)
    const restTrimmed = rest.trim()
    const openLine = i
    let fault: Fault | null = null
    let command: string

    if (restTrimmed.endsWith('-->')) {
      command = restTrimmed.slice(0, -'-->'.length).trim()
      i++
    } else {
      command = rest.trim()
      i++
      let arrowFound = false
      while (i < lines.length) {
        const t = lines[i].trim()
        i++
        if (t === '-->' || t.endsWith('-->')) {
          arrowFound = true
          break
        }
      }
      if (!arrowFound) fault = 'OpenArrowMissing'
    }

    const contentStart = i
    let contentEnd = lines.length
    let closed = false
    while (i < lines.length) {
      if (lines[i].trim() === '<!-- /REGEN -->') {
        contentEnd = i
        i++
        closed = true
        break
      }
      i++
    }
    if (fault === null) {
      if (!closed) fault = 'CloseTagMissing'
      else if (lines.slice(contentStart, contentEnd).some(opensARegen))
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
        swallowedMarkers: swallowed.filter(opensAMarker).length,
      })
    }

    const content = lines.slice(contentStart, contentEnd).join('\n').trim()
    markers.push({ kind: 'Regen', command, content })
  }

  return { markers, malformed }
}

export function parseMarkers(text: string): Marker[] {
  return scanMarkers(text).markers
}

export function updateRegen(text: string, command: string, newContent: string): string {
  const openTag = `<!-- REGEN: ${command}`
  const closeTag = '<!-- /REGEN -->'

  const openPos = text.indexOf(openTag)
  if (openPos === -1) return text

  const afterOpen = openPos + openTag.length
  const arrowRel = text.indexOf('-->', afterOpen)
  if (arrowRel === -1) return text

  const contentStart = arrowRel + 3
  const closeAbs = text.indexOf(closeTag, contentStart)
  if (closeAbs === -1) return text

  if (newContent === '') {
    // Clear the body without leaving a blank line between the markers.
    return `${text.slice(0, contentStart)}\n${text.slice(closeAbs)}`
  }
  return `${text.slice(0, contentStart)}\n${newContent}\n${text.slice(closeAbs)}`
}
