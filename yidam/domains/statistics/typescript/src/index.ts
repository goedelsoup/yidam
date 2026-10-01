export function mean(values: number[]): number {
  if (values.length === 0) return 0
  return values.reduce((s, x) => s + x, 0) / values.length
}

export function variance(values: number[]): number {
  if (values.length < 2) return 0
  const m = mean(values)
  return values.reduce((s, x) => s + (x - m) ** 2, 0) / values.length
}

export function zScore(value: number, mean: number, stdDev: number): number {
  return (value - mean) / stdDev
}

export function pearsonCorrelation(xs: number[], ys: number[]): number {
  if (xs.length < 2 || xs.length !== ys.length) return 0
  const mx = mean(xs)
  const my = mean(ys)
  const n = xs.length
  const cov = xs.reduce((s, x, i) => s + (x - mx) * (ys[i] - my), 0) / n
  const sx = Math.sqrt(variance(xs))
  const sy = Math.sqrt(variance(ys))
  if (sx === 0 || sy === 0) return 0
  return cov / (sx * sy)
}
