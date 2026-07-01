def density(node_count: int, edge_count: int) -> float:
    if node_count < 2:
        return 0.0
    max_edges = node_count * (node_count - 1) / 2
    return edge_count / max_edges

def degree_centrality(degrees: list[int], node_count: int) -> list[float]:
    if node_count < 2:
        return [0.0] * len(degrees)
    n = node_count - 1
    return [d / n for d in degrees]
