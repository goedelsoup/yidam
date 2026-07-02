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

export function parseMarkers(text: string): Marker[] {
  const markers: Marker[] = []
  const lines = text.split('\n')
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

    if (stripped.startsWith('<!-- REGEN:')) {
      const rest = stripped.slice('<!-- REGEN:'.length)
      const restTrimmed = rest.trim()
      let command: string
      if (restTrimmed.endsWith('-->')) {
        command = restTrimmed.slice(0, -'-->'.length).trim()
        i++
      } else {
        command = rest.trim()
        i++
        while (i < lines.length) {
          const t = lines[i].trim()
          i++
          if (t === '-->' || t.endsWith('-->')) break
        }
      }

      const contentLines: string[] = []
      while (i < lines.length) {
        if (lines[i].trim() === '<!-- /REGEN -->') {
          i++
          break
        }
        contentLines.push(lines[i])
        i++
      }
      const content = contentLines.join('\n').trim()
      markers.push({ kind: 'Regen', command, content })
      continue
    }

    i++
  }

  return markers
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
