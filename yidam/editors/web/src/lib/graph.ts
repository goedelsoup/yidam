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
 * A property a class declares — and exactly the three fields the envelope carries.
 *
 * **There is no `required` here, and its absence is the point.** `OntProperty` in
 * `yidam/cli/src/cmd/graph.rs:48-53` serialises `name`, `type` and `description` and nothing
 * else, while `missing-property`'s own rationale says a property declared `required: true`
 * *gates*. So the one distinction between a property whose omission fails CI and one whose
 * omission is reported and forgiven does not survive into `graph --format json`.
 *
 * A first draft of the node page rendered a `Declared` column from this and printed
 * "optional" for every property in the fixture, including any that were not — a fabricated
 * verdict, arriving as a table column rather than as a check. Adding the field to this
 * interface would make that mistake available again, so it is not here. The fix is on the
 * CLI's side and is recorded on #606.
 */
export interface GraphProperty {
  name: string
  type?: string
  description?: string
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
