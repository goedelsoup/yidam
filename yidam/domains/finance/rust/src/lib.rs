pub fn present_value(fv: f64, rate: f64, periods: u32) -> f64 {
    fv / (1.0 + rate).powi(periods as i32)
}

pub fn future_value(pv: f64, rate: f64, periods: u32) -> f64 {
    pv * (1.0 + rate).powi(periods as i32)
}

pub fn simple_interest(principal: f64, rate: f64, time: f64) -> f64 {
    principal * rate * time
}

pub fn sharpe_ratio(ret: f64, risk_free: f64, std_dev: f64) -> f64 {
    if std_dev == 0.0 {
        0.0
    } else {
        (ret - risk_free) / std_dev
    }
}
