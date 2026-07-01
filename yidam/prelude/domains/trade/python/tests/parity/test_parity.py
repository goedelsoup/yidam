import tomllib
from pathlib import Path
from yidam_domain_trade import trade_balance, terms_of_trade, tariff_revenue, revealed_comparative_advantage

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

def test_parity_trade_balance():
    fixtures = load_fixtures("trade.trade_balance")
    assert fixtures, "no trade.trade_balance fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert trade_balance(inp["exports"], inp["imports"]) == fx["expected"]["balance"]

def test_parity_terms_of_trade():
    fixtures = load_fixtures("trade.terms_of_trade")
    assert fixtures, "no trade.terms_of_trade fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert terms_of_trade(inp["export_index"], inp["import_index"]) == fx["expected"]["tot"]

def test_parity_tariff_revenue():
    fixtures = load_fixtures("trade.tariff_revenue")
    assert fixtures, "no trade.tariff_revenue fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert tariff_revenue(inp["import_value"], inp["rate"]) == fx["expected"]["revenue"]

def test_parity_revealed_comparative_advantage():
    fixtures = load_fixtures("trade.revealed_comparative_advantage")
    assert fixtures, "no trade.revealed_comparative_advantage fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert revealed_comparative_advantage(inp["country_share"], inp["world_share"]) == fx["expected"]["rca"]
