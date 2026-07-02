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


_OPERATIONAL_VERBS = frozenset(
    ["extract", "refresh", "compute", "index", "bundle", "reconcile", "build", "fix", "regen"]
)


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
