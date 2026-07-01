export function entropy(probs: number[]): number {
  return probs.reduce((sum, p) => sum + (p > 0 ? -p * Math.log2(p) : 0), 0)
}

export function klDivergence(p: number[], q: number[]): number {
  return p.reduce((sum, pi, i) => sum + (pi > 0 ? pi * Math.log2(pi / q[i]) : 0), 0)
}
