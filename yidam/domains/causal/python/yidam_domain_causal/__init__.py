def ate(treated: list[float], control: list[float]) -> float:
    if not treated or not control:
        return 0.0
    return sum(treated) / len(treated) - sum(control) / len(control)

def confounding_score(r_treatment: float, r_outcome: float) -> float:
    return abs(r_treatment * r_outcome)
