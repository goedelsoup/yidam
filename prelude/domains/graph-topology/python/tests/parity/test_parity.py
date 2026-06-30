import tomllib
from pathlib import Path
from yidam_domain_graph_topology import clustering_coefficient, connected_components

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

def test_parity_clustering_coefficient():
    fixtures = load_fixtures("graph_topology.clustering_coefficient")
    assert fixtures, "no graph_topology.clustering_coefficient fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = clustering_coefficient(inp["degree"], inp["triangle_count"])
        assert result == fx["expected"]["coefficient"]

def test_parity_connected_components():
    fixtures = load_fixtures("graph_topology.connected_components")
    assert fixtures, "no graph_topology.connected_components fixtures"
    for fx in fixtures:
        inp = fx["input"]
        edges = inp.get("edges", [])
        result = connected_components(inp["node_count"], edges)
        assert result == fx["expected"]["components"]
