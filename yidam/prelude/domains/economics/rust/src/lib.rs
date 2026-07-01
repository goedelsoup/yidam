pub fn gdp_expenditure(c: f64, i: f64, g: f64, nx: f64) -> f64 {
    c + i + g + nx
}

pub fn price_elasticity(pct_qty_change: f64, pct_price_change: f64) -> f64 {
    if pct_price_change == 0.0 {
        0.0
    } else {
        pct_qty_change / pct_price_change
    }
}

pub fn opportunity_cost(foregone: f64, chosen: f64) -> f64 {
    foregone - chosen
}
