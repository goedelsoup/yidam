import json
import tomllib
from pathlib import Path

from yidam_core import corpus, git, markers, ontology

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


def test_parity_parse_node():
    fixtures = load_fixtures("parse_node")
    assert fixtures, "no parse_node fixtures"
    for fx in fixtures:
        inp = fx["input"]
        exp = fx["expected"]
        node = corpus.parse_node(inp["path"], inp["content"])
        assert node.path == exp["path"], "path"
        assert node.title == exp["title"], "title"
        assert node.kind.value == exp["kind"], "kind"

        exp_claims = exp.get("claims", [])
        assert len(node.claims) == len(exp_claims), "claim count"
        for claim, ec in zip(node.claims, exp_claims):
            assert claim.text == ec["text"], "claim.text"
            assert claim.tag.value == ec["tag"], "claim.tag"

        exp_links = exp.get("links", [])
        assert len(node.links) == len(exp_links), "link count"
        for link, el in zip(node.links, exp_links):
            assert link.label == el["label"], "link.label"
            assert link.target == el["target"], "link.target"
            assert link.anchor == el.get("anchor"), "link.anchor"


def test_parity_extract_claims():
    fixtures = load_fixtures("extract_claims")
    assert fixtures, "no extract_claims fixtures"
    for fx in fixtures:
        claims = corpus.extract_claims(fx["input"]["content"])
        expected = fx["expected"]
        assert len(claims) == len(expected), "claim count"
        for claim, ec in zip(claims, expected):
            assert claim.text == ec["text"], "claim.text"
            assert claim.tag.value == ec["tag"], "claim.tag"


def test_parity_extract_links():
    fixtures = load_fixtures("extract_links")
    assert fixtures, "no extract_links fixtures"
    for fx in fixtures:
        links = corpus.extract_links(fx["input"]["content"])
        expected = fx["expected"]
        assert len(links) == len(expected), "link count"
        for link, el in zip(links, expected):
            assert link.label == el["label"], "link.label"
            assert link.target == el["target"], "link.target"
            assert link.anchor == el.get("anchor"), "link.anchor"


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
        parsed = markers.parse_markers(fx["input"]["content"])
        expected = fx["expected"]
        assert len(parsed) == len(expected), "marker count"
        for marker, em in zip(parsed, expected):
            assert marker.kind_str() == em["kind"], "marker.kind"
            if isinstance(marker, markers.TemplateMarker):
                assert marker.instruction == em["instruction"], "template.instruction"
            else:
                assert marker.command == em["command"], "regen.command"
                assert marker.content == em["content"], "regen.content"


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
