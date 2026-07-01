export function density(nodeCount: number, edgeCount: number): number {
  if (nodeCount < 2) return 0
  const maxEdges = (nodeCount * (nodeCount - 1)) / 2
  return edgeCount / maxEdges
}

export function degreeCentrality(degrees: number[], nodeCount: number): number[] {
  if (nodeCount < 2) return degrees.map(() => 0)
  const n = nodeCount - 1
  return degrees.map(d => d / n)
}
