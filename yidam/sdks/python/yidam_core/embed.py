"""One canonical embed text, and the reason there has to be exactly one.

An index is built out of a string per node, and *which* string is a judgement: a label, the
prose the class declared, the identifiers a query is actually typed in, the names of the things
this node points at. Two implementations of that judgement over one corpus produce two different
vector spaces from the same weights, and nothing about either looks wrong.

That is not hypothetical. RFC-0007 was filed about two assemblers over one corpus — this
repository's, composing ``label · description · Related: <stems>.``, and a downstream consumer's,
composing ``label · description · class · <curated meta>`` because the SDK shipped no opinion and
it had to invent one. Each was reasonable. Neither could be compared to the other, because
neither was written down anywhere both could read.

So the assembler is a parity function, held to a shared fixture, and
:func:`compose_embed_text` is it. See the Rust reference's ``embed.rs`` for the full argument:
the three field lists and who decides them, why ``retrievable`` is a third axis and not prose,
why the union runs one way, and why absent means false.
"""

from dataclasses import dataclass, field

from . import prose
from ._text import WHITE_SPACE
from .corpus import CorpusInstance, CorpusLink


@dataclass
class EmbedFields:
    """What one class declared about which of its fields belong in an embedding.

    A named shape rather than three positional lists, because the three are not interchangeable
    and a caller passing them in the wrong order would run. It is also what makes the *union*
    impossible to forget — the mistake ``embed`` made for as long as it asked ``prose`` alone,
    and missed every flagged identifier in the corpus.
    """

    #: Top-level keys that carry prose, in the order the caller resolved them.
    prose_keys: list[str] = field(default_factory=list)
    #: Property names the class flagged ``prose: true``, in declaration order.
    prose_properties: list[str] = field(default_factory=list)
    #: Property names the class flagged ``retrievable: true``, in declaration order.
    retrievable_properties: list[str] = field(default_factory=list)


def of(inst: CorpusInstance, fields: EmbedFields) -> list[tuple[str, str]]:
    """Everything this node says that belongs in its embedding: its prose, then the properties
    flagged ``retrievable`` that prose did not already carry.

    Keys come back qualified the way :func:`yidam_core.prose.of` qualifies them —
    ``properties.units``. That qualification is also what makes the de-duplication exact rather
    than a guess: a property flagged both ``prose`` and ``retrievable`` arrives under one key
    from both sides and is emitted once, while a top-level ``parameter`` and a property
    ``parameter`` stay two things.
    """
    out = prose.of(inst, fields.prose_keys, fields.prose_properties)
    already = {k for k, _ in out}

    props = inst.properties or {}
    for name in fields.retrievable_properties:
        key = f"properties.{name}"
        if key in already:
            continue
        value = props.get(name)
        if isinstance(value, str) and value.strip(WHITE_SPACE):
            out.append((key, value))

    return out


def _link_stem(target: str) -> str | None:
    """The file stem of a link target, with hyphens read as spaces.

    **Written out rather than taken from the platform's path type**, and that is the fix rather
    than a style preference. The Rust reference reached this through ``Path::file_stem``, which
    splits on ``\\`` as well as ``/`` when the target happens to be compiled for Windows — so
    the same corpus composed two different strings depending on where the binary was built, and
    no fixture could have said which was right. The rule is four lines and belongs where all
    three languages can read it:

    1. the segment after the last ``/``, ignoring a trailing one;
    2. never ``.`` or ``..``, which name a directory and not a node;
    3. cut at the last ``.`` that is not the first character, so ``plan.v2.yml`` is ``plan.v2``
       and a dotfile keeps its name;
    4. ``-`` reads as a space, because ``lower-canyon`` is two words to an embedder and one
       token to nobody.
    """
    name = target.rstrip("/").rpartition("/")[2]
    if not name or name in (".", ".."):
        return None

    dot = name.rfind(".")
    stem = name[:dot] if dot > 0 else name
    return stem.replace("-", " ")


