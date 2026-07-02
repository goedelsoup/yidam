from __future__ import annotations

import re
from dataclasses import dataclass, field
from enum import Enum


class EvidenceTag(str, Enum):
    Verified = "Verified"
    Inference = "Inference"
    Open = "Open"
    Implicit = "Implicit"


@dataclass
class Claim:
    text: str
    tag: EvidenceTag


@dataclass
class Link:
    label: str
    target: str
    anchor: str | None = None


class NodeKind(str, Enum):
    Concept = "Concept"
    Decision = "Decision"
    Authored = "Authored"
    Generated = "Generated"


@dataclass
class CorpusNode:
    path: str
    title: str
    kind: NodeKind
    claims: list[Claim] = field(default_factory=list)
    links: list[Link] = field(default_factory=list)


_LINK_RE = re.compile(r'(?<!!)\[([^\]]+)\]\(([^)]+)\)')


def extract_links(text: str) -> list[Link]:
    links = []
    for m in _LINK_RE.finditer(text):
        label = m.group(1)
        dest = m.group(2)
        if "#" in dest:
            pos = dest.index("#")
            target = dest[:pos]
            anchor = dest[pos + 1:] or None
        else:
            target = dest
            anchor = None
        links.append(Link(label=label, target=target, anchor=anchor))
    return links


def _contains_md_link(s: str) -> bool:
    return "](" in s


def extract_claims(text: str) -> list[Claim]:
    claims = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        if line[0] in ("#", "`", "|", "-", "*", "!", ">"):
            continue
        if line.startswith("<!--"):
            continue
        if line.endswith(" [verified]"):
            claims.append(Claim(text=line[: -len(" [verified]")], tag=EvidenceTag.Verified))
        elif line.endswith(" [inferred]"):
            claims.append(Claim(text=line[: -len(" [inferred]")], tag=EvidenceTag.Inference))
        elif line.endswith(" [open]"):
            claims.append(Claim(text=line[: -len(" [open]")], tag=EvidenceTag.Open))
        elif not _contains_md_link(line):
            claims.append(Claim(text=line, tag=EvidenceTag.Implicit))
    return claims


def parse_node(path: str, text: str) -> CorpusNode:
    title = ""
    for line in text.splitlines():
        if line.startswith("# "):
            title = line[2:].strip()
            break

    if path.startswith("corpus/"):
        kind = NodeKind.Concept
    elif path.startswith("decisions/"):
        kind = NodeKind.Decision
    else:
        kind = NodeKind.Authored

    return CorpusNode(
        path=path,
        title=title,
        kind=kind,
        claims=extract_claims(text),
        links=extract_links(text),
    )
