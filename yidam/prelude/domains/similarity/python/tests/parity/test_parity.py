import tomllib
from pathlib import Path
from yidam_domain_similarity import cosine, jaccard, edit_distance

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

def test_parity_cosine():
    fixtures = load_fixtures("similarity.cosine")
    assert fixtures, "no similarity.cosine fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = cosine(inp["a"], inp["b"])
        assert result == fx["expected"]["similarity"]

def test_parity_jaccard():
    fixtures = load_fixtures("similarity.jaccard")
    assert fixtures, "no similarity.jaccard fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = jaccard(inp["a"], inp["b"])
        assert result == fx["expected"]["similarity"]

def test_parity_edit_distance():
    fixtures = load_fixtures("similarity.edit_distance")
    assert fixtures, "no similarity.edit_distance fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = edit_distance(inp["s1"], inp["s2"])
        assert result == fx["expected"]["distance"]
