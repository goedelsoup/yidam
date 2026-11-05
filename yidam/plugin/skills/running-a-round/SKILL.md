---
name: running-a-round
description: Use in a yidam repository — one with a .yidam/ directory — when asked to work the loop rather than one task, or after a session brief from `yidam hook` lists acts to take. Runs one round at a time — read where the loop stands, take the first act a tool can take, verify it, record it — and hands back when only a person's acts are left. Triggers on "work the loop", "run a round", "do what is owed", "keep going until it is clean", "what can you do on your own here", "act on the brief".
---

# Running a round

A round is five steps: orient, choose, act, verify, record. Repeat it while there is an act a
tool can take, and stop when there is not. A round is small on purpose: one act, checked, and
written down, so the next round starts from a fact and not from a memory.

## Orient

Call `cycle`. If the session opened with a brief from `yidam hook session-start`, that brief is
the same report cut short, plus what moved since the last round; call `cycle` anyway for the
full `next` list.

## Choose

Take the **first** `next` item whose `kind` is `tool`. Not the easiest and not the one you
would have ranked first: the order is the server's argument, and a round that picks its own
order is a round nobody can compare with the last.

An item with no `kind` is a `person` act. So is one whose `kind` is `person`. Neither is yours
to take, however clear the remedy looks — deciding a question is answered is a resolution
event, and no tool performs one.

## Act

A `tool` act carries `argv`. Run exactly it, from the `root` the report names. Do not edit the
command, add flags, or run a neighbour that looks equivalent: `argv` is the act, not a hint
at one. If the act is drafting commits, `propose` writes them to a proposal branch and nothing
lands on the baseline.

## Verify

Run the gate the act was meant to move:

- `yidam lint` — every finding against the committed baseline. A new one is the act's.
- `yidam graph-check` — the links still resolve.

If either reddened, the act made it so. Undo it, say what happened, and stop: a round that
leaves the gate worse than it found it is not followed by another.

## Record

Run `yidam cycle --since-last --record` from a terminal. It says what arrived and what left
since the last round, then remembers this one under `.yidam/loop/`, which is never committed.
Read the `+` lines: anything there that the act did not intend is a finding, and it is yours to
report.

When the repository runs the loop's Stop hook, the turn's end records too, and tells the person
what moved. Recording by hand as well does no harm.

Commit what the act produced as its own knowledge event, with a subject `check_subject`
accepts. An operational change — a rebuilt index, a refreshed block — is an operational
commit, never mixed with an epistemic one.

## Hand back

Stop when the first `next` item is a `person` act, or `next` is empty. Report:

- each act taken, and the round delta it left;
- each `person` act still waiting, with the finding or clock behind it, worded as a request;
- anything a verify step undid.

A list of `person` acts is not a failure. Being owed is not being broken, and the acts a person
must take are the reason the loop has a person in it.

## If `cycle` is not there

The server lists it only where the repository declared it may write (`act`). Without it, run
`yidam cycle --format json` from a terminal: the same report, and the same steps apply.
