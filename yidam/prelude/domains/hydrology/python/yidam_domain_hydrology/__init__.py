def rational_product(c: float, i: float, a: float) -> float:
    return c * i * a

def manning_velocity(n: float, r: float, s: float) -> float:
    return (1 / n) * (r ** (2 / 3)) * (s ** 0.5)

def return_period(record_years: float, rank: float) -> float:
    return (record_years + 1) / rank
