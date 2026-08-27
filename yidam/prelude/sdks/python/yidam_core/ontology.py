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
    #: Whether every instance of the class must carry this property.
    #:
    #: **Absent means false**, and not out of timidity: every corpus written before this
    #: field existed was written under a schema where the question could not be asked.
    #: Defaulting to ``True`` would require a declaration nobody made, in every derived
    #: repository at once. It is what lets ``missing-property`` gate at all.
    required: bool = False


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



def source_classes(classes: list[OntologyClass]) -> set[str]:
    """Classes the ontology says nothing points at.

    The same derivation ``orphan-in`` exempts on, exposed here so a consumer computing a
    per-class orphan expectation reads the rule rather than re-deriving it.

    **It takes the whole ontology, and that is the correction.** This was once
    ``OntologyClass.is_source_class()``, reading one class's own edge list for a
    ``direction: in`` entry — which reads half the ontology. ``B: {target: A, direction:
    out}`` declares that instances of ``B`` point at instances of ``A``; it is the same fact
    as ``A: {direction: in}`` stated from the authoring end, and ``target`` is *"the class at
    the other end, whichever end authors the link"*. Reading only a class's own list treated
    its silence about inbound edges as a positive declaration that nothing points at it.
    Measured upstream: all three classes of the worked example derived as source classes, so
    ``orphan-in`` could not fire anywhere in it.

    Two things it deliberately does not do:

    - **A class declaring no edges at all is not a source class.** It has said nothing about
      its shape, and reading silence as a declaration would exempt every instance in a corpus
      whose ontology is not filled in.
    - **A self-edge does not make a class pointed at.** ``reach -downstream-of-> reach`` says
      instances relate to each other, not that every instance is cited — any acyclic
      self-relation has an endpoint that is not.
    """
    pointed: set[str] = set()
    for cls in classes:
        for edge in cls.edges:
            if edge.target == cls.name:
                continue
            if edge.direction == "in":
                pointed.add(cls.name)
            elif edge.direction == "out":
                pointed.add(edge.target)
            else:
                # A declaration that does not say which way it runs exempts neither end.
                pointed.add(cls.name)
                pointed.add(edge.target)
    return {c.name for c in classes if c.edges and c.name not in pointed}


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
                required=p.get("required") is True,
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

    Two things about strictness. **A declared property is required only where the class
    says ``required: true``** — the compiled schema must be no stricter than the gate, and
    ``missing-property`` gates on exactly those and warns for the rest, so the same
    declaration decides both and neither can outrun the other. **``links[].relationship``
    is left open** — the gate
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
        bag: dict[str, Any] = {"type": "object", "properties": declared}
        # Emitted for exactly the properties declared `required: true`, and omitted
        # entirely when there are none — an empty `required: []` would be a different
        # document for the same meaning, and these schemas are compared byte for byte
        # across three languages.
        required = [p.name for p in cls.properties if p.required]
        if required:
            bag["required"] = required
        # Closed, matching `undeclared-property`, which gates.
        bag["additionalProperties"] = False
        properties["properties"] = bag

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
