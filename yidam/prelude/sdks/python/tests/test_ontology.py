"""`source_classes` — the derivation `orphan-in` exempts on.

Held here rather than as a parity fixture: it is not on the parity function list, so
`parity-check` neither requires nor runs one. That is a gap worth closing — three
transcriptions of one subtle rule pinned only by themselves is the failure the parity loop
exists to prevent, one function over — but promoting it is a parity-surface change and
belongs in its own PR.
"""

from yidam_core.ontology import OntologyClass, OntologyEdge, source_classes


def cls(name: str, edges: list[tuple[str, str]]) -> OntologyClass:
    return OntologyClass(
        name=name,
        edges=[
            OntologyEdge(relationship="r", target=target, direction=direction)
            for target, direction in edges
        ],
    )


def test_a_class_another_class_points_at_is_not_a_source_class() -> None:
    """The correction: an inbound relationship declared from the authoring end."""
    classes = [cls("gage", [("concept", "out")]), cls("concept", [("concept", "out")])]
    sources = source_classes(classes)
    assert "concept" not in sources
    assert "gage" in sources


def test_a_self_edge_does_not_make_a_class_pointed_at() -> None:
    """A self-relation has endpoints, so it cannot mean every instance is cited."""
    assert source_classes([cls("reach", [("reach", "out")])]) == {"reach"}


def test_a_class_declaring_no_edges_is_not_a_source_class() -> None:
    """Silence is not a declaration."""
    assert source_classes([cls("quiet", [])]) == set()


def test_a_directionless_declaration_exempts_neither_end() -> None:
    """An ambiguous declaration exempts neither end."""
    assert source_classes([cls("a", [("b", "")]), cls("b", [("c", "out")])]) == set()
