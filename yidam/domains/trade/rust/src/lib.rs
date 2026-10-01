pub fn trade_balance(exports: f64, imports: f64) -> f64 {
    exports - imports
}

pub fn terms_of_trade(export_index: f64, import_index: f64) -> f64 {
    if import_index == 0.0 {
        0.0
    } else {
        (export_index / import_index) * 100.0
    }
}

pub fn tariff_revenue(import_value: f64, rate: f64) -> f64 {
    import_value * rate
}

pub fn revealed_comparative_advantage(country_share: f64, world_share: f64) -> f64 {
    if world_share == 0.0 {
        0.0
    } else {
        country_share / world_share
    }
}
