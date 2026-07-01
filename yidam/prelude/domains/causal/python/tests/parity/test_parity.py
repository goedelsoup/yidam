import tomllib
from pathlib import Path
from yidam_domain_causal import ate, confounding_score

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

def test_parity_ate():
    fixtures = load_fixtures("causal.ate")
    assert fixtures, "no causal.ate fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = ate(inp["treated"], inp["control"])
        assert result == fx["expected"]["ate"]

def test_parity_confounding_score():
    fixtures = load_fixtures("causal.confounding_score")
    assert fixtures, "no causal.confounding_score fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = confounding_score(inp["r_treatment"], inp["r_outcome"])
        assert result == fx["expected"]["score"]
