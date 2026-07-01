import tomllib
from pathlib import Path
from yidam_domain_set_theory import union, intersection, difference, is_subset

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

def test_parity_union():
    fixtures = load_fixtures("set_theory.union")
    assert fixtures, "no set_theory.union fixtures"
    for fx in fixtures:
        assert union(fx["input"]["a"], fx["input"]["b"]) == fx["expected"]["elements"]

def test_parity_intersection():
    fixtures = load_fixtures("set_theory.intersection")
    assert fixtures, "no set_theory.intersection fixtures"
    for fx in fixtures:
        assert intersection(fx["input"]["a"], fx["input"]["b"]) == fx["expected"]["elements"]

def test_parity_difference():
    fixtures = load_fixtures("set_theory.difference")
    assert fixtures, "no set_theory.difference fixtures"
    for fx in fixtures:
        assert difference(fx["input"]["a"], fx["input"]["b"]) == fx["expected"]["elements"]

def test_parity_is_subset():
    fixtures = load_fixtures("set_theory.is_subset")
    assert fixtures, "no set_theory.is_subset fixtures"
    for fx in fixtures:
        assert is_subset(fx["input"]["a"], fx["input"]["b"]) == fx["expected"]["result"]
