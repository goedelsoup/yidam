export function rationalProduct(c: number, i: number, a: number): number {
  return c * i * a
}

export function manningVelocity(n: number, r: number, s: number): number {
  return (1 / n) * Math.pow(r, 2 / 3) * Math.sqrt(s)
}

export function returnPeriod(recordYears: number, rank: number): number {
  return (recordYears + 1) / rank
}
