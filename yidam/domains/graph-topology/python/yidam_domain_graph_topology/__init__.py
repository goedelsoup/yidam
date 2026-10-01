def clustering_coefficient(degree: int, triangle_count: int) -> float:
    if degree < 2:
        return 0.0
    possible = degree * (degree - 1) / 2
    return triangle_count / possible

def connected_components(node_count: int, edges: list[list[int]]) -> int:
    if node_count == 0:
        return 0
    parent = list(range(node_count))

    def find(x: int) -> int:
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for edge in edges:
        a, b = find(edge[0]), find(edge[1])
        if a != b:
            parent[a] = b

    return sum(1 for i in range(node_count) if find(i) == i)
