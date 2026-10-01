export function tradeBalance(exports: number, imports: number): number {
  return exports - imports
}

export function termsOfTrade(exportIndex: number, importIndex: number): number {
  return importIndex === 0 ? 0 : (exportIndex / importIndex) * 100
}

export function tariffRevenue(importValue: number, rate: number): number {
  return importValue * rate
}

export function revealedComparativeAdvantage(countryShare: number, worldShare: number): number {
  return worldShare === 0 ? 0 : countryShare / worldShare
}
