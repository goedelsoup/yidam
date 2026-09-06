from __future__ import annotations

from collections import deque
from dataclasses import dataclass


@dataclass
class GraphEdge:
    """A directed edge between two nodes, identified by corpus path."""

    from_: str
    to: str


def find_reachable(edges: list[GraphEdge], node_path: str) -> list[str]:
    """All nodes reachable from ``node_path`` following directed edges (BFS).

    The start node is not included. Result is sorted for determinism.
    """
    visited = {node_path}
    queue: deque[str] = deque([node_path])
    reachable: list[str] = []
    while queue:
        current = queue.popleft()
        for edge in edges:
            if edge.from_ == current and edge.to not in visited:
                visited.add(edge.to)
                reachable.append(edge.to)
                queue.append(edge.to)
    return sorted(reachable)


def find_citations(edges: list[GraphEdge], node_path: str) -> list[str]:
    """All nodes that have a directed edge pointing to ``node_path``.

    Result is sorted and deduplicated for determinism.
    """
    citations = sorted(e.from_ for e in edges if e.to == node_path)
    return [c for i, c in enumerate(citations) if i == 0 or c != citations[i - 1]]
