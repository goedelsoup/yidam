export function presentValue(fv: number, rate: number, periods: number): number {
  return fv / Math.pow(1 + rate, periods)
}

export function futureValue(pv: number, rate: number, periods: number): number {
  return pv * Math.pow(1 + rate, periods)
}

export function simpleInterest(principal: number, rate: number, time: number): number {
  return principal * rate * time
}

export function sharpeRatio(ret: number, riskFree: number, stdDev: number): number {
  return stdDev === 0 ? 0 : (ret - riskFree) / stdDev
}
