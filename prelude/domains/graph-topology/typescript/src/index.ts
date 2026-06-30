export function clusteringCoefficient(degree: number, triangleCount: number): number {
  if (degree < 2) return 0
  const possible = degree * (degree - 1) / 2
  return triangleCount / possible
}

export function connectedComponents(nodeCount: number, edges: [number, number][]): number {
  if (nodeCount === 0) return 0
  const parent = Array.from({ length: nodeCount }, (_, i) => i)

  function find(x: number): number {
    while (parent[x] !== x) {
      parent[x] = parent[parent[x]]
      x = parent[x]
    }
    return x
  }

  for (const [u, v] of edges) {
    const a = find(u)
    const b = find(v)
    if (a !== b) parent[a] = b
  }

  let count = 0
  for (let i = 0; i < nodeCount; i++) {
    if (find(i) === i) count++
  }
  return count
}
