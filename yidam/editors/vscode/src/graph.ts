/**
 * Link navigation, as data.
 *
 * # What this file is allowed to know
 *
 * A corpus edge is a filesystem-relative path inside YAML, resolved as `dir.join(target)`
 * against the instance's own directory and normalized. That rule belongs to `dangling_edge`
 * and `orphan_in`, and **the CLI applies it** — `yidam graph --format json` reports every
 * edge already resolved, with `exists` answered by the same test the gate uses. Nothing here
 * decides whether an edge is broken.
 *
 * What is left is genuinely navigation: reading the scalar under the cursor out of one line
 * of text, joining a path so a `Go to Definition` works in a buffer that has not been saved
 * since the report was taken, and ranking a completion list. When any of it is wrong you
 * fail to jump. The verdict still comes from `lint`.
 *
 * # The ontology is a guide, not a closed list
 *
 * Measured against a live derived repository at 90 nodes and 299 edges: **17 of the
 * (class, relationship) pairs actually in use are not declared as `out` edges on their
 * class**, and one of them — `instance-of`, the edge every node carries to its own
 * `.ont.yml` — is used by every class and declared by none. Nothing lints relationships
 * against the ontology, so a completion list restricted to declared edges would be stricter
 * than any rule in the system and would omit the corpus's single most-used relationship.
 *
 * So: declared edges first, relationships already in use beside them, and the reason each
 * one is offered in its detail text. This is the opposite of the commit vocabulary, which
 * *is* closed and *is* gated — and the difference is why one offers a squiggle and the
 * other does not.
 *
 * No `vscode` import.
 */

import type { Envelope } from './reports.ts'

export interface GraphLink {
  target: string
  relationship: string
  /** Corpus-relative, resolved by the CLI. Empty when it lands outside the corpus. */
  resolved: string
  exists: boolean
}

export interface GraphNode {
  /** Corpus-relative — this is a node's identity throughout. */
  node: string
  class: string
  label: string
  description: string
  links: GraphLink[]
}

export interface OntProperty {
  name: string
  type: string
  description: string
}

export interface OntEdge {
  relationship: string
  /** A class name, not a path. */
  target: string
  direction: string
  description: string
}

export interface GraphClass {
  class: string
  label: string
  description: string
  properties: OntProperty[]
  edges: OntEdge[]
}

export interface GraphReport extends Envelope {
  corpus_dir: string
  nodes: GraphNode[]
  classes: GraphClass[]
}

// ── reading the line under the cursor ────────────────────────────────────────

export interface Scalar {
  value: string
  /** 0-based character offsets of the value, so a range can be drawn on it. */
  start: number
  end: number
}

/**
 * The value of `key:` on this line, if the cursor is inside it.
 *
 * One line of text, not a YAML parse. Two reasons, and the second is the load-bearing one:
 * the buffer may differ from what the report was taken over, so a lookup by document
 * position has to read the document; and parsing corpus YAML in TypeScript would be a
 * second implementation of `parse_node`, which is one of the six parity functions.
 */
export function scalarAt(line: string, character: number, key: string): Scalar | null {
  const m = new RegExp(`^(\\s*(?:-\\s*)?${key}:\\s*)(.*)$`).exec(line)
  if (!m) return null
  const start = m[1].length
  let value = m[2]
  // Trailing comment, then trailing space. A quoted value keeps its quotes out of the range
  // so the path is what gets joined.
  const hash = value.indexOf(' #')
  if (hash !== -1) value = value.slice(0, hash)
  value = value.replace(/\s+$/, '')
  if (value.length === 0) return null
  let offset = 0
  if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
    if (value.length < 2) return null
    offset = 1
    value = value.slice(1, -1)
  }
  const from = start + offset
  const to = from + value.length
  // Inclusive of both ends: clicking just past the last character is still the value.
  if (character < from || character > to) return null
  return { value, start: from, end: to }
}

/**
 * `concept/tailwater.yml` + `../gauge/g.yml` → `gauge/g.yml`.
 *
 * Corpus-relative in, corpus-relative out — the same identity the report uses. Mirrors the
 * CLI's `normalize(dir.join(target))`; a `..` that climbs above the corpus root yields the
 * empty string, which is what the CLI reports for the same case.
 */
export function resolveFrom(nodeId: string, target: string): string {
  const dir = nodeId.slice(0, nodeId.lastIndexOf('/') + 1)
  const parts: string[] = []
  for (const part of `${dir}${target}`.split('/')) {
    if (part === '' || part === '.') continue
    if (part === '..') {
      if (parts.length === 0) return ''
      parts.pop()
      continue
    }
    parts.push(part)
  }
  return parts.join('/')
}

