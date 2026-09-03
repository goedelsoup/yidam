---
name: writing-a-corpus-commit
description: Use before writing any commit message in a yidam corpus — a repository with a .yidam/ directory. The leading verb of the subject is a closed vocabulary that `yidam lint --commits` reads, and the yidam MCP server can check a subject before it is written. Triggers on "commit", "git commit", "commit message", "which verb", "commit this work".
---

# Writing a commit in a corpus

In a yidam corpus the git history *is* the knowledge graph, and a commit is an event in it.
The leading verb of the subject — everything before the first `": "` — is what files that
event. The vocabulary is closed.

**Do not recall the vocabulary. Ask for it.**

Before you run `git commit`, call `check_subject` with the exact subject line you intend to
write. It is total and never fails: an unrecognized verb comes back as a finding in the
payload, not as an error.

What comes back, and what to do with each:

- `recognized` — false means the verb is outside the vocabulary. Do not commit it.
- `vocabulary` — the closed list, carried in the same response. Correct from it rather than
  calling again.
- `kind` — what the commit will be *filed* as. Derived from the verb, so it is the output of
  your choice rather than an input to it. Read it to check the verb you picked says what you
  meant.
- `violations` — each with the rule that broke and the severity the gate would report.

## The one that costs twice

A conventional-commits scope suffix — `vendor(yidam): …` — is read as the verb
`vendor(yidam)`, which is in no list. The verb then goes unrecognized *and* the commit is
misfiled, because classification falls through. `check_subject` reports that as its own
violation with its own repair. If you are used to conventional commits, check the first
subject you write here.

## Where the reasoning is

The vocabulary and the two kinds of event are defined in `GRAPH.md` under "Commit
vocabulary", in the corpus's vendored prelude at `.yidam/.vendor/prelude/GRAPH.md`. This
skill deliberately does not restate the list: a second copy of a closed vocabulary is a
second thing to hold in step, and the tool answers from the same predicate the gate uses.
Read the prelude for *why* a verb exists; call the tool for *whether* yours is one.

