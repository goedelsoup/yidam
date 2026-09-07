# Reading the corpus

Half of the loop is writing. This is the other half.

The loop in a yidam-derived repository is **find a gap → retrieve → correct → settle a
phase**, and the first two steps are reading. There are commands for them. Use them.

## Why this file exists

Across roughly 170 corpus-building sessions in twelve derived repositories, **62,276 shell
invocations named `yidam`.** The gates were run constantly — `lint` 1,953 times,
`graph-check` 1,289, `status` 571. The reading commands were run **zero** times: `query` 0,
`pack` 0, `estimate` 0, `neighbors` 0.

What was used instead was the filesystem. Against paths under `.yidam/corpus/`, the same
sessions ran 17,755 `cat`/`sed`/`head`/`tail` reads, 10,938 `grep`s and 9,907 `ls`es. A
corpus of 691 nodes was being navigated with `grep`.

The cost is measurable and it is paid in context. **83 of 1,140 unique user turns — 7%, spread
across eleven of the twelve repositories — are the sentence "This session is being continued
from a previous conversation that ran out of context."** Reading a corpus with `cat` until the
window ends is what that sentence sounds like from the inside.

None of this was a preference for `grep`. The reading surface appeared in the agent-facing
guidance of a derived repository **zero times**, so no agent working in one had any way to
learn it existed. That was the defect, and this file is its repair.

## `grep` cannot answer the question you are asking

`grep` searches text. The corpus is a **typed graph**, and the three things you most often
want from it are not textual facts:

| You want | `grep` gives you | The corpus knows |
|---|---|---|
| the nodes one relationship away | every line mentioning a filename | which links are edges, in which direction, and which class they land on |
| whether the corpus covers something | zero lines, meaning nothing in particular | whether the class exists, holds instances, and what values it actually has |
| enough context to write from | whatever you cat until the window ends | the full answer, ordered, filled to a budget, with an account of what did not fit |

A `grep` that returns nothing and a class nobody has written into are the same output. They
are not the same fact, and the difference is exactly where an agent starts writing from its
own weights under a claim that it worked in the corpus.

## The four commands

Substitute this corpus's own class and relationship names throughout. The shapes are what
matter.

### `yidam query` — a typed path

```sh
yidam query 'reach -measured-by-> gage'
```

```
2 result(s)
  Canyon Outlet gage  (gage/canyon-outlet.yml)
  Valley Bridge gage  (gage/valley-bridge.yml)
2 step(s), 2 edge(s) walked, 4 of 8 node(s) read, ~39 token(s)
```

Whitespace around a hop is **required** — `-rel->` and `<-rel-` are single tokens, which is
what makes a hyphenated relationship name unambiguous. `<-rel-` walks the edge backwards.
Steps can be filtered: `reach[claim_tag=open]`, and `*` stands for any class.

It is checked against the ontology before it runs, so a misspelled class or relationship comes
back as a **diagnosis naming the near miss**, never as an empty result.

`--select` projects fields, `--limit` bounds what is shown but never what is walked, `--at
<ref>` answers as of a past commit, and `--across` reaches installed dependencies.

### The absence diagnosis is the reason to prefer it

An empty answer is where an agent invents. `query` says *why* it is empty, derived from what
the corpus states:

```sh
yidam query 'reach[claim_tag=verified]'
```

```
0 result(s)
  [absent] step 1: `reach` holds 2 instance(s) and none satisfies `claim_tag=verified`.
  The value(s) present are `inference`. This is a real empty result: the corpus has the
  nodes and does not have the value. (predicate-unsatisfied)
```

That is a different fact from *the class is not declared*, from *the class is declared and
empty*, and from *nothing authors that relationship* — and it names which one, every time.
**Coverage gaps are defects**, and this is the command that reports them.

### `yidam pack` — context for one goal, to a budget

```sh
yidam pack 'concept~"hydropeaking" <-exhibits- reach' --budget 4000
```

It returns the query's full answer as prose, filled to the budget, and then tells you what it
left out and why. This is the remedy for reading a corpus with `cat` until the window ends —
the pack is the artefact, so `yidam pack '…' > context.md` is the point of the command.

`~"…"` is a similarity anchor: it enters at the nodes closest in meaning to the text, when you
know what you are after but not which node holds it. An anchor is a starting point, not an
answer.

### `yidam neighbors` — one node's neighbourhood

```sh
yidam neighbors concept/hydropeaking.yml --depth 1
```

```
Hydropeaking — concept/hydropeaking.yml
4 neighbour(s) within 1 hop(s)
  -→ instance-of concept.ont.yml  (concept.ont.yml)
  -→ refines Low flow  (concept/low-flow.yml)
  ←- sources-from Canyon Outlet gage  (gage/canyon-outlet.yml)
  ←- exhibits Tailwater reach  (reach/tailwater.yml)
```

Undirected and untyped — it floods outward in both directions. That makes it the right first
move when you have a node and do not yet know what surrounds it, and the wrong one once you
know which relationship you mean. Depth grows fast: on a small corpus, depth 2 is most of it.

### `yidam estimate` — the quote before the charge

```sh
yidam estimate 'reach -measured-by-> gage'
```

```
2 node(s) match

  select                          chars    ~tokens
  node,class,label                  157         39
  node,class,label,description     1429        357
  a context pack                   1596        399
```

The traversal costs the machine what an answer costs and costs you a few hundred bytes. Run it
before a `pack` you are not sure will fit.

## Where each one belongs in the loop

| Step | Command |
|---|---|
| What is owed? | `yidam due` — index staleness, catalog TTL, unanswered questions, phases in flight |
| Where is the gap? | `yidam query` on the class you suspect, and read the **absence** |
| What is around this node? | `yidam neighbors <node>` |
| What will this cost? | `yidam estimate '…'` |
| Load context to write from | `yidam pack '…' --budget N` |
| Then write, then gate | `mise run ci` |

Reach for `grep` when you want a *string* — a phrase you half-remember, a spelling across the
tree. Reach for these when you want a *node*, an *edge*, or an *answer*.

## What is not being claimed

None of this makes reading files wrong. Opening a node you have already identified is exactly
what `cat` is for, and every one of these commands prints node ids so you can. What the
measurement found is not agents reading files — it is agents **searching** a typed graph with
a text tool, and paying for it in context windows that ran out.