/** `gauge/g.yml` seen from `concept/tailwater.yml` → `../gauge/g.yml`. */
export function relativeFrom(nodeId: string, targetId: string): string {
  const from = nodeId.split('/').slice(0, -1)
  const to = targetId.split('/')
  let i = 0
  while (i < from.length && i < to.length - 1 && from[i] === to[i]) i += 1
  const up = '../'.repeat(from.length - i)
  const down = to.slice(i).join('/')
  return `${up}${down}` || down
}

/**
 * The line holding `target: <raw>`, 0-based, or 0 when it is not found.
 *
 * A text scan, and only ever used to *place a cursor*. Falling back to line 0 rather than to
 * "no reference" is deliberate: the edge is in the report, so it is in the file, and pointing
 * at the top of the right file beats dropping a reference the graph says exists.
 */
export function lineOfTarget(text: string, rawTarget: string): number {
  const lines = text.split('\n')
  for (let i = 0; i < lines.length; i += 1) {
    const found = scalarAt(lines[i], -1, 'target')
    if (found === null) {
      // `scalarAt` gates on the cursor; re-read without one.
      const m = /^\s*(?:-\s*)?target:\s*(.*)$/.exec(lines[i])
      if (m && m[1].replace(/\s+$/, '').replace(/^["']|["']$/g, '') === rawTarget) return i
      continue
    }
    if (found.value === rawTarget) return i
  }
  return 0
}

// ── lookups ──────────────────────────────────────────────────────────────────

export function nodeById(graph: GraphReport, id: string): GraphNode | undefined {
  return graph.nodes.find((n) => n.node === id)
}

export interface Reference {
  /** The node holding the edge. */
  from: string
  relationship: string
  /** The raw target text, so the line can be found in the file. */
  target: string
}

/**
 * Every inbound edge to a node.
 *
 * Nothing surfaced reverse traversal for corpus nodes: `used-by` covers catalog entries
 * only, and `orphan-in` reports the *absence* of inbound edges without ever naming the
 * present ones.
 */
export function referencesTo(graph: GraphReport, id: string): Reference[] {
  const out: Reference[] = []
  for (const n of graph.nodes) {
    for (const l of n.links) {
      if (l.resolved === id) out.push({ from: n.node, relationship: l.relationship, target: l.target })
    }
  }
  return out
}

/** The hover for a link: what is on the other end, without leaving the node. */
export function hoverFor(graph: GraphReport, id: string): string | null {
  const node = nodeById(graph, id)
  if (!node) {
    // An `.ont.yml` is a legitimate target — every instance carries an `instance-of` edge
    // to one — and it is not a node, so say what it is rather than nothing.
    if (id.endsWith('.ont.yml')) {
      const cls = graph.classes.find((c) => `${c.class}.ont.yml` === id)
      if (cls) return `**${cls.label || cls.class}** — \`${id}\`\n\n${cls.description}`
    }
    return null
  }
  const inbound = referencesTo(graph, id).length
  const meta = `\`${node.class}\` · ${node.links.length} out · ${inbound} in`
  return `**${node.label || node.node}** — ${meta}\n\n${node.description}`
}

// ── completion ───────────────────────────────────────────────────────────────

export interface Candidate {
  label: string
  detail: string
  documentation: string
  /** 0 sorts first. Declared beats observed beats everything. */
  rank: number
}

function sortKey(c: Candidate, i: number): string {
  return `${c.rank}${String(i).padStart(4, '0')}`
}

export function sorted(cs: Candidate[]): (Candidate & { sortText: string })[] {
  return cs
    .slice()
    .sort((a, b) => a.rank - b.rank || a.label.localeCompare(b.label))
    .map((c, i) => ({ ...c, sortText: sortKey(c, i) }))
}

/**
 * Out-edges the class declares, then relationships its instances already use.
 *
 * **One relationship may be declared against several classes.** A real ontology has
 * `maneuver -[operates-on]->` legislation, ballot-measure *and* election as three separate
 * declarations; three classes there do it. They are one offer with three destinations, and
 * treating each declaration as its own candidate put `concerns` in the list twice.
 */
export function declaredOut(graph: GraphReport, cls: string, relationship?: string): OntEdge[] {
  return (graph.classes.find((c) => c.class === cls)?.edges ?? []).filter(
    (e) => e.direction !== 'in' && (relationship === undefined || e.relationship === relationship),
  )
}

export function relationshipCandidates(graph: GraphReport, cls: string): Candidate[] {
  const byRelationship = new Map<string, OntEdge[]>()
  for (const e of declaredOut(graph, cls)) {
    byRelationship.set(e.relationship, [...(byRelationship.get(e.relationship) ?? []), e])
  }
  const out: Candidate[] = [...byRelationship.entries()].map(([relationship, edges]) => ({
    label: relationship,
    detail: edges.some((e) => e.target)
      ? `→ ${edges.map((e) => e.target).filter(Boolean).join(' | ')}`
      : 'declared',
    documentation: edges
      .map((e) => (edges.length > 1 && e.target ? `**${e.target}** — ${e.description}` : e.description))
      .filter(Boolean)
      .join('\n\n'),
    rank: 0,
  }))

  const named = new Set(out.map((c) => c.label))
  const inUse = new Map<string, number>()
  for (const n of graph.nodes) {
    if (n.class !== cls) continue
    for (const l of n.links) {
      if (!l.relationship || named.has(l.relationship)) continue
      inUse.set(l.relationship, (inUse.get(l.relationship) ?? 0) + 1)
    }
  }
  for (const [relationship, count] of inUse) {
    out.push({
      label: relationship,
      detail: `in use — ${count}×`,
      documentation:
        `Used by ${count} \`${cls}\` node(s) and not declared in \`${cls}.ont.yml\`. ` +
        'Nothing gates relationships against the ontology, so this is offered rather than ' +
        'hidden — but it is worth knowing the ontology does not describe it.',
      rank: 1,
    })
  }
  return out
}

/**
 * What this edge may point at, as a path relative to the editing node.
 *
 * Three tiers, most specific first. The fallback is *everything*: an editor that offered
 * nothing because the ontology was silent would be worse than one that offered too much,
 * and the ontology is silent for 17 of the pairs a real corpus uses.
 */
export function targetCandidates(graph: GraphReport, fromId: string, relationship: string): Candidate[] {
  const from = nodeById(graph, fromId)
  const cls = from?.class ?? ''
  // Every declaration, not the first. `maneuver -[operates-on]->` is declared against three
  // classes, and taking one of them would silently hide two thirds of the legal targets —
  // which reads to a user as "those nodes do not exist".
  const declared = declaredOut(graph, cls, relationship)

  const candidates = new Map<string, Candidate>()
  const offer = (id: string, detail: string, documentation: string, rank: number) => {
    if (id === fromId) return
    const existing = candidates.get(id)
    if (existing && existing.rank <= rank) return
    candidates.set(id, { label: relativeFrom(fromId, id), detail, documentation, rank })
  }

  for (const edge of declared) {
    if (!edge.target) continue
    for (const n of graph.nodes.filter((n) => n.class === edge.target)) {
      offer(n.node, `${n.label || n.node} · ${edge.target}`, edge.description, 0)
    }
  }

  // What the corpus already does with this relationship. Catches `instance-of`, which every
  // node carries, no ontology declares, and which points at an `.ont.yml` rather than at an
  // instance — so no class filter could ever have found it.
  for (const n of graph.nodes) {
    if (n.class !== cls) continue
    for (const l of n.links) {
      if (l.relationship !== relationship || !l.resolved) continue
      const target = nodeById(graph, l.resolved)
      if (target) {
        for (const sibling of graph.nodes.filter((s) => s.class === target.class)) {
          offer(sibling.node, `${sibling.label || sibling.node} · ${target.class}`, '', 1)
        }
      } else {
        offer(l.resolved, 'in use', `Targeted by ${n.node} under the same relationship.`, 1)
      }
    }
  }

  if (candidates.size === 0 && declared.length === 0) {
    for (const n of graph.nodes) {
      offer(n.node, `${n.label || n.node} · ${n.class}`, '', 2)
    }
  }
  return [...candidates.values()]
}

// ── new node ─────────────────────────────────────────────────────────────────

export interface NewNode {
  class: string
  /** Filename stem, kebab-case. */
  name: string
  label: string
  description: string
  relationship: string
  /** Corpus-relative id of the target. */
  target: string
}

/** `Tailwater regime` → `tailwater-regime`. Filenames are stable identity; keep them plain. */
export function slugify(text: string): string {
  return text
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/**
 * The file, scaffolded from the class's declared properties.
 *
 * Written only once a link has been chosen, which is why `relationship` and `target` are
 * required fields of the input rather than optional ones: a node with no outgoing edge is a
 * lint error the moment it exists, and offering to create one is offering to break the gate.
 */
export function scaffold(input: NewNode, cls: GraphClass | undefined): string {
  const id = `${input.class}/${input.name}.yml`
  const lines = [
    `class: ${input.class}`,
    `label: ${input.label}`,
    `description: ${JSON.stringify(input.description)}`,
  ]
  const props = cls?.properties ?? []
  if (props.length > 0) {
    lines.push('properties:')
    for (const p of props) {
      lines.push(`  # ${p.description}`)
      lines.push(`  ${p.name}: ""   # ${p.type}`)
    }
  }
  lines.push('links:')
  lines.push(`  - target: ${relativeFrom(id, input.target)}`)
  lines.push(`    relationship: ${input.relationship}`)
  return `${lines.join('\n')}\n`
}
