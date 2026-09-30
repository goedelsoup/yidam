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
class RegenSpan:
    """Where a well-formed REGEN block sits in the text.

    So that a writer does not have to look for it a second time. The offsets are **code
    point** indices, because this is Python. They are not a parity contract and no fixture
    asserts one: the Rust SDK counts bytes and the TypeScript SDK UTF-16 code units, so a
    single number here is three different claims about any document holding a character
    outside ASCII — the trap ``find_reachable``'s sort order already fell into. What the
    fixtures grade is what the scan and the writer *do*. See RFC-0043.

    Only blocks the scan read to the end get one. A block whose open tag never closed, or
    whose ``<!-- /REGEN -->`` never arrived, has no extent to write into, and
    :func:`update_regen` left such a block alone before this type existed.
    """

    #: The command on the open tag, as the scan read it — stripped.
    command: str
    #: Index of the ``<`` that opens the block.
    open: int
    #: Index just past the ``-->`` that ends the open tag. The body starts here.
    body: int
    #: Index of the ``<`` in this block's ``<!-- /REGEN -->``. The body ends here.
    close: int
    #: Whether the body holds no newline — the whole block sits on one line.
    #:
    #: The rule RFC-0043 states, applied to the text rather than to how the block was found:
    #: ``-->44<!-- /REGEN -->`` and ``--><!-- /REGEN -->`` are inline, and a body of ``\n`` —
    #: the shape a cleared section has — is not.
    inline: bool


@dataclass
class Scan:
    """What one pass over the text found: the markers, the blocks that are malformed, and
    where the well-formed ones are."""

    markers: list[Marker]
    malformed: list[MalformedBlock]
    #: One per well-formed REGEN block, in document order. :func:`update_regen` is defined
    #: over this and nothing else, which is what makes the reader and the writer agree about
    #: what a block is (#1094).
    regen: list[RegenSpan]


_REGEN_OPEN = "<!-- REGEN:"
_REGEN_CLOSE = "<!-- /REGEN -->"
_ARROW = "-->"


def _lines_with_offsets(text: str) -> list[tuple[int, str]]:
    """``str.splitlines()``, keeping each line's index into the original text.

    Built on ``keepends=True`` rather than on a running ``len(line) + 1`` because
    ``splitlines`` breaks on more than ``\n`` — ``\r\n`` is two characters and ``\u2028``
    is one — and the offsets have to survive every separator it recognises, not just the
    one this repository writes.
    """
    out: list[tuple[int, str]] = []
    at = 0
    for raw in text.splitlines(keepends=True):
        parts = raw.splitlines()
        out.append((at, parts[0] if parts else ""))
        at += len(raw)
    return out


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
    regen: list[RegenSpan] = []
    lines = _lines_with_offsets(text)
    i = 0

    while i < len(lines):
        line_at, line = lines[i]
        stripped = line.strip()

        if stripped.startswith("<!-- TEMPLATE:"):
            rest = stripped[len("<!-- TEMPLATE:"):]
            if rest.endswith(_ARROW):
                markers.append(TemplateMarker(instruction=rest[:-3].strip()))
            i += 1
            continue

        # Every block this line opens *and closes*, left to right — the inline form. A line
        # may carry more than one, and a block found here is finished: nothing below runs
        # for it. `col` walks past each one so the next search starts after its close tag
        # rather than inside its body.
        col = 0
        block_open: int | None = None
        while True:
            rel = line.find(_REGEN_OPEN, col)
            if rel == -1:
                break
            after_open = rel + len(_REGEN_OPEN)
            arrow_at = line.find(_ARROW, after_open)
            if arrow_at == -1:
                block_open = rel
                break
            body_col = arrow_at + len(_ARROW)
            close_col = line.find(_REGEN_CLOSE, body_col)
            if close_col == -1:
                block_open = rel
                break
            inline_command = line[after_open:arrow_at].strip()
            regen.append(
                RegenSpan(
                    command=inline_command,
                    open=line_at + rel,
                    body=line_at + body_col,
                    close=line_at + close_col,
                    inline=True,
                )
            )
            markers.append(
                RegenMarker(command=inline_command, content=line[body_col:close_col].strip())
            )
            col = close_col + len(_REGEN_CLOSE)

        # What is left is an open tag with no close beside it — the block form, which is
        # only a block when it starts its line. A `<!-- REGEN:` in the middle of a sentence
        # with no close tag after it is prose, and was prose before this function learned
        # the inline form; reading it as a block would make it swallow the rest of the file.
        if block_open is None or line[:block_open].strip() != "":
            i += 1
            continue

        open_col = block_open
        rest = line[open_col + len(_REGEN_OPEN):]
        rest_stripped = rest.strip()
        open_line = i
        fault: Fault | None = None
        body_at: int | None = None

        if rest_stripped.endswith(_ARROW):
            # Single-line open tag. The arrow is the last thing on the line by that test, so
            # the body starts where the line's own trailing whitespace does.
            command = rest_stripped[:-3].strip()
            body_at = line_at + len(line.rstrip())
            i += 1
        else:
            command = rest.strip()
            i += 1
            arrow_found = False
            while i < len(lines):
                at, raw = lines[i]
                t = raw.strip()
                i += 1
                if t == _ARROW or t.endswith(_ARROW):
                    body_at = at + len(raw.rstrip())
                    arrow_found = True
                    break
            if not arrow_found:
                fault = Fault.OPEN_ARROW_MISSING

        content_start = i
        content_end = len(lines)
        close_at: int | None = None
        closed = False
        while i < len(lines):
            at, raw = lines[i]
            if raw.strip() == _REGEN_CLOSE:
                content_end = i
                close_at = at + (len(raw) - len(raw.lstrip()))
                i += 1
                closed = True
                break
            i += 1
        if fault is None:
            if not closed:
                fault = Fault.CLOSE_TAG_MISSING
            elif any(_opens_a_regen(raw) for _, raw in lines[content_start:content_end]):
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
                    swallowed_markers=sum(1 for _, raw in swallowed if _opens_a_marker(raw)),
                )
            )

        # An extent, but only for a block with two ends. OpenArrowMissing has no body to
        # start, and CloseTagMissing none to end; update_regen returned such a file unchanged
        # when it did its own searching, and it returns it unchanged now.
        if body_at is not None and close_at is not None:
            regen.append(
                RegenSpan(
                    command=command,
                    open=line_at + open_col,
                    body=body_at,
                    close=close_at,
                    inline="\n" not in text[body_at:close_at],
                )
            )

        content = "\n".join(raw for _, raw in lines[content_start:content_end]).strip()
        markers.append(RegenMarker(command=command, content=content))

    return Scan(markers=markers, malformed=malformed, regen=regen)


