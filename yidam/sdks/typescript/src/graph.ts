/** A directed edge between two nodes, identified by corpus path. */
export interface GraphEdge {
  from: string
  to: string
}

/**
 * Code-point order, which `Array.prototype.sort` is not.
 *
 * The default comparator orders by UTF-16 code unit, and an astral character is a surrogate
 * pair beginning in D800–DBFF — below every BMP character from E000 up. Rust sorts `String`
 * by UTF-8 bytes and Python by code point, and those two agree with each other everywhere,
 * so JavaScript is the one runtime that would answer a sorted contract differently. It takes
 * a path pairing an emoji with halfwidth katakana to see it, which is what
 * `astral-and-bmp.toml` is; every ASCII fixture passes with either comparator, and that is
 * exactly the shape a divergence hides in.
 */
function byCodePoint(a: string, b: string): number {
  const ac = Array.from(a)
  const bc = Array.from(b)
  const n = Math.min(ac.length, bc.length)
  for (let i = 0; i < n; i++) {
    const d = (ac[i].codePointAt(0) as number) - (bc[i].codePointAt(0) as number)
    if (d !== 0) return d
  }
  return ac.length - bc.length
}

/**
 * All nodes reachable from `nodePath` following directed edges (BFS).
 * The start node is not included. Result is sorted for determinism.
 */
export function findReachable(edges: GraphEdge[], nodePath: string): string[] {
  const visited = new Set<string>([nodePath])
  const queue: string[] = [nodePath]
  const reachable: string[] = []
  while (queue.length > 0) {
    const current = queue.shift() as string
    for (const edge of edges) {
      if (edge.from === current && !visited.has(edge.to)) {
        visited.add(edge.to)
        reachable.push(edge.to)
        queue.push(edge.to)
      }
    }
  }
  return reachable.sort(byCodePoint)
}

/**
 * All nodes that have a directed edge pointing to `nodePath`.
 * Result is sorted and deduplicated for determinism.
 */
export function findCitations(edges: GraphEdge[], nodePath: string): string[] {
  const citations = edges.filter(e => e.to === nodePath).map(e => e.from)
  citations.sort(byCodePoint)
  return citations.filter((c, i) => i === 0 || c !== citations[i - 1])
}
