import tomllib
from pathlib import Path
from yidam_domain_statistics import mean, variance, z_score, pearson_correlation

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

def test_parity_mean():
    fixtures = load_fixtures("statistics.mean")
    assert fixtures, "no statistics.mean fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = mean(inp["values"])
        assert result == fx["expected"]["mean"]

def test_parity_variance():
    fixtures = load_fixtures("statistics.variance")
    assert fixtures, "no statistics.variance fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = variance(inp["values"])
        assert result == fx["expected"]["variance"]

def test_parity_z_score():
    fixtures = load_fixtures("statistics.z_score")
    assert fixtures, "no statistics.z_score fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = z_score(inp["value"], inp["mean"], inp["std_dev"])
        assert result == fx["expected"]["z_score"]

def test_parity_pearson_correlation():
    fixtures = load_fixtures("statistics.pearson_correlation")
    assert fixtures, "no statistics.pearson_correlation fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = pearson_correlation(inp["xs"], inp["ys"])
        assert result == fx["expected"]["r"]
