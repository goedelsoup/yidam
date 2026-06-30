export function cosine(a: number[], b: number[]): number {
  const dot = a.reduce((s, x, i) => s + x * b[i], 0)
  const normA = Math.sqrt(a.reduce((s, x) => s + x * x, 0))
  const normB = Math.sqrt(b.reduce((s, x) => s + x * x, 0))
  if (normA === 0 || normB === 0) return 0
  return dot / (normA * normB)
}

export function jaccard(a: string[], b: string[]): number {
  const setA = new Set(a)
  const setB = new Set(b)
  const intersection = [...setA].filter(x => setB.has(x)).length
  const union = new Set([...a, ...b]).size
  if (union === 0) return 0
  return intersection / union
}

export function editDistance(s1: string, s2: string): number {
  const m = s1.length
  const n = s2.length
  const dp = Array.from({ length: n + 1 }, (_, i) => i)
  for (let i = 1; i <= m; i++) {
    let prev = dp[0]
    dp[0] = i
    for (let j = 1; j <= n; j++) {
      const temp = dp[j]
      dp[j] = s1[i - 1] === s2[j - 1] ? prev : 1 + Math.min(prev, dp[j], dp[j - 1])
      prev = temp
    }
  }
  return dp[n]
}
