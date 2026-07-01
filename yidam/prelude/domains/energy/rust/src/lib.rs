pub fn kinetic_energy(mass: f64, velocity: f64) -> f64 {
    0.5 * mass * velocity * velocity
}

pub fn potential_energy(mass: f64, height: f64, g: f64) -> f64 {
    mass * g * height
}

pub fn power(work: f64, time: f64) -> f64 {
    if time == 0.0 { 0.0 } else { work / time }
}

pub fn efficiency(output: f64, input: f64) -> f64 {
    if input == 0.0 { 0.0 } else { output / input }
}