def _related(links: list[CorpusLink]) -> list[str]:
    """The names of the things this node points at, in the order it wrote them.

    A class definition is not one of them. ``.ont.yml`` is the contract an instance is written
    *under*, not a thing it relates to, and every instance of a class carries the same one — so
    admitting it would add one constant phrase to every vector in the class and distinguish
    nothing.
    """
    out: list[str] = []
    for link in links:
        if link.target is None or link.target.endswith(".ont.yml"):
            continue
        stem = _link_stem(link.target)
        if stem is not None:
            out.append(stem)
    return out


def compose_embed_text(inst: CorpusInstance, fields: EmbedFields) -> str:
    """Compose the text this node is embedded as.

    Three parts, joined by a single space, each omitted when it is empty:

    1. the node's ``label``;
    2. everything :func:`of` returns, each value trimmed at the end and joined by newlines;
    3. ``Related: <names>.`` over the link targets.

    **The class is not in the text, and that is a decision rather than an omission.** It is a
    column beside the vector, where a filter can use it exactly; folded into the text it would
    put one identical phrase in every vector of a class, which pulls the whole class toward one
    point and distinguishes no member of it from any other. The consumer RFC-0007 was filed
    about folded it in, which is one of the two places the two assemblers disagreed.
    """
    parts: list[str] = []

    if inst.label:
        parts.append(inst.label)

    said = "\n".join(v.rstrip(WHITE_SPACE) for _, v in of(inst, fields))
    if said:
        parts.append(said)

    names = _related(inst.links or [])
    if names:
        parts.append(f"Related: {', '.join(names)}.")

    return " ".join(parts)


#: How many characters a passage may hold, label included. See the Rust reference's
#: ``PASSAGE_MAX_CHARS`` for why the budget is in characters and why 1,200.
PASSAGE_MAX_CHARS = 1200


def _is_cut(c: str) -> bool:
    # Two characters only, so three languages agree on what a cut is.
    return c == " " or c == "\t"


def _cut(chars: str, room: int) -> tuple[str, str]:
    if len(chars) <= room:
        return chars, ""
    for j in range(room, 0, -1):
        if _is_cut(chars[j]):
            return chars[:j], chars[j + 1 :]
    return chars[:room], chars[room:]


def split_passages(label: str, text: str, max_chars: int) -> list[str]:
    """Split a composed embed text into passages no longer than ``max_chars``, each led by the label.

    RFC-0060 §2.1; see the Rust reference's ``split_passages`` for the argument. A text that
    fits comes back whole. A text that does not begin with ``label`` and a space is read as
    having no label. Lengths are counted in code points.
    """
    if len(text) <= max_chars:
        return [text]

    if label and text == label:
        body = ""
    elif label and text.startswith(label + " "):
        body = text[len(label) + 1 :]
    else:
        label, body = "", text

    half = max_chars // 2
    if len(label) + 1 > max_chars - half:
        label = _cut(label, half)[0].rstrip(" \t")
    prefix = f"{label} " if label else ""
    room = max(max_chars - len(prefix), 1)

    i = body.rfind(" Related: ")
    if i >= 0 and "\n" not in body[i:]:
        lines = body[:i].split("\n") + [body[i + 1 :]]
    else:
        lines = body.split("\n")

    pieces: list[str] = []
    for line in lines:
        if all(_is_cut(c) for c in line):
            continue
        rest = line
        while rest:
            head, rest = _cut(rest, room)
            if head:
                pieces.append(head)

    passages: list[str] = []
    current: list[str] = []
    current_len = 0
    for piece in pieces:
        if current_len > 0 and current_len + 1 + len(piece) > room:
            passages.append(prefix + "\n".join(current))
            current, current_len = [], 0
        if current_len > 0:
            current_len += 1
        current.append(piece)
        current_len += len(piece)
    if current_len > 0:
        passages.append(prefix + "\n".join(current))

    return passages or [label]
