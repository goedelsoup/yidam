pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let m = mean(values);
    values.iter().map(|&x| (x - m).powi(2)).sum::<f64>() / values.len() as f64
}

pub fn z_score(value: f64, mean: f64, std_dev: f64) -> f64 {
    (value - mean) / std_dev
}

pub fn pearson_correlation(xs: &[f64], ys: &[f64]) -> f64 {
    if xs.len() < 2 || xs.len() != ys.len() { return 0.0; }
    let mx = mean(xs);
    let my = mean(ys);
    let n = xs.len() as f64;
    let cov: f64 = xs.iter().zip(ys.iter()).map(|(&x, &y)| (x - mx) * (y - my)).sum::<f64>() / n;
    let sx = variance(xs).sqrt();
    let sy = variance(ys).sqrt();
    if sx == 0.0 || sy == 0.0 { return 0.0; }
    cov / (sx * sy)
}
