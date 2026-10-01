import tomllib
from pathlib import Path
from yidam_domain_energy import kinetic_energy, potential_energy, power, efficiency

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

def test_parity_kinetic_energy():
    fixtures = load_fixtures("energy.kinetic_energy")
    assert fixtures, "no energy.kinetic_energy fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert kinetic_energy(inp["mass"], inp["velocity"]) == fx["expected"]["joules"]

def test_parity_potential_energy():
    fixtures = load_fixtures("energy.potential_energy")
    assert fixtures, "no energy.potential_energy fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert potential_energy(inp["mass"], inp["height"], inp["g"]) == fx["expected"]["joules"]

def test_parity_power():
    fixtures = load_fixtures("energy.power")
    assert fixtures, "no energy.power fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert power(inp["work"], inp["time"]) == fx["expected"]["watts"]

def test_parity_efficiency():
    fixtures = load_fixtures("energy.efficiency")
    assert fixtures, "no energy.efficiency fixtures"
    for fx in fixtures:
        inp = fx["input"]
        assert efficiency(inp["output"], inp["input"]) == fx["expected"]["ratio"]
