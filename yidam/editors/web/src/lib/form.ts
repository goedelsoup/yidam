/**
 * A node form's affordances, generated from the ontology — RFC-0030 Phase 2, #607.
 *
 * Everything here is a suggestion. Which properties a class declares, which relationships
 * are offered and why, what a new node's path would be, and the YAML those answers become:
 * all of it is shape, and none of it is a verdict. The buffer this produces goes to the
 * overlay, and the overlay's diagnostics are the only judgement on the page. A form that
 * refused to submit an undeclared relationship would be a gate this repository does not
 * have — `yidam/editors/vscode/src/graph.ts` measured that against a live corpus: 17 of the
 * (class, relationship) pairs in use are undeclared, and `instance-of`, the edge every node
 * carries, is declared by none.
 *
 * So the relationship picker is three tiers with the reason on each, and a free-text escape:
 * declared by the class, then in use by the class's own instances, then in use anywhere. The
 * ontology is a guide, not a closed list; the commit vocabulary is closed and gated, and the
 * three claim tags are closed and gated, and those are the only two closed lists on this
 * surface. `property.type === 'claim'` is where the second one shows.
 *
 * `relativeFrom` and `slugify` are copied from the extension's `graph.ts` rather than
 * imported: `test/boundary.mjs` holds that `src/` reaches outside this package only for the
 * design system, and two twelve-line functions are cheaper than a third exception.
 */

import type { GraphClass, GraphClassEdge, GraphNode, GraphProperty } from './graph.ts'
import { emitNode } from './yaml.ts'

/** The closed list. Three tags, gated by `claim-tag`, and the form offers exactly these. */
export const CLAIM_TAGS = ['verified', 'inference', 'open'] as const

export interface Offer {
  relationship: string
  /** Why it is offered — shown beside the name, because the tier is the information. */
  reason: string
  /** 0 declared, 1 in use by this class, 2 in use elsewhere. */
  rank: 0 | 1 | 2
}

/** Out-edges the class declares, one per relationship however many targets it names. */
export function declaredOut(cls: GraphClass | undefined, relationship?: string): GraphClassEdge[] {
  return (cls?.edges ?? []).filter(
    (e) => e.direction !== 'in' && (relationship === undefined || e.relationship === relationship),
  )
}

/**
 * Declared edges first, relationships already in use beside them, the reason on each.
 *
 * Sorted by tier and then by name, so a declared `sources-from` sits above an undeclared one
 * of any count: what the ontology says comes before what the corpus does, and both come
 * before what some other class does.
 */
export function relationshipOffers(classes: GraphClass[], nodes: GraphNode[], className: string): Offer[] {
  const cls = classes.find((c) => c.class === className)
  const byRelationship = new Map<string, GraphClassEdge[]>()
  for (const e of declaredOut(cls)) {
    byRelationship.set(e.relationship, [...(byRelationship.get(e.relationship) ?? []), e])
  }
  const offers: Offer[] = [...byRelationship.entries()].map(([relationship, edges]) => {
    const targets = edges.map((e) => e.target).filter((t): t is string => Boolean(t))
    return {
      relationship,
      reason: targets.length > 0 ? `declared by ${className} → ${targets.join(' | ')}` : `declared by ${className}`,
      rank: 0,
    }
  })

  const own = new Map<string, number>()
  const elsewhere = new Map<string, number>()
  for (const n of nodes) {
    for (const l of n.links ?? []) {
      if (!l.relationship) continue
      const tally = n.class === className ? own : elsewhere
      tally.set(l.relationship, (tally.get(l.relationship) ?? 0) + 1)
    }
  }
  const named = new Set(offers.map((o) => o.relationship))
  // The one edge the form itself writes. Every node carries it and no ontology declares it,
  // so on a corpus whose report shows no links yet it would be offered by nobody — and the
  // default row would fall straight through to the free-text escape.
  if (!named.has('instance-of') && !own.has('instance-of')) {
    named.add('instance-of')
    offers.push({ relationship: 'instance-of', reason: 'the edge every node carries to its class file; declared by none', rank: 1 })
  }
  for (const [relationship, count] of own) {
    if (named.has(relationship)) continue
    named.add(relationship)
    offers.push({
      relationship,
      reason: `in use by ${count} ${className} ${count === 1 ? 'node' : 'nodes'}, not declared`,
      rank: 1,
    })
  }
  for (const [relationship, count] of elsewhere) {
    if (named.has(relationship)) continue
    named.add(relationship)
    offers.push({ relationship, reason: `in use by ${count} ${count === 1 ? 'node' : 'nodes'} of other classes`, rank: 2 })
  }
  return offers.sort((a, b) => a.rank - b.rank || a.relationship.localeCompare(b.relationship))
}

export interface TargetOffer {
  /** Corpus-relative id. */
  id: string
  detail: string
  rank: 0 | 1 | 2
}

