export function gdpExpenditure(c: number, i: number, g: number, nx: number): number {
  return c + i + g + nx
}

export function priceElasticity(pctQtyChange: number, pctPriceChange: number): number {
  return pctPriceChange === 0 ? 0 : pctQtyChange / pctPriceChange
}

export function opportunityCost(foregone: number, chosen: number): number {
  return foregone - chosen
}
