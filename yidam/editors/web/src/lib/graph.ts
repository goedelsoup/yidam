/**
 * The corpus graph, as the CLI reported it.
 *
 * Shapes and traversal, and no judgement of any kind. Every verdict on this page was made by
 * the binary before this process saw it: `resolved` is the CLI's own path resolution and
 * `exists` is answered by the same test `dangling_edge` gates on, so a broken edge renders as
 * broken here because `yidam graph` said so. `test/boundary.mjs` is what holds that, and
 * `src/lib/cli.ts` is the only door the report comes through.
 *
 * The field names are off `yidam graph --format json` and off the extension's `reports.ts`,
 * not out of a guess. Three of them were guessed once and all three were wrong — `node` rather
 * than `id` is the one that rendered a column of empty cells against a corpus with eight nodes
 * in it, which is how a guess in this position fails: quietly, and looking like an empty
 * corpus.
 *
 * `referencesTo` is reverse traversal, which the extension's `graph.ts` already carries and
 * argues for: `used-by` covers catalog entries only, and `orphan-in` reports the *absence* of
 * inbound edges without ever naming the present ones — so nothing in this system tells a
 * reader what points at the node they are looking at. It inverts an adjacency list the CLI
 * resolved. It does not resolve one, which is the whole difference between an affordance and
 * a verdict here.
 */

export interface GraphLink {
  /** The raw target text, as authored — relative to the node's own directory. */
  target: string
  relationship: string
  /** Corpus-relative, resolved by the CLI. Absent when it lands outside the corpus. */
  resolved?: string
  /** The CLI's answer, not this process's. */
  exists?: boolean
}

export interface GraphNode {
  /** Corpus-relative. This is a node's identity throughout, and it is `node`, not `id`. */
  node: string
  class?: string
  label?: string
  description?: string
  links?: GraphLink[]
}

/**
 * A property a class declares.
 *
 * `required` is the distinction `missing-property` gates on: a property declared `true` fails
 * the gate when an instance omits it, and every other omission is reported and forgiven.
 *
 * It did not reach this surface until the CLI change #606 carried. `OntProperty` serialised
 * `name`, `type` and `description` and stopped, so a first draft of the node page rendered a
 * required/optional column from a report that could not answer it and printed "optional" for
 * every property in the corpus — a fabricated verdict arriving as a table column, which is the
 * same failure as one arriving as a check and harder to see.
 *
 * **Optional here, and `undefined` does not mean optional.** A repository pins the binary that
 * governs it and this client is versioned independently, so a corpus on a binary older than
 * the field is a normal state rather than an exceptional one — and `format_version` does not
 * rise for an additive field, by `report.schema.json`'s own rule. `undefined` means *this
 * binary does not report requiredness*, and the page says that rather than guessing, because
 * guessing is what produced the column that had to be removed.
 */
export interface GraphProperty {
  name: string
  type?: string
  description?: string
  required?: boolean
}

/**
 * Whether the binary that answered reports requiredness at all.
 *
 * Asked of the whole set rather than per property, because it is a fact about the binary and
 * not about any one declaration: the CLI emits `required` on every property or on none. So the
 * column appears when the answer is available and is replaced by one sentence when it is not,
 * rather than every row carrying its own shrug.
 */
export function reportsRequired(properties: GraphProperty[]): boolean {
  return properties.length > 0 && properties.every((p) => typeof p.required === 'boolean')
}

export interface GraphClassEdge {
  relationship: string
  target?: string
  direction?: string
  description?: string
}

export interface GraphClass {
  class: string
  label?: string
  description?: string
  properties?: GraphProperty[]
  edges?: GraphClassEdge[]
}

export interface GraphReport {
  nodes?: GraphNode[]
  classes?: GraphClass[]
}

/** The node with this identity, or nothing. A miss is a real state — see `[...id].astro`. */
export function nodeById(nodes: GraphNode[], id: string): GraphNode | undefined {
  return nodes.find((n) => n.node === id)
}

/** The class declaration behind a node, for reading its properties beside its values. */
export function classOf(classes: GraphClass[], node: GraphNode): GraphClass | undefined {
  return node.class === undefined ? undefined : classes.find((c) => c.class === node.class)
}

export interface Reference {
  /** The node holding the edge. */
  from: string
  relationship: string
}

/**
 * Every inbound edge to a node, by inverting what the CLI already resolved.
 *
 * Matched on `resolved` rather than on `target`: `target` is relative to the *authoring*
 * node's directory, so two different nodes can carry the same `target` string and mean two
 * different files. Comparing the raw text would be this process doing the path resolution the
 * CLI is here to do — the exact re-derivation the gates exist to stop, arriving as a
 * three-line convenience.
 */
export function referencesTo(nodes: GraphNode[], id: string): Reference[] {
  const out: Reference[] = []
  for (const node of nodes) {
    for (const link of node.links ?? []) {
      if (link.resolved === id) out.push({ from: node.node, relationship: link.relationship })
    }
  }
  return out
}

/**
 * A node's URL.
 *
 * Segment by segment, because a node's identity is a path and `/` is what separates its parts
 * rather than something to escape. `encodeURIComponent` on the whole string would turn
 * `concept/low-flow.yml` into one segment containing `%2F`, which the rest route then reads
 * back as a single name that matches no node.
 */
export function nodeHref(id: string): string {
  return `/node/${id.split('/').map(encodeURIComponent).join('/')}`
}
