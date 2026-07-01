import tomllib
from pathlib import Path
from yidam_domain_hydrology import rational_product, manning_velocity, return_period

FIXTURES_DIR = Path(__file__).parent.parent.parent.parent.parent / "parity" / "fixtures"
EPSILON = 1e-9

def load_fixtures(function: str) -> list[dict]:
    d = FIXTURES_DIR / function
    if not d.exists():
        return []
    out = []
    for p in sorted(d.glob("*.toml")):
        with open(p, "rb") as f:
            out.append(tomllib.load(f))
    return out

def test_parity_rational_product():
    fixtures = load_fixtures("hydrology.rational_product")
    assert fixtures, "no hydrology.rational_product fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = rational_product(inp["c"], inp["i"], inp["a"])
        assert abs(result - fx["expected"]["result"]) < EPSILON

def test_parity_manning_velocity():
    fixtures = load_fixtures("hydrology.manning_velocity")
    assert fixtures, "no hydrology.manning_velocity fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = manning_velocity(inp["n"], inp["r"], inp["s"])
        assert abs(result - fx["expected"]["velocity"]) < EPSILON

def test_parity_return_period():
    fixtures = load_fixtures("hydrology.return_period")
    assert fixtures, "no hydrology.return_period fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = return_period(inp["record_years"], inp["rank"])
        assert abs(result - fx["expected"]["years"]) < EPSILON
