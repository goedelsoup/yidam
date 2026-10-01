pub fn rational_product(c: f64, i: f64, a: f64) -> f64 {
    c * i * a
}

pub fn manning_velocity(n: f64, r: f64, s: f64) -> f64 {
    (1.0 / n) * r.powf(2.0 / 3.0) * s.sqrt()
}

pub fn return_period(record_years: f64, rank: f64) -> f64 {
    (record_years + 1.0) / rank
}
