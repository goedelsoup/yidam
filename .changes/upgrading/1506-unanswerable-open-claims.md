### An open claim may say the record cannot answer it

**Write `[open — unanswerable: <reason>]` on an open claim the record cannot answer (#1506, RFC-0061).**

The reason is one of three: `not in the public record`, `out of scope` or `superseded`.
The claim stays open, listed and counted.
See [RFC-0061](rfcs/0061-unanswerable-open-claims.md) for why the list is closed.

**The questions clock in `due` stops counting a node once every open claim on it is marked.**

A marked question is named beside the count, as in `2 open, 1 unanswerable`.
With none marked, every `due` line reads exactly as before.
`status` names the count beside its open questions, and `--json` gains `unanswerable`.

**`open_questions` gains `unanswerable` and `reasons` (MCP contract 0.35.0).**

An entry whose every open claim is marked carries `unanswerable`, its first reason.
An entry with only some marked carries `reasons`.
Both are absent on a plain entry.

**`claim-tag-malformed` names two misuses of the form.**

A reason outside the three reads as a plain `[open]` and stays owed.
The word on a claim that is not open is no claim at all.
No corpus wrote the form before this, so no existing output changes.
