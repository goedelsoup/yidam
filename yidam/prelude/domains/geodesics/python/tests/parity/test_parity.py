import math
import tomllib
from pathlib import Path
from yidam_domain_geodesics import haversine_km, bearing_deg, central_angle_deg

FIXTURES_DIR = Path(__file__).parent.parent.parent.parent.parent / "parity" / "fixtures"
EPSILON = 1e-4

def load_fixtures(function: str) -> list[dict]:
    d = FIXTURES_DIR / function
    if not d.exists():
        return []
    out = []
    for p in sorted(d.glob("*.toml")):
        with open(p, "rb") as f:
            out.append(tomllib.load(f))
    return out

def test_parity_haversine_km():
    fixtures = load_fixtures("geodesics.haversine_km")
    assert fixtures, "no geodesics.haversine_km fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = haversine_km(inp["lat1"], inp["lon1"], inp["lat2"], inp["lon2"])
        assert abs(result - fx["expected"]["km"]) < EPSILON

def test_parity_bearing_deg():
    fixtures = load_fixtures("geodesics.bearing_deg")
    assert fixtures, "no geodesics.bearing_deg fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = bearing_deg(inp["lat1"], inp["lon1"], inp["lat2"], inp["lon2"])
        assert abs(result - fx["expected"]["degrees"]) < EPSILON

def test_parity_central_angle_deg():
    fixtures = load_fixtures("geodesics.central_angle_deg")
    assert fixtures, "no geodesics.central_angle_deg fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = central_angle_deg(inp["lat1"], inp["lon1"], inp["lat2"], inp["lon2"])
        assert abs(result - fx["expected"]["degrees"]) < EPSILON
