---
name: tagging-a-claim
description: Use before writing or editing a claim in a yidam corpus node — a repository with a .yidam/ directory. Claims carry an evidence tag, the exact spelling is what every tool reads, and the yidam MCP server serves the vocabulary. Triggers on "verified", "inference", "open question", "evidence tag", "claim", "writing a node", "editing a corpus node".
---

# Tagging a claim

A corpus node states things at different levels of certainty, and the tag is how a reader —
and every tool — tells them apart without re-reading the sources. Untagged inference is the
defect this vocabulary exists to prevent.

**Call `claim_tags`.** It takes no arguments and returns the three tags, what each means, and
how each may be written — tens of tokens instead of a prose file held in context.

## What the tool will tell you that is easy to get wrong

- **Write the tag alone.** `[verified — Pearl 2009]` matches nothing and is counted as *no
  claim at all*. It looks tagged to a reader and reads as bare assertion to every tool. Write
  the tag, then the citation beside it. `yidam lint` reports the near miss as
  `claim-tag-malformed`.
- **Two spellings, two places.** A property on a class declared `type: claim` accepts the
  bare token and the bracketed one. Prose is scanned for the bracketed form only.
- **A `[verified]` claim in a node that cites no catalog entry is reported**, as
  `verified-unsourced`. The repair is a citation *or* a demotion, and which one is a judgment
  — nothing proposes a promotion for you. Over-counting evidence is the flattering error.
- **A tag on a claim you cited from a dependency does not transfer.** The producer's standing
  is theirs. See the `citing-a-dependency` skill.

## Naming a tag rather than making one

A node whose subject *is* the evidence vocabulary has to write the tokens in order to talk
about them, and a scanner reading bytes cannot tell that from an assertion. It is resolved by
**grammar, not typography**, on four sentence shapes and no others — the table is in the
prelude section below. If you are writing prose about the tags, read it first; anything
inside a fenced block is shown rather than said and is never counted.

## Where the reasoning is

`.yidam/.vendor/prelude/guidelines/agent-conduct.md`, under "Mark claim confidence". The
prose carries the reasoning, which is what makes the vocabulary arguable; the tool carries
the content, which is what makes it cheap to obey. Neither replaces the other.
