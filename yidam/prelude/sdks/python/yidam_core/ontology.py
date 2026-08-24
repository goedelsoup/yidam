"""The class contract, read and published.

``.yidam/corpus/<class>.ont.yml`` declares what an instance of a class carries and what it
may link to. ``yidam lint`` **enforces** that declaration; this module **publishes** it, as
JSON Schema any editor or validator can apply without linking against yidam.

The Rust implementation in ``sdks/rust/src/ontology.rs`` is the reference and carries the
full argument for each decision. The short version, because it is the part that is easy to
get wrong here: the compiled schema is deliberately **no stricter than the checks**. A
consumer that rejected what the gate accepts would fail somebody's build on a file that
looked fine everywhere else.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

import yaml

#: The evidence tokens a ``claim`` property may hold, in both spellings.
#:
#: Bare is what a typed vocabulary stores; bracketed is what a corpus writes after being
#: told the prose scan needs brackets. Both are accepted by the counter, so both here.
CLAIM_TOKENS = [
    "verified",
    "inference",
    "open",
    "[verified]",
    "[inference]",
    "[open]",
]


@dataclass
class OntologyProperty:
    name: str
    #: ``string``, ``text``, ``date``, ``ref``, ``claim`` — or a type this corpus coined.
    type: str = ""
    description: str = ""


@dataclass
class OntologyEdge:
    relationship: str
    target: str = ""
    #: ``out`` when instances of this class author the link, ``in`` when the other side does.
    direction: str = ""
    description: str = ""


@dataclass
class OntologyClass:
    name: str
    label: str = ""
    description: str = ""
    properties: list[OntologyProperty] = field(default_factory=list)
    edges: list[OntologyEdge] = field(default_factory=list)

    def is_source_class(self) -> bool:
        """A class nothing is meant to point at: it declares edges, none of them inbound.

        A class that declares no edges at all is **not** a source class — it has said
        nothing about its shape, and reading silence as a declaration would exempt every
        instance in a corpus whose ontology is not filled in.
        """
        return bool(self.edges) and not any(e.direction == "in" for e in self.edges)


def _str(value: Any) -> str:
    return value if isinstance(value, str) else ""


def _mappings(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def parse_class(name: str, content: str) -> OntologyClass:
    """Read a class definition. ``name`` is the fallback when the file does not name itself.

    A file that does not parse yields a class that declares nothing, which under the
    silence rule constrains nothing — the same direction ``lint`` degrades in. A malformed
    ontology is a finding of its own, not a reason to start rejecting instances.
    """
    try:
        doc = yaml.safe_load(content)
    except yaml.YAMLError:
        doc = None
    if not isinstance(doc, dict):
        doc = {}

    declared = _str(doc.get("class"))
    return OntologyClass(
        name=declared or name,
        label=_str(doc.get("label")),
        description=_str(doc.get("description")),
        properties=[
            OntologyProperty(
                name=_str(p.get("name")),
                type=_str(p.get("type")),
                description=_str(p.get("description")),
            )
            for p in _mappings(doc.get("properties"))
        ],
        edges=[
            OntologyEdge(
                relationship=_str(e.get("relationship")),
                target=_str(e.get("target")),
                direction=_str(e.get("direction")),
                description=_str(e.get("description")),
            )
            for e in _mappings(doc.get("edges"))
        ],
    )


def _property_schema(property_type: str) -> Any:
    """Mirrors ``lint``'s ``property-type`` check, including what it declines to check.

    A type the corpus coined for itself compiles to ``True`` — valid against anything —
    because a schema rejecting every type it had not heard of would make coining one
    impossible.
    """
    if property_type in ("string", "text", "ref"):
        return {"type": "string", "minLength": 1}
    # Structural, not a calendar: what it catches is a date field carrying prose.
    if property_type == "date":
        return {"type": "string", "pattern": "^[0-9]{4}(-[0-9]{2}(-[0-9]{2})?)?$"}
    # A list is legal here and nowhere else: the counter reads a list of tags as one claim
    # each, so `claim_tag: [open]` unquoted is a one-element list nobody meant to write.
    if property_type == "claim":
        return {
            "anyOf": [
                {"enum": list(CLAIM_TOKENS)},
                {"type": "array", "items": {"enum": list(CLAIM_TOKENS)}},
            ]
        }
    return True


def compile_class_schema(cls: OntologyClass) -> dict[str, Any]:
    """Compile a class definition into a JSON Schema for its instances.

    Two things it deliberately does not constrain. **No declared property is required** —
    ``missing-property`` reports and does not gate, so demanding them would reject
    instances the gate accepts. **``links[].relationship`` is left open** — the gate
    licenses a relationship only for edges landing on another instance, and JSON Schema
    cannot resolve a path, so a constraint here would reject the ``instance-of`` link every
    instance is required to carry. The declared relationships are published as
    ``x-yidam-edges`` for completion instead.
    """
    schema: dict[str, Any] = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": f"yidam corpus node — {cls.name}",
    }
    if cls.description:
        schema["description"] = cls.description
    schema["type"] = "object"

    properties: dict[str, Any] = {"class": {"const": cls.name}}

    # Silence is not a contract. A class declaring no properties constrains none, and in
    # particular does not close the bag — which would reject every instance in a corpus
    # whose ontology is not filled in.
    if cls.properties:
        declared: dict[str, Any] = {}
        for p in cls.properties:
            body = _property_schema(p.type)
            if isinstance(body, dict) and p.description:
                body = {**body, "description": p.description}
            declared[p.name] = body
        properties["properties"] = {
            "type": "object",
            "properties": declared,
            # Closed, matching `undeclared-property`, which gates.
            "additionalProperties": False,
        }

    schema["properties"] = properties
    schema["required"] = ["class"]
    # Permissive at the top level, as the shared node schema is: derived corpora carry their
    # own fields, and closing this rejected 117 nodes of 117 in one repository.
    schema["additionalProperties"] = True

    if cls.edges:
        schema["x-yidam-edges"] = [
            {
                "relationship": e.relationship,
                "target": e.target,
                "direction": e.direction,
            }
            for e in cls.edges
        ]
    return schema
