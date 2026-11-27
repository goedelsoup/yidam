from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

from ._text import WHITE_SPACE, first_line


class CommitKind(str, Enum):
    Epistemic = "Epistemic"
    Operational = "Operational"


@dataclass
class CommitEvent:
    hash: str
    kind: CommitKind
    verb: str
    subject: str
    context: str | None = None


#: Verbs marking a commit as pipeline work rather than a change in understanding.
#:
#: Kept in sync with the commit vocabulary in ``prelude/GRAPH.md`` and with the
#: ``OperationalVerbs`` set in ``yidam/sdks/spec/graph.dfy``.
OPERATIONAL_VERBS = frozenset(
    [
        "extract",
        "refresh",
        "compute",
        "index",
        "bundle",
        "reconcile",
        "regen",
        "build",
        "implement",
        "scaffold",
        "catalog",
        "migrate",
        "fix",
        "vendor",
        "consume",
    ]
)

#: Verbs marking a commit as a change in understanding.
#:
#: :func:`classify_commit` does not consult this list — Operational is the explicitly
#: marked case and everything else is Epistemic, a totality the Dafny spec proves. The
#: list exists so that a verb belonging to *neither* set can be recognized as outside the
#: vocabulary entirely; see :func:`is_recognized_verb`.
EPISTEMIC_VERBS = frozenset(
    [
        "establish",
        "revise",
        "assess",
        "scope",
        "synthesize",
        "withdraw",
        "open",
        "close",
        "transport",
        "resolve",
        "adopt",
        "decide",
        "phase",
        "genesis",
        "overlay",
    ]
)

_OPERATIONAL_VERBS = OPERATIONAL_VERBS


def is_recognized_verb(verb: str) -> bool:
    """Whether ``verb`` is in the closed vocabulary defined by ``prelude/GRAPH.md``.

    Deliberately separate from :func:`classify_commit`, which must remain total: every
    commit gets a kind, including ones written before a repository adopted the vocabulary.
    Recognition asks whether the log is legible; classification asks what a commit did.
    """
    return verb in OPERATIONAL_VERBS or verb in EPISTEMIC_VERBS


def classify_commit(hash: str, message: str) -> CommitEvent:
    # The first line and the trim are the Rust SDK's rule, not ``splitlines`` and ``strip``:
    # those cut a subject at a U+2028, lost a line led by U+0085, and stripped U+001F from a
    # verb, so one commit had a different kind here than in the other two SDKs (#1666).
    line = first_line(message).strip(WHITE_SPACE)
    if ": " in line:
        pos = line.index(": ")
        verb = line[:pos].strip(WHITE_SPACE)
        subject = line[pos + 2:].strip(WHITE_SPACE)
    else:
        verb = ""
        subject = line

    kind = CommitKind.Operational if verb in _OPERATIONAL_VERBS else CommitKind.Epistemic

    return CommitEvent(hash=hash, kind=kind, verb=verb, subject=subject, context=None)
