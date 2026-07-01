pub fn modular_add(a: i64, b: i64, n: i64) -> i64 {
    ((a % n) + (b % n)).rem_euclid(n)
}

pub fn modular_mul(a: i64, b: i64, n: i64) -> i64 {
    ((a % n) * (b % n)).rem_euclid(n)
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

pub fn additive_order(a: i64, n: i64) -> i64 {
    let a = a.rem_euclid(n);
    if a == 0 {
        return 1;
    }
    n / gcd(a, n)
}
