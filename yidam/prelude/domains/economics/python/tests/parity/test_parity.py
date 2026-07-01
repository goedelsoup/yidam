import tomllib
from pathlib import Path
from yidam_domain_economics import gdp_expenditure, price_elasticity, opportunity_cost

FIXTURES_DIR = Path(__file__).parent.parent.parent.parent.parent / "parity" / "fixtures"

def load_fixtures(function: str) -> list[dict]:
    d = FIXTURES_DIR / function
    if not d.exists():
        return []
    out = []
    for p in sorted(d.glob("*.toml")):
        with open(p, "rb") as f:
            out.append(tomllib.load(f))
    return out

def test_parity_gdp_expenditure():
    fixtures = load_fixtures("economics.gdp_expenditure")
    assert fixtures, "no economics.gdp_expenditure fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert gdp_expenditure(inp["c"], inp["i"], inp["g"], inp["nx"]) == fx["expected"]["gdp"]

def test_parity_price_elasticity():
    fixtures = load_fixtures("economics.price_elasticity")
    assert fixtures, "no economics.price_elasticity fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert price_elasticity(inp["pct_qty_change"], inp["pct_price_change"]) == fx["expected"]["elasticity"]

def test_parity_opportunity_cost():
    fixtures = load_fixtures("economics.opportunity_cost")
    assert fixtures, "no economics.opportunity_cost fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert opportunity_cost(inp["foregone"], inp["chosen"]) == fx["expected"]["cost"]
