def present_value(fv: float, rate: float, periods: int) -> float:
    return fv / (1 + rate) ** periods

def future_value(pv: float, rate: float, periods: int) -> float:
    return pv * (1 + rate) ** periods

def simple_interest(principal: float, rate: float, time: float) -> float:
    return principal * rate * time

def sharpe_ratio(ret: float, risk_free: float, std_dev: float) -> float:
    return 0.0 if std_dev == 0.0 else (ret - risk_free) / std_dev
