def gdp_expenditure(c: float, i: float, g: float, nx: float) -> float:
    return c + i + g + nx

def price_elasticity(pct_qty_change: float, pct_price_change: float) -> float:
    return 0.0 if pct_price_change == 0.0 else pct_qty_change / pct_price_change

def opportunity_cost(foregone: float, chosen: float) -> float:
    return foregone - chosen
