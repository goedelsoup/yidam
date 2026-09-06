from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


@dataclass
class TemplateMarker:
    instruction: str

    def kind_str(self) -> str:
        return "Template"


@dataclass
class RegenMarker:
    command: str
    content: str

    def kind_str(self) -> str:
        return "Regen"


Marker = TemplateMarker | RegenMarker


class Fault(str, Enum):
    """What is wrong with a REGEN block the scan crossed."""

    #: The open tag ran onto further lines and its ``-->`` never arrived.
    OPEN_ARROW_MISSING = "OpenArrowMissing"
    #: ``<!-- /REGEN -->`` never arrived; the rest of the input became this block's content.
    CLOSE_TAG_MISSING = "CloseTagMissing"
    #: The block closed on a tag belonging to a block opened inside its own body. A close tag
    #: is missing above, and this is the shape a real file has: ``CLOSE_TAG_MISSING`` needs
    #: the damaged block to be the last one in the document, and it usually is not.
    CLOSED_ON_ANOTHERS_TAG = "ClosedOnAnothersTag"


@dataclass
class MalformedBlock:
    """A REGEN block whose extent the scan could not read the way it was meant.

    In every case the block has taken lines that were not its content, and every marker among
    them is a marker the caller never sees — which is what ``swallowed_markers`` counts.
    """

    command: str
    #: 1-indexed line the open tag sits on.
    line: int
    fault: Fault
    #: Lines after the open tag that this block took as its own.
    swallowed_lines: int
    #: How many of those lines open a marker — markers that are now content.
    swallowed_markers: int


@dataclass
class Scan:
    """What one pass over the text found: the markers, and the blocks that are malformed."""

    markers: list[Marker]
    malformed: list[MalformedBlock]


def _opens_a_regen(line: str) -> bool:
    """A body containing one of these means a close tag is missing above it."""
    return line.strip().startswith("<!-- REGEN:")


def _opens_a_marker(line: str) -> bool:
    return line.strip().startswith("<!-- REGEN:") or line.strip().startswith("<!-- TEMPLATE:")


def scan_markers(text: str) -> Scan:
    """The markers, and the blocks that took lines which were not theirs.

    One pass, two outputs. :func:`parse_markers` is this without the second, and keeps its
    signature: the marker sequence is a frozen parity contract and does not change here.
    """
    markers: list[Marker] = []
    malformed: list[MalformedBlock] = []
    lines = text.splitlines()
    i = 0

    while i < len(lines):
        stripped = lines[i].strip()

        if stripped.startswith("<!-- TEMPLATE:"):
            rest = stripped[len("<!-- TEMPLATE:"):]
            if rest.endswith("-->"):
                markers.append(TemplateMarker(instruction=rest[:-3].strip()))
            i += 1
            continue

        if not stripped.startswith("<!-- REGEN:"):
            i += 1
            continue

        rest = stripped[len("<!-- REGEN:"):]
        rest_stripped = rest.strip()
        open_line = i
        fault: Fault | None = None

        if rest_stripped.endswith("-->"):
            command = rest_stripped[:-3].strip()
            i += 1
        else:
            command = rest.strip()
            i += 1
            arrow_found = False
            while i < len(lines):
                t = lines[i].strip()
                i += 1
                if t == "-->" or t.endswith("-->"):
                    arrow_found = True
                    break
            if not arrow_found:
                fault = Fault.OPEN_ARROW_MISSING

        content_start = i
        content_end = len(lines)
        closed = False
        while i < len(lines):
            if lines[i].strip() == "<!-- /REGEN -->":
                content_end = i
                i += 1
                closed = True
                break
            i += 1
        if fault is None:
            if not closed:
                fault = Fault.CLOSE_TAG_MISSING
            elif any(_opens_a_regen(line) for line in lines[content_start:content_end]):
                fault = Fault.CLOSED_ON_ANOTHERS_TAG

        if fault is not None:
            # From the open tag to wherever the content stopped, which in the OpenArrow case
            # is the end of the input: the body is empty there and everything was consumed
            # looking for the arrow, so a count over the body alone reports nothing.
            swallowed = lines[open_line + 1:content_end]
            malformed.append(
                MalformedBlock(
                    command=command,
                    line=open_line + 1,
                    fault=fault,
                    swallowed_lines=len(swallowed),
                    swallowed_markers=sum(1 for line in swallowed if _opens_a_marker(line)),
                )
            )

        content = "\n".join(lines[content_start:content_end]).strip()
        markers.append(RegenMarker(command=command, content=content))

    return Scan(markers=markers, malformed=malformed)


def parse_markers(text: str) -> list[Marker]:
    return scan_markers(text).markers


def update_regen(text: str, command: str, new_content: str) -> str:
    open_tag = f"<!-- REGEN: {command}"
    close_tag = "<!-- /REGEN -->"

    open_pos = text.find(open_tag)
    if open_pos == -1:
        return text

    after_open = open_pos + len(open_tag)
    arrow_rel = text[after_open:].find("-->")
    if arrow_rel == -1:
        return text

    content_start = after_open + arrow_rel + 3
    close_rel = text[content_start:].find(close_tag)
    if close_rel == -1:
        return text

    close_abs = content_start + close_rel
    if new_content == "":
        # Clear the body without leaving a blank line between the markers.
        return f"{text[:content_start]}\n{text[close_abs:]}"
    return f"{text[:content_start]}\n{new_content}\n{text[close_abs:]}"
