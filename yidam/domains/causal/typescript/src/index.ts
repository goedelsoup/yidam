export function ate(treated: number[], control: number[]): number {
  if (treated.length === 0 || control.length === 0) return 0
  const tMean = treated.reduce((s, x) => s + x, 0) / treated.length
  const cMean = control.reduce((s, x) => s + x, 0) / control.length
  return tMean - cMean
}

export function confoundingScore(rTreatment: number, rOutcome: number): number {
  return Math.abs(rTreatment * rOutcome)
}
