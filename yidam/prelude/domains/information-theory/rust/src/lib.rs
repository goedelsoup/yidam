pub fn entropy(probs: &[f64]) -> f64 {
    probs.iter()
        .filter(|&&p| p > 0.0)
        .map(|&p| -p * p.log2())
        .sum()
}

pub fn kl_divergence(p: &[f64], q: &[f64]) -> f64 {
    p.iter()
        .zip(q.iter())
        .filter(|(&pi, _)| pi > 0.0)
        .map(|(&pi, &qi)| pi * (pi / qi).log2())
        .sum()
}
