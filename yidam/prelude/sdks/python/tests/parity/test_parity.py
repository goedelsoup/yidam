import json
import tomllib
from pathlib import Path

from yidam_core import corpus, git, graph, markers, ontology

FIXTURES_DIR = Path(__file__).parent.parent.parent.parent / "parity" / "fixtures"


def load_fixtures(function: str) -> list[dict]:
    d = FIXTURES_DIR / function
    if not d.exists():
        return []
    out = []
    for p in sorted(d.glob("*.toml")):
        with open(p, "rb") as f:
            out.append(tomllib.load(f))
    return out


def test_parity_parse_instance():
    """Compared as parsed JSON, for ``compile_class_schema``'s reason.

    Key order is not part of the contract. Non-string scalar resolution is not either, and
    the fixtures avoid it: ``010`` is the string ``"010"`` to serde_yaml, the number 10 to
    the ``yaml`` package, and 8 here. The one such divergence a corpus actually hits is the
    timestamp — on 41% of the nodes in the measured population — and it IS in contract.
    ``unquoted-dates-stay-text.toml`` is what fails if ``_InstanceLoader`` is replaced by a
    plain ``SafeLoader``: PyYAML resolves the scalar to ``datetime.date`` and ``json.dumps``
    refuses it outright, so the failure is an error rather than a wrong value.
    """
    fixtures = load_fixtures("parse_instance")
    assert fixtures, "no parse_instance fixtures"
    for fx in fixtures:
        got = corpus.instance_to_json(corpus.parse_instance(fx["input"]["content"]))
        want = json.loads(fx["expected"]["instance"])
        # Round-tripped through JSON so a value PyYAML resolved to something unserialisable
        # fails here, where the fixture names it, rather than deeper in a comparison.
        assert json.loads(json.dumps(got)) == want, fx["description"]


def test_parity_classify_commit():
    fixtures = load_fixtures("classify_commit")
    assert fixtures, "no classify_commit fixtures"
    for fx in fixtures:
        inp = fx["input"]
        exp = fx["expected"]
        event = git.classify_commit(inp["hash"], inp["message"])
        assert event.kind.value == exp["kind"], "kind"
        assert event.verb == exp["verb"], "verb"
        assert event.subject == exp["subject"], "subject"


def test_parity_is_recognized_verb():
    fixtures = load_fixtures("is_recognized_verb")
    assert fixtures, "no is_recognized_verb fixtures"
    for fx in fixtures:
        verb = fx["input"]["verb"]
        assert git.is_recognized_verb(verb) is fx["expected"]["recognized"], (
            f"is_recognized_verb({verb!r})"
        )


def test_parity_parse_markers():
    fixtures = load_fixtures("parse_markers")
    assert fixtures, "no parse_markers fixtures"
    for fx in fixtures:
        scan = markers.scan_markers(fx["input"]["content"])
        parsed = scan.markers
        expected = fx["expected"]
        assert len(parsed) == len(expected), "marker count"
        for marker, em in zip(parsed, expected):
            assert marker.kind_str() == em["kind"], "marker.kind"
            if isinstance(marker, markers.TemplateMarker):
                assert marker.instruction == em["instruction"], "template.instruction"
            else:
                assert marker.command == em["command"], "regen.command"
                assert marker.content == em["content"], "regen.content"

        # Absent means none, not "not checked". A fixture written before this field existed
        # is asserting that its input has no malformed block.
        bad = fx.get("expected_malformed", [])
        assert len(scan.malformed) == len(bad), f"malformed count: {scan.malformed}"
        for block, eb in zip(scan.malformed, bad):
            assert block.command == eb["command"], "malformed.command"
            assert block.line == eb["line"], "malformed.line"
            assert block.fault.value == eb["fault"], "malformed.fault"
            assert block.swallowed_lines == eb["swallowed_lines"], "malformed.swallowed_lines"
            assert block.swallowed_markers == eb["swallowed_markers"], "malformed.swallowed_markers"


def _edges(inp: dict) -> list[graph.GraphEdge]:
    return [graph.GraphEdge(from_=e["from"], to=e["to"]) for e in inp["edges"]]


def test_parity_find_reachable():
    fixtures = load_fixtures("find_reachable")
    assert fixtures, "no find_reachable fixtures"
    for fx in fixtures:
        inp = fx["input"]
        reachable = graph.find_reachable(_edges(inp), inp["node_path"])
        assert reachable == fx["expected"]["reachable"], fx["description"]


def test_parity_find_citations():
    fixtures = load_fixtures("find_citations")
    assert fixtures, "no find_citations fixtures"
    for fx in fixtures:
        inp = fx["input"]
        citations = graph.find_citations(_edges(inp), inp["node_path"])
        assert citations == fx["expected"]["citations"], fx["description"]


def test_parity_update_regen():
    fixtures = load_fixtures("update_regen")
    assert fixtures, "no update_regen fixtures"
    for fx in fixtures:
        inp = fx["input"]
        result = markers.update_regen(inp["content"], inp["command"], inp["new_content"])
        assert result == fx["expected"]["content"]


def test_parity_compile_class_schema():
    fixtures = load_fixtures("compile_class_schema")
    assert fixtures, "no compile_class_schema fixtures"
    for fx in fixtures:
        inp = fx["input"]
        cls = ontology.parse_class(inp["name"], inp["content"])
        got = ontology.compile_class_schema(cls)
        # Compared as parsed JSON, not as text: key order and whitespace are not part of
        # the contract, and three languages will not agree on either.
        assert got == json.loads(fx["expected"]["schema"]), fx["description"]
