pub fn ate(treated: &[f64], control: &[f64]) -> f64 {
    if treated.is_empty() || control.is_empty() {
        return 0.0;
    }
    let t_mean = treated.iter().sum::<f64>() / treated.len() as f64;
    let c_mean = control.iter().sum::<f64>() / control.len() as f64;
    t_mean - c_mean
}

pub fn confounding_score(r_treatment: f64, r_outcome: f64) -> f64 {
    (r_treatment * r_outcome).abs()
}
