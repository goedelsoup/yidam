from math import log2

def entropy(probs: list[float]) -> float:
    return sum(-p * log2(p) for p in probs if p > 0.0)

def kl_divergence(p: list[float], q: list[float]) -> float:
    return sum(pi * log2(pi / qi) for pi, qi in zip(p, q) if pi > 0.0)