/**
 * What an edge may point at, three tiers and a fallback of everything.
 *
 * Declared targets first — every declaration, since one relationship may be declared against
 * several classes. Then what this class's instances already point at under the relationship,
 * including the `.ont.yml` an `instance-of` reaches, which no class filter could find. Then,
 * when the ontology is silent, every node: an empty list reads as "those nodes do not exist".
 */
export function targetOffers(
  classes: GraphClass[],
  nodes: GraphNode[],
  className: string,
  relationship: string,
): TargetOffer[] {
  const cls = classes.find((c) => c.class === className)
  const declared = declaredOut(cls, relationship)
  const offers = new Map<string, TargetOffer>()
  const offer = (id: string, detail: string, rank: 0 | 1 | 2) => {
    const existing = offers.get(id)
    if (existing && existing.rank <= rank) return
    offers.set(id, { id, detail, rank })
  }
  if (relationship === 'instance-of') offer(`${className}.ont.yml`, 'this class', 0)
  for (const edge of declared) {
    if (!edge.target) continue
    for (const n of nodes) {
      if (n.class === edge.target) offer(n.node, `${n.label || n.node} · ${edge.target}`, 0)
    }
  }
  for (const n of nodes) {
    if (n.class !== className) continue
    for (const l of n.links ?? []) {
      if (l.relationship !== relationship || !l.resolved) continue
      const target = nodes.find((t) => t.node === l.resolved)
      if (target) {
        for (const sibling of nodes) {
          if (sibling.class === target.class) offer(sibling.node, `${sibling.label || sibling.node} · ${target.class}`, 1)
        }
      } else {
        offer(l.resolved, 'in use', 1)
      }
    }
  }
  if (offers.size === 0 && declared.length === 0) {
    for (const n of nodes) offer(n.node, `${n.label || n.node} · ${n.class ?? ''}`, 2)
  }
  return [...offers.values()].sort((a, b) => a.rank - b.rank || a.id.localeCompare(b.id))
}

/** `Tailwater regime` → `tailwater-regime`. Filenames are stable identity; keep them plain. */
export function slugify(text: string): string {
  return text
    .toLowerCase()
    .normalize('NFKD')
    // The combining marks NFKD split off, so `régime` is `regime` and not `re-gime`.
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/** A target as it is written in the file: relative to the authoring node's directory. */
export function relativeFrom(nodeId: string, targetId: string): string {
  const from = nodeId.split('/').slice(0, -1)
  const to = targetId.split('/')
  let i = 0
  while (i < from.length && i < to.length - 1 && from[i] === to[i]) i += 1
  const up = '../'.repeat(from.length - i)
  const down = to.slice(i).join('/')
  return `${up}${down}` || down
}

export interface DraftLink {
  relationship: string
  /**
   * A corpus-relative id when it came from the picker, or whatever was typed. Either way it
   * is written relative to the new node — the picker's ids are the graph's, and a typed path
   * is the author's, and the file gets what the author meant.
   */
  target: string
  /** Whether `target` is a corpus-relative id (to be made relative) or already a raw path. */
  raw?: boolean
}

export interface Draft {
  class: string
  /** Filename stem. */
  name: string
  label: string
  description: string
  properties: Record<string, string>
  links: DraftLink[]
}

/** Where the node would live. The doc id the overlay is told about. */
export function docOf(draft: Draft): string {
  return `${draft.class}/${draft.name || 'untitled'}.yml`
}

/** The edge every node carries and no ontology declares: to its own class file. */
export function instanceOf(className: string): DraftLink {
  return { relationship: 'instance-of', target: `${className}.ont.yml` }
}

/** A fresh draft of a class, as its declared properties and its one universal edge. */
export function blankDraft(className: string): Draft {
  return { class: className, name: '', label: '', description: '', properties: {}, links: [instanceOf(className)] }
}

/**
 * The buffer. Properties in the class's declared order, then any the class does not declare;
 * empty ones are left out so `missing-property` says which are missing rather than the form
 * papering over them with `""`.
 */
export function render(draft: Draft, properties: GraphProperty[]): string {
  const doc = docOf(draft)
  const ordered: [string, string][] = []
  const seen = new Set<string>()
  for (const p of properties) {
    seen.add(p.name)
    const value = draft.properties[p.name] ?? ''
    if (value.trim() !== '') ordered.push([p.name, value])
  }
  for (const [name, value] of Object.entries(draft.properties)) {
    if (!seen.has(name) && value.trim() !== '') ordered.push([name, value])
  }
  return emitNode({
    class: draft.class,
    label: draft.label,
    description: draft.description,
    properties: ordered,
    links: draft.links
      .filter((l) => l.relationship.trim() !== '' || l.target.trim() !== '')
      .map((l) => ({ relationship: l.relationship, target: l.raw ? l.target : relativeFrom(doc, l.target) })),
  })
}
