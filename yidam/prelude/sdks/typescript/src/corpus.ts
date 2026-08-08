export type EvidenceTag = 'Verified' | 'Inference' | 'Open' | 'Implicit'
export type NodeKind = 'Concept' | 'Decision' | 'Authored' | 'Generated'

export interface Claim {
  text: string
  tag: EvidenceTag
}

export interface Link {
  label: string
  target: string
  anchor?: string
}

export interface CorpusNode {
  path: string
  title: string
  kind: NodeKind
  claims: Claim[]
  links: Link[]
}

const LINK_RE = /(?<!!)\[([^\]]+)\]\(([^)]+)\)/g

export function extractLinks(text: string): Link[] {
  const links: Link[] = []
  for (const m of text.matchAll(LINK_RE)) {
    const label = m[1]
    const dest = m[2]
    const hashPos = dest.indexOf('#')
    if (hashPos !== -1) {
      const target = dest.slice(0, hashPos)
      const anchor = dest.slice(hashPos + 1) || undefined
      links.push({ label, target, anchor })
    } else {
      links.push({ label, target: dest })
    }
  }
  return links
}

function containsMdLink(s: string): boolean {
  return s.includes('](')
}

export function extractClaims(text: string): Claim[] {
  const claims: Claim[] = []
  for (let line of text.split('\n')) {
    line = line.trim()
    if (!line) continue
    if ('#`|-*!>'.includes(line[0])) continue
    if (line.startsWith('<!--')) continue
    if (line.endsWith(' [verified]')) {
      claims.push({ text: line.slice(0, -' [verified]'.length), tag: 'Verified' })
    } else if (line.endsWith(' [inference]')) {
      claims.push({ text: line.slice(0, -' [inference]'.length), tag: 'Inference' })
    } else if (line.endsWith(' [open]')) {
      claims.push({ text: line.slice(0, -' [open]'.length), tag: 'Open' })
    } else if (!containsMdLink(line)) {
      claims.push({ text: line, tag: 'Implicit' })
    }
  }
  return claims
}

export function parseNode(path: string, text: string): CorpusNode {
  let title = ''
  for (const line of text.split('\n')) {
    if (line.startsWith('# ')) {
      title = line.slice(2).trim()
      break
    }
  }

  const kind: NodeKind = path.startsWith('corpus/')
    ? 'Concept'
    : path.startsWith('decisions/')
      ? 'Decision'
      : 'Authored'

  return {
    path,
    title,
    kind,
    claims: extractClaims(text),
    links: extractLinks(text),
  }
}
