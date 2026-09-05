# RFC-0019 — Citing a corpus you cannot revise (`cites:`)

- **Status:** Implemented
- **Track:** I14
- **Relates to:**
  - RFC-0018 (the query this must not silently widen)
  - RFC-0002 (the node model a citation is a field of)
  - RFC-0001 (the report contract the checks emit on)
  - RFC-0003 (the light binary this must run in)
  - RFC-0005 (the MCP surface that already distinguishes foreign nodes by `origin`)
- **Versioning layers touched:** template (the node model gains a field; `prelude/guidelines`
  gains a rule) / tooling (`yidam` CLI implements the checks) — **no parity-surface change and
  no MCP contract change**; see [What this does not touch](#what-this-does-not-touch)
- **Downstream reference case:** Project BOSC (watermark-directory)
- **Parent epic:** #251 (E3) — this RFC is #265, and #266, #267 and #268 are built against it

## Summary

`tonpa` installs a corpus you can search and cannot cite. E3 reads that as a deferral waiting
to be taken up. It is not — it is a **rule in force**, argued and shipped, and the first job
of this RFC is to say so.

`prelude/guidelines/agent-conduct.md` does not merely record #194's decision. It states the
rule normatively, gives the reason, and prescribes what to do instead:

> **A foreign node may be read. It may not be an edge target.** An edge is a claim and the
> constitution governs who may assert one; a citation into a corpus with a different ontology,
> its own electors, and its own revision history is a different object. […] **put what you took
> into a local node, in this corpus's terms, tagged at this corpus's standard, saying in prose
> where it came from.** That local node is the thing this sangha becomes accountable for.

That argument is correct and this RFC does not reverse it. **`cites:` is not an edge**, does
not appear in traversal, is not licensed by `edge_policy`, and does not make a foreign node
reachable. What it changes is the last four words of the prescription: *saying in prose where
it came from* is invisible to every tool this repository has. The practice is right and
unenforceable, and an unenforceable practice is one the gate cannot tell from its absence.

So: a structured, checkable form of the citation the guidelines already require, carrying the
one thing that survives the boundary — **a verbatim span** — and the two things that let
`tonpa update` say what moved: the pin it was read at, and the standing it was read at.

## Problem

### The epic's four premises, checked against the code

Each of these is written into #265, #266 or #267. Each is wrong as stated, and the corrections
are the design.

#### 1. There is no "pinned commit" to resolve a foreign node at

#266 says a cited foreign node "must exist at the pinned commit". No such resolution is
available. A `.yiz` bundle is a **tarball**, not a repository — `manifest.yml`, `corpus/`,
`skills/`, `decisions/`, `index/`
([`bundle.rs:98-109`](../../yidam/cli/src/cmd/bundle.rs#L98-L109)) — so there is no history to
resolve against and no object store to ask.

What exists is one string. `manifest.yml` carries `commit`, and that field is
`head_commit_short(root)` ([`model.rs:188`](../../yidam/cli/src/model.rs#L188)) — `git
rev-parse --short`, whose **length git chooses from the producing repository's object count**.
Bundling the eight-node example produces `commit: "8d35441"`. The same commit, bundled from a
larger repository, is spelled with more characters.

So a citation cannot pin *into* a dependency. It can pin *to the bundle it was read from*, and
the check available is: **the node exists in the bundle installed here, and the bundle's
manifest commit is the one the citation names.** Those are two different findings — the second
is `tonpa update`'s whole subject — and collapsing them is what #266 warns against for a
different pair.

#### 2. Path dependencies have no pin at all, and are first-class

`deps.rs` states it in its own header: a path dependency is "not fetched, not hashed, not
locked, because hashing a working tree that changes under you records nothing"
([`deps.rs:9-12`](../../yidam/cli/src/deps.rs#L9-L12)). It is also the **only** form that
supports a development loop, and `resolved()` deliberately lets it win over an unpacked
directory of the same name ([`deps.rs:133-136`](../../yidam/cli/src/deps.rs#L133-L136)).

A citation form that requires a pin therefore either excludes the dependency form people
actually develop against, or admits an unpinnable citation. This RFC admits it, and makes the
absence visible rather than silent: a citation into a path dependency records no commit and is
reported as **unpinned**, every time it is checked.

#### 3. "A foreign `[inference]` under a local `[verified]`" is already answered, in the negative

#265 names this as the case to reason about. `agent-conduct.md` reasons about it and reaches a
harder conclusion than a rule about tag arithmetic:

> **A foreign tag is the producer's tag.** `[verified]` in a dependency means *that* corpus's
> electors accepted *that* provenance. It does not transfer, and you cannot check it: a bundle
> carries `corpus/`, `skills/` and `decisions/` — no sangha, no elector register, no resolution
> history. You receive conclusions without the apparatus that made them accountable. The rule
> that a derived assertion travels only as far as the weakest claim beneath it still holds, and
> across this boundary **"weakest" is genuinely unknown.**

The min-tag rule cannot be computed across a boundary the apparatus does not cross. This RFC
does not compute it, and does not let the tooling appear to. A foreign tag is recorded as an
**observation** — *this is what the producer said, at this pin* — never folded into a local
standing. What that buys is the thing #267 asks for and nothing else: when the producer demotes
a `[verified]` to `[open]`, the citing corpus can be *told*, and a person decides.

#### 4. A foreign link is an error today, reported by the check least able to explain it

Verified rather than reasoned about. Adding

```yaml
  - target: upstream::concept/hydrograph.yml
    relationship: cites-external
```

to a node in `examples/streamflow` produces:

```
ERROR [dangling-edge] Edge pointing at nothing — 1 finding(s)
  .yidam/corpus/reach/tailwater.yml: target does not exist: upstream::concept/hydrograph.yml
```

`dangling_edge` is a filesystem `exists()` test
([`checks.rs:533-550`](../../yidam/cli/src/cmd/lint/checks.rs#L533-L550)). `unlicensed-edge`
and `edge-target-class` never see the link at all, because `instance_links` drops every target
that does not resolve to another instance. So the failure mode of the obvious syntax is: an
Error, from the check with the least to say about it, and silence from the two checks whose
subject it is.

### What is actually missing

Not the ability to point at a foreign node — the guidelines forbid that on purpose. What is
missing is that **the prescribed alternative leaves no trace a tool can read.** "Saying in
prose where it came from" cannot be checked for existence, cannot be checked for accuracy,
cannot be found when the dependency moves, and cannot be counted. A corpus that follows the
practice perfectly and one that ignores it are, to `lint`, `graph-check`, `query` and
`tonpa update`, the same corpus.

## Proposal

### `cites:` is a field of a node, not an edge

A new optional top-level key on a corpus instance, beside `links:` and never inside it:

```yaml
class: reach
label: Tailwater reach
description: |
  Flow below the outlet works, set by operations rather than by weather. Base-flow separation
  is not meaningful here in the usual sense [inference] — the upstream corpus treats the slow
  component as groundwater-sustained, which a regulated reach does not have.
cites:
  - package: upstream
    node: concept/base-flow-separation
    commit: 8d35441
    tag: verified
    span: >-
      the slowly varying component sustained by groundwater discharge
links:
  - target: ../gage/canyon-outlet.yml
    relationship: measured-by
```

Five fields, and each one is load-bearing:

| Field | Required | Why it is there |
|---|---|---|
| `package` | yes | which dependency, as `tonpa.toml` names it — the same string `origin` carries |
| `node` | yes | `<class>/<name>`, unqualified: `package` already says whose |
| `commit` | for a fetched dependency | the `manifest.yml` commit this was read at |
| `tag` | no | the producer's standing **as observed**, so a demotion is detectable |
| `span` | yes | verbatim text from that node — the part that survives the boundary |

**`span` is the field the design turns on.** `agent-conduct.md` already argues for it in the
mirror direction, for claims leaving the repository:

> **Cite a span, not a node.** An external assertion names a **verbatim span** of the corpus
> node it rests on, and the gate asserts that span appears there character-for-character. This
> does not verify the inference; nothing can. It forces the actual sentence to sit beside the
> assertion, where the gap between them is visible to a reader.

Every word of that transfers inbound. A node reference alone rots invisibly: the node keeps its
name while its content is rewritten, and the citation still resolves. A span cannot rot
invisibly — it either still appears or it does not, and either way the gate can say. It is also
the only check available that does not require the producer's apparatus, which is exactly the
apparatus the boundary does not carry.

### What `cites:` is not

Stated as prohibitions because each one is a thing a reader will assume:

- **Not traversable.** `query` does not walk it, `neighbors` does not report it, and
  `instance_links` — the gate's own edge reader, and RFC-0018's — never sees it. `--across`
  (#268) queries the dependency set as a *scope*; it does not follow citations.
- **Not licensed by `edge_policy`.** `unlicensed-edge`'s own rationale draws this line
  already ([`checks.rs:1460-1462`](../../yidam/cli/src/cmd/lint/checks.rs#L1460-L1462)): *a link to the
  class file or into the catalog is a citation, not a relationship.*
  A class's `edges:` bounds relationships; a citation is not one, and asking a class to
  declare which foreign corpora its instances may cite would be asking the ontology a
  governance question it has no vocabulary for.
- **Not a claim-tag input.** No local tag is computed from a foreign one. See premise 3.
- **Not `origin`.** `NodeView::origin` says whose node *this result* is, on a retrieval that
  spans dependencies. `cites:` says whose node a *local* node leaned on. A node with a
  citation is still wholly local and the corpus is still wholly accountable for it.

### Four checks, all in the light build

#266 requires the light feature set, because derived-repo CI downloads a binary rather than
compiling one. All four read `.yidam/tonpa/<pkg>/` and `tonpa.toml` off disk; none needs
`--features tonpa`, which buys the network and nothing these want.

| Check | Severity | Fires when |
|---|---|---|
| `external-citation-unresolved` | Error | the package is not installed, or the node is not in it |
| `external-citation-span-drift` | Error | the node is there and the span is not in it, character-for-character |
| `external-citation-pin-moved` | Warn | the installed bundle's manifest commit is not the cited one |
| `external-citation-unpinned` | Info | the citation names a path dependency, which cannot be pinned |

The first two are #266. The third is #267's data, available at lint time rather than only on
update — which matters because `tonpa update` is `--features tonpa` and CI is not.

**A missing package and a missing node are one check with two messages, not two checks.**
#266 asks for them not to be collapsed, and they are not: the finding names which it is and
what the repair is. They are one *check id* because the baseline ratchet keys on
`(check id, node)`, and splitting them would let a repository bless the absence of a
dependency and inherit a free pass on every node inside it.

**Severity is Error for the first two on purpose,** against the instinct that a dependency
problem should not gate a corpus. A citation that does not resolve is worse than no citation:
it asserts a provenance that is not there, which is `dangling-edge`'s own argument, and it is
the property that makes the local graph trustworthy applied across the boundary. #266 puts it
exactly right — without this, "a citation into a dependency is unverifiable in exactly the way
a local link is not, which inverts the property that makes the local graph trustworthy."

`external-citation-pin-moved` is a **Warn** and not an Error because a stale dependency is a
normal state — `agent-conduct.md` says so — and because escalating it would mean a producer's
release could turn a consumer's CI red without the consumer changing anything. That is the
failure mode a pin exists to prevent.

### What a violation is, in one sentence each

- **Unresolved:** the citation names something that is not there.
- **Span drift:** the citation names something that is there and says something else.
- **Pin moved:** the citation is honest about a state that is no longer the installed one.
- **Unpinned:** the citation cannot be honest about a state, and says so rather than implying
  one.

The second is the interesting one and is the reason `span` exists. Span drift is the only one
of the four that can catch a dependency **revising a node out from under a claim** — the
scenario #267 opens with — and it catches it without any cooperation from the producer, without
history, and without the sangha the bundle does not carry.

## The ambition test, and whether this passes it

E3's test: *two corpora by different authors, each gating on the other's integrity, neither able
to silently break the other.*

This passes the second clause and **deliberately fails the first**, which is worth being
explicit about rather than claiming a win.

- *Neither able to silently break the other* — yes. A producer revising a cited node breaks
  the consumer's build at the next `lint`, loudly, on span drift. A producer deleting one
  breaks it on unresolved. Neither requires the producer to know the consumer exists.
- *Each gating on the other's integrity* — **no, and not symmetric.** The consumer gates on
  what it cited. The producer gates on nothing about the consumer, and must not: a producer
  whose CI could be turned red by a stranger's citation has handed strangers a veto over its
  own corpus. Federation here is deliberately one-directional, and the direction is the one
  where the accountability already lies — with the corpus making the claim.

## What this does not touch

- **No parity-surface change.** `sdks/parity/VERSION` stays 0.7.0. `find_reachable` and
  `find_citations` model an edge as `{from, to}` with no relationship and no origin; a citation
  is not an edge and would have to arrive as a new function on every implementation to serve
  one consumer.
- **No MCP contract change.** `cites:` is part of the node, so `get_node` returns it the moment
  the node model carries it. A dedicated tool would be RFC-0005's business and E6's.
- **No ontology change.** No class declares anything about citations. See above.
- **No change to `links:`.** The obvious alternative — a `cites-external` relationship on a
  link — is rejected below.
- **The rule in `agent-conduct.md` stands.** The guidelines gain a paragraph saying the
  prescribed prose has a structured form and what its fields mean; the prohibition it sits
  under is unchanged. That is a **patch** bump to the template layer by `VERSIONING.md`'s own
  table — a documentation change to prelude — and the node-model field is the minor part.

## Alternatives considered

- **A `cites-external` relationship inside `links:`.** The shape the epic's title implies, and
  it fails on three counts. It puts a citation in the list `instance_links` reads, so every
  traversal has to learn to skip it — RFC-0018's executor, `neighbors`, `graph.rs`, and the
  editor — and a traversal that forgets is a traversal that crosses the boundary silently. It
  has nowhere to put `span`, `commit` or `tag`, because a link is `{target, relationship}` and
  widening it widens every edge in the system. And it inherits `dangling-edge`'s filesystem
  test, whose message for this case is demonstrably the wrong one.
- **A relative path into `.yidam/tonpa/<pkg>/corpus/`,** mirroring how catalog citations are
  written. Genuinely tempting: `dangling-edge` would then check existence for free, and
  `instance_links` would drop it for free. Rejected because it breaks on path dependencies —
  whose corpus is outside the repository, so the link escapes it, and whose absence on a
  fresh clone is a *normal state* `deps.rs` explicitly declines to report. It also records no
  pin, no span and no tag, which is most of what the citation is for.
- **Reversing the rule: foreign nodes become edge targets.** The reading E3's framing invites.
  Rejected, and the reason is not conservatism: the argument in `agent-conduct.md` is that an
  edge is a claim under a constitution, and a corpus cannot revise a class definition it does
  not own. E3's own sequencing agrees — it says foreign edges over an untyped graph "would
  export the untypedness across a repository boundary, where it is far more expensive to fix."
  Typing the graph (E1) removed the stated blocker and did not touch the constitutional one.
- **Computing a local tag from the foreign one.** Rejected by the constitution's own reasoning:
  across this boundary "weakest" is genuinely unknown, so any computed tier would be a number
  with no denominator. Recording the observed tag and reporting movement is the most that can
  be said truthfully.
- **Verifying the producer's `[verified]` by fetching its sangha.** Not available: bundles carry
  no elector register and no resolution history. Adding them to the bundle format is a real
  design and it is E8's, not this one.
- **Nothing — keep prose-only citation.** The status quo, and the honest case for it is that
  the practice is already correct and this only adds enforcement. Rejected because the epic's
  finding survives it: a corpus following the practice and one ignoring it are indistinguishable
  to every tool, so the practice degrades exactly where no one is watching, and `tonpa update`
  has nothing to report against.

## Open questions

- **Span normalization.** `span` is compared character-for-character, which means a producer
  reflowing a paragraph breaks every citation into it. That is arguably correct — the text
  changed — and arguably intolerable. Lean: exact for now, with the finding's message showing
  both strings, and revisit with evidence from a real pair of corpora rather than from taste.
- **Multiple spans per citation.** One node may support a claim through two sentences. Lean:
  two `cites:` entries naming the same node, rather than a list inside one, so each span drifts
  independently.
- **Whether `query` should offer `cites:` as a projection.** `--select cites` is cheap and
  makes the boundary visible in an answer, which is #268's whole concern. Lean: yes, once #268
  lands and there is an `--across` result to distinguish it from.
- **Citations into a corpus that is not a dependency at all.** A published bundle read once and
  not installed. Out of scope: with nothing on disk, no check here can run, and a citation
  nothing can check is the prose form with extra syntax.
