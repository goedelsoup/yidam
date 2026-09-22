/**
 * The smallest YAML emitter that can write a node — RFC-0030 Phase 2, #607.
 *
 * An emitter and never a parser. The form holds a node as fields; what the overlay judges is
 * a buffer; this is the one-way step between them. Nothing here reads YAML back, because the
 * binary already does and a second reading is a second opinion — the hand-edit escape on the
 * form detaches the fields rather than parse the text into them.
 *
 * Two scalar forms and no third. A plain scalar when the value is safely one: letters,
 * digits, spaces and a few path characters, starting with a letter or a dot, and not a word YAML
 * reads as something other than a string. Everything else is double-quoted with JSON's
 * escaping, which YAML's double-quoted style accepts. A literal block (`|`) for prose that
 * has more than one line, since that is how every shipped corpus writes a description and it
 * keeps `[verified]` and its siblings readable where the claim check looks for them.
 */

const NOT_A_STRING = new Set(['true', 'false', 'null', 'yes', 'no', 'on', 'off', 'y', 'n', '~'])

/** Whether a value survives as a plain scalar — read back as the same string, unquoted. */
export function isPlain(value: string): boolean {
  // A leading `.` is allowed for the relative paths every link target is.
  if (!/^[A-Za-z.][A-Za-z0-9 _./-]*$/.test(value)) return false
  if (value.endsWith(' ')) return false
  return !NOT_A_STRING.has(value.toLowerCase())
}

export function scalar(value: string): string {
  return isPlain(value) ? value : JSON.stringify(value)
}

/**
 * Prose as a value: a literal block when there is more than one line, a scalar otherwise.
 *
 * `indent` is the column the block's lines sit at. A first line that starts with a space
 * needs the indentation indicator (`|2`) or YAML cannot tell content from indentation;
 * trailing newlines are trimmed so `|` (clip) and the text agree about how many there are.
 */
export function prose(value: string, indent: number): string {
  const text = value.replace(/\r\n/g, '\n').replace(/\n+$/, '')
  if (!text.includes('\n')) return scalar(text)
  const pad = ' '.repeat(indent)
  const lines = text.split('\n').map((line) => (line === '' ? '' : `${pad}${line}`))
  const header = text.startsWith(' ') ? `|${indent}` : '|'
  return `${header}\n${lines.join('\n')}`
}

export interface NodeDocument {
  class: string
  label: string
  description: string
  /** Emitted in the order given; empty values are left out (see `form.ts`). */
  properties: [string, string][]
  links: { relationship: string; target: string }[]
}

/** A node file, in the shape the shipped corpora write by hand. */
export function emitNode(node: NodeDocument): string {
  const lines = [`class: ${scalar(node.class)}`, `label: ${scalar(node.label)}`]
  lines.push(`description: ${prose(node.description, 2)}`)
  if (node.properties.length > 0) {
    lines.push('properties:')
    for (const [name, value] of node.properties) lines.push(`  ${name}: ${prose(value, 4)}`)
  }
  lines.push(node.links.length === 0 ? 'links: []' : 'links:')
  for (const link of node.links) {
    lines.push(`  - target: ${scalar(link.target)}`)
    lines.push(`    relationship: ${scalar(link.relationship)}`)
  }
  return `${lines.join('\n')}\n`
}