def parse_markers(text: str) -> list[Marker]:
    return scan_markers(text).markers


def _regen_body(new_content: str, inline: bool, stands_alone: bool) -> str | None:
    """The body to write between a block's markers.

    An inline block keeps its line: the author wrote it that way and no regeneration
    second-guesses them. Every other block is bracketed by newlines, and the empty case is
    collapsed so that clearing a section does not leave a blank line between the markers.

    Both conditions are read, not just ``inline``: writing a multi-line value into a
    one-line block would leave a document whose form no longer matches its body, and the
    next run would read it as a block-form block and rewrite it differently. Deciding on
    both is what makes a second run a no-op.

    ``None`` is a block that cannot hold the value. Block form is a *line* form: the scan
    reads one only where the open tag starts its line and the close tag stands alone on its
    own (#1137). An inline block that shares its line would lose its open tag to prose, or
    have its close tag read as some later block's, so it is left as it is.
    """
    if inline and "\n" not in new_content:
        return new_content
    if inline and not stands_alone:
        return None
    if new_content == "":
        return "\n"
    return f"\n{new_content}\n"


def _stands_alone(text: str, span: RegenSpan) -> bool:
    """Whether nothing but whitespace shares the block's line, before its open tag or after
    its close tag. See the Rust note."""
    start = text.rfind("\n", 0, span.open) + 1
    end = span.close + len(_REGEN_CLOSE)
    stop = text.find("\n", end)
    if stop == -1:
        stop = len(text)
    return text[start:span.open].strip() == "" and text[end:stop].strip() == ""


def update_regen(text: str, command: str, new_content: str) -> str:
    """Replace the body of every REGEN block named ``command``.

    Defined over :func:`scan_markers`, which is the point: before #1094 this searched the
    text itself, and the two searches disagreed. A block the scan cannot read is a block this
    leaves alone, and a block it reads inline stays inline — or, given a value that needs more
    than one line, takes the block form if the line is its own and is left alone if not.

    Two things changed with the second search's removal, and both close a defect rather than
    open a feature. The command matches **exactly**, where the old prefix search let
    ``yidam status`` claim a ``yidam status-quo`` block (#1058). And **every** block with the
    command is rewritten, where the old search rewrote the first and left a second copy of
    the same figure stale with nothing to report it.
    """
    spans = [s for s in scan_markers(text).regen if s.command == command]
    if not spans:
        return text

    out: list[str] = []
    cursor = 0
    for span in spans:
        body = _regen_body(new_content, span.inline, _stands_alone(text, span))
        if body is None:
            continue
        out.append(text[cursor:span.body])
        out.append(body)
        cursor = span.close
    out.append(text[cursor:])
    return "".join(out)
