import tomllib
from pathlib import Path
from yidam_domain_group_theory import modular_add, modular_mul, additive_order

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

def test_parity_modular_add():
    fixtures = load_fixtures("group_theory.modular_add")
    assert fixtures, "no group_theory.modular_add fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert modular_add(inp["a"], inp["b"], inp["n"]) == fx["expected"]["result"]

def test_parity_modular_mul():
    fixtures = load_fixtures("group_theory.modular_mul")
    assert fixtures, "no group_theory.modular_mul fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert modular_mul(inp["a"], inp["b"], inp["n"]) == fx["expected"]["result"]

def test_parity_additive_order():
    fixtures = load_fixtures("group_theory.additive_order")
    assert fixtures, "no group_theory.additive_order fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert additive_order(inp["a"], inp["n"]) == fx["expected"]["order"]
