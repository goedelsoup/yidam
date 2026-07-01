import tomllib
from pathlib import Path
from yidam_domain_graph_metrics import density, degree_centrality

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

def test_parity_density():
    fixtures = load_fixtures("graph_metrics.density")
    assert fixtures, "no graph_metrics.density fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = density(inp["node_count"], inp["edge_count"])
        assert result == fx["expected"]["density"]

def test_parity_degree_centrality():
    fixtures = load_fixtures("graph_metrics.degree_centrality")
    assert fixtures, "no graph_metrics.degree_centrality fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = degree_centrality(inp["degrees"], inp["node_count"])
        expected = fx["expected"]["centrality"]
        assert result == expected
