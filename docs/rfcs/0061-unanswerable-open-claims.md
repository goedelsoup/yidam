# RFC-0061 — An open claim may declare itself unanswerable, and the questions clock stops counting it

- **Status:** Draft
- **Track:** I44
- **Relates to:**
  - #1506 (a corpus set `questions_after = 400` to quiet the clock; the evidence this answers),
    parent epic #1513
  - RFC-0037 (the `?` label marker, and the split between a question node and an open claim
    that moves this RFC off the node and onto the claim)
  - RFC-0031 and #789 (the scoped tag, `[verified — as proposed]`, whose grammar this extends)
  - RFC-0020 (a resolution event is a person's act, which is why the clock proposes nothing)
- **Versioning layers touched:**
  - **The claim grammar.** `claims.rs` reads one new detail shape on an `[open]` tag. The
    standing vocabulary (`verified`, `inference`, `open`) does not change.
  - **The MCP contract.** An `open_questions` entry gains `unanswerable`. The change is
    additive, 0.34.0 to 0.35.0.
  - **Reports.** `due`'s questions clock, `status` and `open-questions` each gain a figure or a
    section. `status --format json` gains `unanswerable`.
  - **Lint.** `claim-tag-malformed` gains two messages. It gains no check.
  - **The prelude.** `GRAPH.md` gains one paragraph under "An open question is marked in its
    label".
  - **The bump.** It is minor for the CLI. Nothing on disk in any corpus moves, so no
    migration and no upgrade note are owed.
- **Downstream reference cases:** measured read-only on 2026-11-23 at the checked-out head of
  every derived repository on the measuring machine. That is 18 corpora. The clock figures for
  ohio-campaign-finance-2025-2026 come from a clone, at its pin, 0.24.0.
  allen-county-ohio-democratic-party, which the issue also cites, is not on this machine.

## Summary

An open question that the public record cannot answer stays open forever. The questions clock
counts it forever, and the clock has no way to hear that the corpus already knows. One corpus
answered by raising `[due] questions_after` to 400 commits. That silences every question
alike, including the ones that can still be answered.

The issue proposes `status: unanswerable` on a question node. The measurement moves it off the
node. In the corpus that motivated the issue, every one of the 408 open questions is an
`[open]` tag in the prose of an ordinary node, and none is a `?` question node. A raw scan of
18 corpora finds 4,989 bare `[open]` tags, and only 10 brackets that begin with `open` and say
anything more. A node is where a question is filed, not what
a question is.

This RFC puts the terminal state on the claim:

```
Who lent the $5,775 is [open — unanswerable: not in the public record].
```

- **The standing stays `open`.** The tag is still an open claim. `open-questions` still lists
  it, and the MCP contract's five arms do not move.
- **The reason is required, and the list of reasons is closed.** It is one of
  `not in the public record`, `out of scope` and `superseded`.
- **The questions clock skips a node only when every open claim on it is unanswerable.** One
  answerable question keeps the node on the clock.
- **The count stays visible.** `status`, `due` and `open-questions` each report the
  unanswerable figure beside the open one, so declaring a question unanswerable removes it
  from what is owed and not from what is seen.

## Problem

### 1.1 What was measured

**ohio-campaign-finance-2025-2026.** The issue cites 450 open questions. At its head on
2026-11-23 the figure is 408.

- 341 of the 408 are past 400 commits, so even the raised interval does not quiet the clock.
  The oldest has stood for 847 commits.
- All 408 are inline `[open]` claims. By class: committee 194, contribution 91, organization
  42, contest 38, expenditure 31, rule 9, person 2. One more is an edge.
- None carries a `?` label. No class in the corpus is a question class.

A sample of the sentences shows two populations, side by side on the same nodes:

| Can be answered later | Cannot be answered from the record |
|---|---|
| "so who wins is [open]" (ballots still outstanding) | "Who lent the $5,775 is [open]" |
| "Whether a 2026 event's costs fall after 15 October … is [open]" | "Who funds it is [open]" |
| "which rows the cover leaves out is [open]" | "the shares cannot be checked here [open]" |

Only the author can tell these apart. A count of 408 that mixes them tells the author nothing
about either.

**Every corpus on the machine.**

| Corpus | Open questions | `?` labels | Nodes in a question class |
|---|---|---|---|
| allen-county-ohio | 476 | 1 | 17 |
| ohio-democratic-party-rural-caucus | 447 | 0 | 0 |
| ohio-campaign-finance-2025-2026 | 408 | 0 | 0 |
| traffic-design | 386 | 0 | 0 |
| allen-county-ohio-affordability-2026 | 181 | 0 | 67 |
| urban-land-markets | 138 | 0 | 0 |
| ohio-136th-assembly-hb-96 | 120 | 0 | 0 |
| winter-warmer | 109 | 7 | 7 |
| land-use-activity | 87 | 15 | 0 |

The rest hold fewer than 100 each. The two corpora with a question class already record
state on it, in two different ways:

- allen-county-ohio-affordability-2026 has a `status` property: open 53, answered 10,
  narrowed 4.
- allen-county-ohio writes a `claim_tag` (verified 9, open 8) and a `closed:` date.

### 1.2 Why the node is the wrong place

A node-level status would serve the 91 nodes in a question class and the 23 with a `?` label.
It would not reach the dominant population at all. In ohio-campaign-finance, an author who
wanted to quiet one unanswerable question on a committee would have to declare the whole
committee unanswerable. That committee may carry three other questions that a report due next
month will settle.

RFC-0037 already drew this line. `[open]` is the standing of a claim inside a node, and `?`
says the node is the question. The thing that turns out to be unanswerable is the claim.

### 1.3 Why raising the interval is the wrong answer

`questions_after = 400` is a corpus-wide setting. It tells the clock to wait longer for every
question, so the questions that can be answered are owed later too. It also does not work:
341 questions are past even 400. The setting was the only lever there was, which is the
defect.

## Design

### 2.1 The form

A terminal open claim is an `[open]` tag whose detail is `unanswerable`, then a separator,
then a reason:

```
[open — unanswerable: not in the public record]
[open — unanswerable: out of scope]
[open — unanswerable: superseded]
```

- Any of the scoped tag's separators may stand between `open` and `unanswerable`, and between
  `unanswerable` and the reason. `[open: unanswerable — out of scope]` reads the same.
- The reason is matched after collapsing whitespace and ignoring case, and it must match one
  of the three exactly. Nothing may follow it inside the bracket. A citation goes beside the
  tag, as it does for every other tag.
- The same detail is read wherever an open tag is read: in prose, in a property its class
  declared `type: claim`, and in a link's `claim_tag`. In a field, the bracketless
  `open — unanswerable: superseded` is read too, as a bare `open` is today.

**Only `open` takes it.** `[verified — unanswerable: …]` is not a tag. A claim that has been
verified has been answered.

**Today this form is not a tag at all.** `unanswerable` is not one of the narrowing heads, so
`tag_from_inner` refuses it as a citation folded into the bracket. No corpus writes it, so no
existing count moves when it starts to read.

### 2.2 The three reasons

| Reason | Meaning |
|---|---|
| `not in the public record` | The answer exists, and no source the corpus can reach holds it. |
| `out of scope` | The question is real, and answering it is not this corpus's work. |
| `superseded` | A later question replaced this one, and the prose says which. |

The list is closed because a reason is something a reader filters on. An open list would let
each corpus coin its own, and the count by reason across corpora would be a count of
spellings. Adding a fourth is a contract change.

The issue proposed these three. They cover the unanswerable shapes in the §1.1 sample.

### 2.3 What reads it

**`has_open_claim` is unchanged.** A terminal claim is still an open claim. The predicate's
four node arms and its edge arm keep their membership, and `open-questions` lists exactly the
entries it lists today.

**A new companion, `open_claim_state`,** answers the question the clock needs. For a node it
returns one of three values:

- **none:** the node has no open claim;
- **open:** at least one open claim on the node is not terminal;
- **unanswerable(reason):** every open claim on the node is terminal.

The arms combine as follows:

- **A `?` label** counts as a non-terminal claim only when the node has no other open claim.
  A question node whose body says `[open — unanswerable: superseded]` is a terminal question,
  and the label is how the reader knew it was a question.
- **A finding carried under `yidam.findings`** with `standing: open` is never terminal. It is a
  question a tool raised, and the finding record has no reason field.
- **Edges are their own entries**, as they are today. An edge's terminal state is its own.

When the terminal claims on one node give different reasons, the state carries the first in
file order. The full list is available from `open-questions`, one claim at a time.

### 2.4 The questions clock

`open_at_head` keeps calling `open_questions_data`. It drops every entry that carries
`unanswerable`, so the clock counts and ages only what is still owed. Its summary line names
the excluded figure, so it is never silent. The figures here are illustrative:

```
questions  due  12 of 67 open question(s) past 100 commit(s), longest 412 (…); 341 unanswerable
```

With no `questions_after`, the undeclared line reads `N open question(s), M unanswerable`.

The remedy line does not change. Declaring a question unanswerable is a judgement about the
record, as answering one is, so the clock names a person and proposes no commit.

### 2.5 The other surfaces

- **`yidam open-questions`** lists unanswerable entries after the open ones, under their own
  heading, with the reason on each line. `--format json` gives each such entry an
  `unanswerable` field holding the reason. The field is absent on an entry that is still owed.
- **The MCP `open_questions` tool** answers the same way, at contract 0.35.0. A client older
  than 0.35.0 sees the same list it sees today, and no field it reads changes meaning.
- **`yidam status`** keeps `open_questions` as the total and adds `unanswerable` beside it.
  The text line reads `N open (M unanswerable)`, and drops the parenthesis at zero.
- **The coverage clock (#1491)** does not change. It asks whether every member of a declared
  universe has an instance. A node that carries an unanswerable question is still an
  instance, so the issue's worry that the question would be lost there does not arise.

### 2.6 Lint

`claim-tag-malformed` already reports a detail that reads as nothing. It gains two messages,
at its existing level, Warn:

- `[open — unanswerable: no filing exists]`: the reason is not one of the three, and the
  message lists them.
- `[verified — unanswerable: …]` or `[inference — unanswerable: …]`: only an open claim can be
  unanswerable.

No new check is registered.

## Alternatives considered

**A node-level `status: unanswerable`, as the issue proposed.** Rejected by §1.2. At most it reaches
the 91 nodes in a question class and the 23 with a `?` label, against 2,352 open questions in
the nine corpora tabled in §1.1. It forces an author to declare
a whole node to quiet one claim. And `status` is a key two corpora already use with their own
values, which RFC-0037 and the contract both forbid a tool to bless.

**A fourth standing word, such as `[unanswerable]`.** Rejected. `STANDING_WORDS` is frozen in
the MCP contract, and every counter in the repository reads exactly three. A fourth standing
would remove the claim from `open-questions`, which is the opposite of keeping it visible. And
an unanswerable claim is not a new kind of standing. It is an open claim with a known reason
to stay open.

**A per-node `questions_after` override.** Rejected. It moves the same blunt setting from the
corpus to the node. It still cannot say why, and it still mixes the answerable claims on a
node with the one that is not.

**An open list of reasons.** Rejected by §2.2.

## Migration

None is owed. The form is new, and no corpus writes it, so every count reads the same before
and after the change. A corpus that raised `questions_after` to quiet the clock may lower it
once its unanswerable claims are marked. That is the corpus's decision, and nothing prompts it.

## Falsifier

The design is wrong if marking becomes the new way to quiet the clock. That is the failure
`questions_after = 400` already showed.

- **Measure:** six months after release, the share of open claims marked unanswerable in each
  corpus that has adopted the form.
- **Revert line:** if any corpus marks more than half of its open claims unanswerable, the
  form has become a mute switch. In that case a terminal claim should cost something to write,
  for instance a `source:` that shows where the record was searched.

The design is also wrong if the reasons do not fit. If more than one in ten marked claims in
a corpus draws the unknown-reason warning, the list is too short.

## Unresolved

- **The 23 `?`-labelled nodes** have no reason form of their own. §2.3 makes one terminal
  through its body's tag. If question-class corpora find that awkward, a node-level form can
  be added later as sugar for marking every open claim on the node. It is not in this RFC.
- **`superseded` does not name its replacement** in a field. The prose says which question
  replaced it. A link from the old question to the new one may be worth a relationship of its
  own, once a corpus uses the reason.
