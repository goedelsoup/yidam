function gcd(a: number, b: number): number {
  a = Math.abs(a)
  b = Math.abs(b)
  while (b !== 0) {
    const t = b
    b = a % b
    a = t
  }
  return a
}

export function modularAdd(a: number, b: number, n: number): number {
  return ((a % n) + (b % n) + n) % n
}

export function modularMul(a: number, b: number, n: number): number {
  return ((a % n) * (b % n) + n * n) % n
}

export function additiveOrder(a: number, n: number): number {
  a = ((a % n) + n) % n
  if (a === 0) return 1
  return n / gcd(a, n)
}
