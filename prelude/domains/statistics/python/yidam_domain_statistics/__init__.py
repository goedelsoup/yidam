from math import sqrt

def mean(values: list[float]) -> float:
    if not values:
        return 0.0
    return sum(values) / len(values)

def variance(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    m = mean(values)
    return sum((x - m) ** 2 for x in values) / len(values)

def z_score(value: float, mean: float, std_dev: float) -> float:
    return (value - mean) / std_dev

def pearson_correlation(xs: list[float], ys: list[float]) -> float:
    if len(xs) < 2 or len(xs) != len(ys):
        return 0.0
    mx = mean(xs)
    my = mean(ys)
    n = len(xs)
    cov = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / n
    sx = sqrt(variance(xs))
    sy = sqrt(variance(ys))
    if sx == 0.0 or sy == 0.0:
        return 0.0
    return cov / (sx * sy)
