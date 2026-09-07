"""The corpus node, as it is written on disk.

This module used to hold a Markdown parser — ``parse_node``, ``extract_claims``,
``extract_links`` — that three SDKs agreed about exactly and no product ever called. The
argument is in the Rust reference's ``corpus.rs``; the short version is that a node is a YAML
document, RFC-0002 said so, RFC-0013 closed the question, and neither function the close
specified was ever written.

Unknown top-level keys are KEPT, in ``extra``. Closing the shape rejected 117 nodes of 117 in
one derived repository, and dropping them silently is worse.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

import yaml

_DECLARED = {"class", "label", "description", "properties", "links", "cites"}

_TIMESTAMP = "tag:yaml.org,2002:timestamp"


class _InstanceLoader(yaml.SafeLoader):
    """``SafeLoader`` with the timestamp resolver removed.

    **This is the one place the three SDKs would silently disagree, and it is not a corner
    case: 41% of the nodes in the measured population carry a date.** PyYAML implements YAML
    1.1, where an unquoted ``1886-07-04`` resolves to ``datetime.date`` — which ``json.dumps``
    then refuses outright. ``serde_yaml`` and the ``yaml`` package both read it as the string
    ``"1886-07-04"``, and so does every consumer of these values, which read them as text.

    So the resolver goes, and the scalar falls through to ``str`` carrying its source
    spelling. ``parse_instance/unquoted-dates-stay-text.toml`` is the fixture that fails when
    this class is replaced by a plain ``SafeLoader``.

    Scoped to this module rather than applied to ``ontology.py``: that reads ``.ont.yml``,
    where a date *value* appears in none of the corpora measured, and changing a shipped
    parity function's behaviour is not something to do in passing.
    """


_InstanceLoader.yaml_implicit_resolvers = {
    ch: [(tag, regexp) for tag, regexp in resolvers if tag != _TIMESTAMP]
    for ch, resolvers in yaml.SafeLoader.yaml_implicit_resolvers.items()
}


@dataclass
class CorpusLink:
    """One relationship, as the instance wrote it."""

    target: str | None = None
    relationship: str | None = None
    #: A standing, or a list of them. Carried, not interpreted — see #587.
    claim_tag: Any = None
    source: str | None = None


@dataclass
class ExternalCitation:
    """One claim resting on a node in an installed dependency (RFC-0019)."""

    package: str | None = None
    node: str | None = None
    commit: str | None = None
    tag: str | None = None
    span: str | None = None


@dataclass
class CorpusInstance:
    """A corpus node: ``.yidam/corpus/<class>/<name>.yml``."""

    cls: str | None = None
    label: str | None = None
    description: str | None = None
    properties: dict[str, Any] | None = None
    links: list[CorpusLink] | None = None
    cites: list[ExternalCitation] | None = None
    #: Every other top-level key, kept rather than dropped.
    extra: dict[str, Any] = field(default_factory=dict)


def _str(v: Any) -> str | None:
    return v if isinstance(v, str) else None


def parse_instance(text: str) -> CorpusInstance:
    """Parse one corpus node.

    No ``path`` argument, and RFC-0013 specified one: its only use was deriving the node's
    kind from its directory, which is the Markdown model's move and the one RFC-0002 rejected
    when it made ``class:`` explicit. Unparseable is the empty instance rather than a raise —
    one bad file must not take a corpus down.
    """
    try:
        doc = yaml.load(text, Loader=_InstanceLoader)
    except yaml.YAMLError:
        return CorpusInstance()
    if not isinstance(doc, dict):
        return CorpusInstance()

    links = None
    if isinstance(doc.get("links"), list):
        links = []
        for item in doc["links"]:
            m = item if isinstance(item, dict) else {}
            links.append(
                CorpusLink(
                    target=_str(m.get("target")),
                    relationship=_str(m.get("relationship")),
                    claim_tag=m.get("claim_tag"),
                    source=_str(m.get("source")),
                )
            )

    cites = None
    if isinstance(doc.get("cites"), list):
        cites = []
        for item in doc["cites"]:
            m = item if isinstance(item, dict) else {}
            cites.append(
                ExternalCitation(
                    package=_str(m.get("package")),
                    node=_str(m.get("node")),
                    commit=_str(m.get("commit")),
                    tag=_str(m.get("tag")),
                    span=_str(m.get("span")),
                )
            )

    props = doc.get("properties")
    return CorpusInstance(
        cls=_str(doc.get("class")),
        label=_str(doc.get("label")),
        description=_str(doc.get("description")),
        properties=props if isinstance(props, dict) else None,
        links=links,
        cites=cites,
        extra={k: v for k, v in doc.items() if k not in _DECLARED},
    )


def instance_to_json(inst: CorpusInstance) -> dict[str, Any]:
    """This node as one JSON-shaped dict, the cross-language form of the contract.

    ``extra`` is nested here and flattened on disk, deliberately: flattening is how the keys
    are read, and nesting is how they are compared. A fixture that could not tell a declared
    key from a coined one would pass while the split it exists to check was broken.

    ``cls`` is spelled ``class`` here, because ``class`` is a Python keyword and the wire name
    is the contract's.
    """
    return {
        "class": inst.cls,
        "label": inst.label,
        "description": inst.description,
        "properties": inst.properties,
        "links": (
            None
            if inst.links is None
            else [
                {
                    "target": l.target,
                    "relationship": l.relationship,
                    "claim_tag": l.claim_tag,
                    "source": l.source,
                }
                for l in inst.links
            ]
        ),
        "cites": (
            None
            if inst.cites is None
            else [
                {
                    "package": c.package,
                    "node": c.node,
                    "commit": c.commit,
                    "tag": c.tag,
                    "span": c.span,
                }
                for c in inst.cites
            ]
        ),
        "extra": inst.extra,
    }
