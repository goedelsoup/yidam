import math
import tomllib
from pathlib import Path
from yidam_domain_finance import present_value, future_value, simple_interest, sharpe_ratio

FIXTURES_DIR = Path(__file__).parent.parent.parent.parent.parent / "parity" / "fixtures"
# Relative, with an absolute floor in the input's currency.
# Why this domain is not exact: finance/README.md#parity-tolerance
REL_TOL = 1e-12
ABS_TOL = 1e-9

def load_fixtures(function: str) -> list[dict]:
    d = FIXTURES_DIR / function
    if not d.exists():
        return []
    out = []
    for p in sorted(d.glob("*.toml")):
        with open(p, "rb") as f:
            out.append(tomllib.load(f))
    return out

def test_parity_present_value():
    fixtures = load_fixtures("finance.present_value")
    assert fixtures, "no finance.present_value fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = present_value(inp["fv"], inp["rate"], inp["periods"])
        assert math.isclose(result, fx["expected"]["pv"], rel_tol=REL_TOL, abs_tol=ABS_TOL)

def test_parity_future_value():
    fixtures = load_fixtures("finance.future_value")
    assert fixtures, "no finance.future_value fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = future_value(inp["pv"], inp["rate"], inp["periods"])
        assert math.isclose(result, fx["expected"]["fv"], rel_tol=REL_TOL, abs_tol=ABS_TOL)

def test_parity_simple_interest():
    fixtures = load_fixtures("finance.simple_interest")
    assert fixtures, "no finance.simple_interest fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = simple_interest(inp["principal"], inp["rate"], inp["time"])
        assert math.isclose(result, fx["expected"]["interest"], rel_tol=REL_TOL, abs_tol=ABS_TOL)

def test_parity_sharpe_ratio():
    fixtures = load_fixtures("finance.sharpe_ratio")
    assert fixtures, "no finance.sharpe_ratio fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = sharpe_ratio(inp["ret"], inp["risk_free"], inp["std_dev"])
        assert math.isclose(result, fx["expected"]["ratio"], rel_tol=REL_TOL, abs_tol=ABS_TOL)
