from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


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
#: ``OperationalVerbs`` set in ``prelude/sdks/spec/graph.dfy``.
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
        "synthesize",
        "withdraw",
        "open",
        "close",
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
    first_line = message.splitlines()[0].strip() if message else ""
    if ": " in first_line:
        pos = first_line.index(": ")
        verb = first_line[:pos].strip()
        subject = first_line[pos + 2:].strip()
    else:
        verb = ""
        subject = first_line

    kind = CommitKind.Operational if verb in _OPERATIONAL_VERBS else CommitKind.Epistemic

    return CommitEvent(hash=hash, kind=kind, verb=verb, subject=subject, context=None)
