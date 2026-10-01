export function union(a: number[], b: number[]): number[] {
  return [...new Set([...a, ...b])].sort((x, y) => x - y)
}

export function intersection(a: number[], b: number[]): number[] {
  const bs = new Set(b)
  return [...new Set(a.filter(x => bs.has(x)))].sort((x, y) => x - y)
}

export function difference(a: number[], b: number[]): number[] {
  const bs = new Set(b)
  return [...new Set(a.filter(x => !bs.has(x)))].sort((x, y) => x - y)
}

export function isSubset(a: number[], b: number[]): boolean {
  const bs = new Set(b)
  return a.every(x => bs.has(x))
}
