"""Where a line ends and what whitespace is — the Rust SDK's rule, written out.

One copy for every parity function that reads lines or trims text, so that ``parse_markers``
and ``classify_commit`` cannot drift apart again: each called Python's built-ins on its own,
and the built-ins draw both lines differently from Rust (#1620, #1666).
"""

from __future__ import annotations

#: Rust's ``char::is_whitespace``: the Unicode ``White_Space`` property, and nothing else.
#:
#: Not ``str.strip()``'s default, which also strips U+001C to U+001F — so a close tag followed
#: by U+001F closed a block here and nowhere else (#1620), and a commit verb followed by one
#: was Operational here and Epistemic in the other two SDKs (#1666). Pass it to ``strip``,
#: ``lstrip`` and ``rstrip``.
WHITE_SPACE = "".join(
    chr(c)
    for c in [
        *range(0x09, 0x0E),
        0x20,
        0x85,
        0xA0,
        0x1680,
        *range(0x2000, 0x200B),
        0x2028,
        0x2029,
        0x202F,
        0x205F,
        0x3000,
    ]
)


def lines_with_offsets(text: str) -> list[tuple[int, str]]:
    """The lines of ``text``, each with its index into the original — the Rust SDK's rule.

    A line ends at ``\\n`` and nowhere else, and one ``\\r`` before that ``\\n`` is not part
    of it. A last line with no ``\\n`` after it is kept as it is, and a text ending in ``\\n``
    has no empty line after it.

    Not ``str.splitlines()``, which also breaks on ``\\f``, ``\\v``, a lone ``\\r``, U+001C to
    U+001E, U+0085, U+2028 and U+2029. Each of those split a block here that the other two
    SDKs read whole, so the same file had different content in Python (#1620).
    """
    out: list[tuple[int, str]] = []
    at = 0
    raw = text.split("\n")
    for line in raw[:-1]:
        out.append((at, line[:-1] if line.endswith("\r") else line))
        at += len(line) + 1
    if raw[-1] != "":
        out.append((at, raw[-1]))
    return out


def first_line(text: str) -> str:
    """The first line of ``text`` by :func:`lines_with_offsets`' rule, or ``""`` if it has none.

    Rust's ``text.lines().next().unwrap_or("")``. ``str.splitlines()[0]`` cut a commit
    subject at a U+2028 or a form feed, and lost the whole line when one led it (#1666).
    """
    lines = lines_with_offsets(text)
    return lines[0][1] if lines else ""
