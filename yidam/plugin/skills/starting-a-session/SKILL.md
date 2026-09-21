---
name: starting-a-session
description: Use at the start of work in a yidam repository — a repository with a .yidam/ directory — when you do not yet know what is worth doing. One call says where the repository is in its loop, and what the next act is; a second discharges the one clock a tool can discharge. Triggers on "where should I start", "what should I work on", "what is due", "what is owed", "catch me up on this repository", "is anything blocked", "propose", "draft the commits", "what is next".
---

# Starting a session

A corpus is a practice, and a practice is performed **because it is time** rather than because
somebody suspected something. Before this, nothing said it was time: the clocks, the work in
flight and the gates each had a home, and an agent opening a session read none of them.

## First call: where am I

`cycle` answers it, in four halves. It composes the surfaces that already own each fact, so it
cannot disagree with them.

| Half | What it is |
|---|---|
| `owed` | four clocks — index staleness, a source past its TTL, how long a question has gone unanswered, how long a phase has been in flight |
| `in_flight` | the unsettled inquiry refs |
| `blocked` | what a gate would fail on **today** |
| `next` | one act per half, each carrying the finding, clock or declaration behind it |

Three things to read correctly, because each is easy to read as the opposite:

- **`passed` is true however much is owed, and being owed is not being broken.** A corpus with
  three expired sources is doing exactly what it is meant to do. Do not report `due` as a count
  of defects; the report is not a gate.
- **`blocked` is what would fail a gate, not everything a corpus was ever forgiven.** Accepted
  findings are inherited debt the repository agreed to, and they are not here.
- **`next` is ranked by the server, not by a declaration.** A corpus declares which *kinds* of
  phase it runs, never an order between them. Treat the list as an argument you may disagree
  with, and say so if you do.

`practice` carries what the corpus declared its work is aimed at. A corpus that declared nothing
says so there — that is a supported state, not a gap to fill in.

## Second call: discharge what you can

**One of the four clocks is dischargeable by a tool at all.** A source past its TTL is; the index
wants a build, and an unanswered question and a phase in flight both want a person. Deciding a
question is answered is a resolution event, and no tool performs one.

For that one, `propose` drafts the commits the corpus's own findings license. Three verbs —
`open`, `withdraw`, `close` — each asserting only what a finding or a declaration already
asserts. There is no `establish` here, no new node and no edge: proposing a synthesis would be
asserting something nobody authored.

What comes back:

- `proposals` — what was drafted, with the `check` behind each and the `detail` it quotes.
- `skipped` — a finding it could **not** draft about, with the reason. Read it. A finding
  silently not proposed about is the failure this tool exists to remove.
- `written` — the `branch` and the commits on it. **Null on a run with nothing to propose and
  null on a `dry_run`**; those are different facts, and `proposals` is what tells them apart.

Pass `dry_run` when you want the plan without the branch.

## Nothing merges itself, and say so

The commits go to a proposal branch and **nothing lands on the baseline**. A person reviews the
branch as commits, merges it, amends it, or rejects it by deleting it. When you report what you
did, report it as a proposal awaiting a person — not as work that landed. The commit records the
tool as its author and whoever ran it as its committer, which is a true account of what happened
and is not an adoption.

## If these two tools are not there

They are served only where the repository has said a server may write to it, so a server that
was not told refuses them with `capability-not-supported` and does not list them. That is a
statement you can read rather than a hole to discover. Working without them is ordinary: run the
reports from a terminal instead, and leave the writing to the person you are working with.

## Where the reasoning is

`docs/mcp-server.md` §3 in this repository, and RFC-0029 for why a write is a capability a
deployment declares rather than a transport it happens to have.
