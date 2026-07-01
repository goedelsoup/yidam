import tomllib
from pathlib import Path
from yidam_domain_information_theory import entropy, kl_divergence

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

def test_parity_entropy():
    fixtures = load_fixtures("information_theory.entropy")
    assert fixtures, "no information_theory.entropy fixtures"
    for fx in fixtures:
        result = entropy(fx["input"]["probs"])
        assert result == fx["expected"]["entropy"]

def test_parity_kl_divergence():
    fixtures = load_fixtures("information_theory.kl_divergence")
    assert fixtures, "no information_theory.kl_divergence fixtures"
    for fx in fixtures:
        result = kl_divergence(fx["input"]["p"], fx["input"]["q"])
        assert result == fx["expected"]["kl"]
