# RFC-0017 — Serving assertions, not documents (`claims` and the practice tools)

- **Status:** Draft
- **Track:** I12
- **Relates to:** RFC-0005 (the MCP contract this extends), RFC-0001 (the report contract),
  RFC-0002 (node-model unification — the two claim readers this reconciles), RFC-0003 (the
  light binary these must run in)
- **Versioning layers touched:** SDK+parity (the MCP contract, `mcp/VERSION` 0.4.0 → 0.5.0) /
  template (the Rust CLI implements it)
- **Parent epic:** #279 (E6)

## Summary

`serve --mcp` exposes five tools and **every one of them returns a node.** The unit of
assertion in this system is not the node — it is the **claim**: one statement, one standing
tag, `[verified]` / `[inference]` / `[open]`. An agent asking what a corpus takes as known
pays node-sized tokens for a claim-sized answer, and learns the standing of what it was
handed only if the tag happens to survive into the prose.

Separately, the **practice** an agent must follow — which edges a class licenses, which
verbs the commit vocabulary admits, what the three tags mean — is prose, reloaded into
context every session as a tax paid to *possibly* comply.

This RFC adds four tools: one that serves assertions at claim granularity, and three that
make the practice callable rather than remembered. It also settles the question that decides
whether the first of them is safe to build at all.

## Problem

### The unit of assertion is the claim, and nothing serves claims

A corpus node is 2–10 sentences by the model's own rule. An agent that wants *"what does this
corpus take as verified about X"* has one move available: retrieve nodes, read all of them,
and hope the tags survived. Filtering by standing — the query it should be making before it
writes anything — cannot be expressed at all.

### Two claim readers exist and they disagree

This is the part that makes the tool dangerous rather than merely missing.

**`yidam_core::corpus::extract_claims`** is one of the parity functions, implemented in three
languages and held to shared fixtures. It is **line-oriented over markdown**: a tag counts
when it is a *suffix* of a line, and every other non-link line becomes an `Implicit` claim.

**`claims.rs`** counts markers wherever they appear — prose and property values — and
discriminates *mention* from *use* by grammar: naming nouns, past-tense reporting verbs,
negations, plurals.

They do not merely differ in detail. They answer different questions, and for a yidam corpus
one of them answers the wrong one:

1. **yidam corpus instances are YAML, not markdown.** `extract_claims` over an instance file
   would read `class: gage` and `label: Canyon Outlet gage` as `Implicit` claims. The
   function was written for the markdown node model RFC-0002 describes; the corpus this
   server serves is `.yml`.
2. **`extract_claims` has no mention/use rule.** A line ending *"…is not [verified]"* strips
   to a `Verified` claim. `claims.rs` exists because that exact failure shipped: a derived
   repository published **1 verified claim against a true 0** for four commits, inside a
   `REGEN` block in `README.md`, which no human writes and everyone therefore trusts.
3. **`extract_claims` cannot see structural tags.** A corpus that stores `claim_tag: open` as
   a declared `type: claim` property — which E1 now validates and the ontology schema
   publishes — has claims that a suffix scan never finds.

### The safety rule is not "when in doubt, drop it"

`claims.rs` records the direction the danger runs in, and it is easy to read it as one rule
when it is one *invariant* with two opposite consequences:

> **Never make the corpus look better-evidenced than it is.**

Serving a `[verified]` the corpus did not assert breaks it in the obvious direction. But so
does **dropping an `[open]` the corpus did assert** — a corpus with its open questions
silenced reads as settled. Both are promotion; they just live on different tags.

That is what the note beside the reporting-verb list is recording. Treating the present
tense and the copulas as narration produced eight false positives, *"every one a live
claim"*, and *"All eight were in the promoting direction"* — because each one silenced a
claim that was being made. The same measurement retired the typographic rule for the same
reason: 80% of a mature corpus's `[open]` tags are backticked and are claims, and calling
backticks a mention understated that repository's open questions fivefold, on its front page,
with no diagnostic.

So a claims tool that applied a uniform *when in doubt, leave it out* policy would get
`[open]` exactly backwards. The rule this tool follows is the counter's, unchanged and not
re-tuned: `is_narrated`'s four shapes and no fifth, and no backtick rule.

### The practice is prose an agent reloads every session

`docs/post-genesis-measurement.md` establishes when a norm survives: *"Prose holds a norm
inside the act and loses a step after completion."* The commit vocabulary held for humans
because `lint --commits` echoes it back at commit time. An agent has no equivalent, and the
echo it needs is earlier still — **before** the act, as a call.

## Proposal

### The decision: `claims` serves from the discriminating reader

**`claims` extracts using `claims.rs`'s rules, not `extract_claims`.**

`extract_claims` keeps its job — it is the SDK's markdown node parser and one of the parity
functions, and nothing here changes its contract or its fixtures. It is simply not the thing
to expose for a YAML corpus whose tags are load-bearing.

The rule that follows, and the one to hold on to: **serve the tag or serve nothing.** A claim
is emitted only where the standing is read, never inferred. There is no `Implicit` arm in
this tool — an untagged sentence is prose, and prose is what `get_node` is for.

Three further things the extraction must inherit rather than re-derive, each of which a
fresh implementation gets wrong in a way its own tests would pass:

- **Fenced blocks are masked; inline code is not.** A node explaining the vocabulary inside a
  fence is not asserting it; a tag in backticks mid-sentence usually is.
- **Structural tags are read only from properties the class declared `type: claim`.** A bare
  `open` under an undeclared key is a word, not a standing.
- **A structural value that is already bracketed is one claim, not two.** The counter handles
  this by subtracting a tally; a *list* cannot, because it has to know which occurrence is
  the duplicate. This tool dedupes by position in the bytes.

To keep the two from drifting, the invariant is stated as a test rather than as a hope:

> the number of claims `claims` serves for a node equals the number `count_in_node` counts
> for it, tag by tag.

One extraction, one count, one discrimination shared. A future edit to the grammar rules
moves both or fails.

### The four tools

| Tool | Tier | Answers |
|---|---|---|
| `claims` | `core` | assertions with their standing, filterable by standing and class |
| `licensed_edges` | `ontology` | what a class may link to, and what it declared nothing about |
| `check_subject` | `core` | is this commit subject in vocabulary, and what would it be filed as |
| `claim_tags` | `core` | the three tokens and what each means |

`licensed_edges` takes a **new capability**, `ontology`. A projected or on-disk mirror can
hold nodes and edges and still have no `.ont.yml` — it can back `graph` and not this. Per the
contract's own rule, optional is not the same as absent: such a server declares
`"ontology": false` and its cases are skipped rather than passed.

The other three are `core`. `check_subject` and `claim_tags` answer from constants compiled
into every server, and `claims` needs only node content, which every server has by definition.

### None of them re-derive anything

Each tool calls the surface that already governs, so a server and its gate cannot come to
different answers:

| Tool | Calls |
|---|---|
| `claims` | `claims.rs` extraction + `ClaimFields`, already loaded into `ServerState` |
| `licensed_edges` | `yidam_core::ontology::parse_class` for the edges, **keyed by the filename stem** — see below |
| `check_subject` | `cmd::vocabulary::check_subject` — the same function `yidam vocabulary --check` returns, violations and all |
| `claim_tags` | `claims::{VERIFIED, INFERENCE, OPEN}` |

`check_subject` is the clearest case: the function exists, it already carries the
scope-suffix rule and reads its severity from the gate rather than restating it, and the tool
is one match arm calling it.

**`licensed_edges` keys a class by its filename, not by its `class:` field.** The two
disagree, and which one governs is decided by the gate: `load_classes` uses the stem always,
and `unknown-class` compares an instance's `class:` against the set of stems. `parse_class`
prefers the field and falls back to the stem — correct for an SDK reading one file, wrong for
a tool that must answer the same way the gate does. This is not hypothetical: the schema
compiler shipped in #258 followed the field and emitted `class/station.json` mapped to
`.yidam/corpus/station/*.yml` for a class whose instances live in `gage/` — a schema applied
to nothing, asserting a `const` no instance carries. Fixed here, in the same change, because
it is the same question.

`claim_tags` carries a one-line gloss per token, which is the one piece of new prose here.
The glosses are **pinned by test** against `prelude/guidelines/agent-conduct.md`, which
defines the tokens: the test asserts that document names exactly these three and no fourth.
The prose stays where the *reasoning* lives; the tool carries the *content*.

### What a claim carries, and what it must not

A served claim carries its text, its standing, its node, and the **catalog sources that node
cites**. That last one cannot come from the node model the server already has: prose
citations — `[Pearl 2009](../../catalog/pearl-2009.md)`, the form the conventions actually
prescribe — never enter `NodeView.links`, so the citation set is computed with
`lint::checks::linked_paths`, the same resolver `catalog-uncited` gates on. Not by matching a
slug: that defect failed an error-severity gate on a node containing no citation, because
connector crates are named after the sources they fetch.

Two things it must **not** carry:

- **A standing computed over the supporting chain.** The guidelines describe a minimum-over-
  supporting-claims rule and no code implements it. A field named for it and filled with the
  node's own tag would manufacture a tier nobody computed.
- **A dependency's claims, unmarked.** #194 settled composition as retrieval-only: a
  dependency is searchable and is never an edge target. Its assertions are *its* corpus's,
  and this tool serves the local corpus only rather than quietly presenting a foreign claim
  as this repository's. Citing across that boundary is E3's argued change, not a side effect
  of a retrieval tool.

### Absence, where it is cheap

`claims` returns `total` beside `returned`. An agent told "here are 5 claims" and an agent
told "here are 5 of 41 claims" can take different next actions, and only the second can
decide to ask for more. This is the `LlmsPack` receipt principle at its cheapest; the full
per-goal version is #282 and depends on E2.

**#282 landed as `pack`, at contract 0.7.0.** It shares the whole-corpus pack's fill rather
than re-deriving it, so the two cannot come to spend a budget by different rules, and it adds
the one field this cheap version cannot carry: `omitted_by_class`. `total` beside `returned`
says how much was dropped; the per-goal receipt says what kind, which is the half a caller can
act on. Building it also named a property the whole-corpus pack has always had and never
stated — **the receipt is the floor**: a budget too small to hold the account itself cannot be
met, and the pack says `over_budget` rather than dropping its own receipt to fit.

`licensed_edges` distinguishes *"this class licenses these three relationships"* from *"this
class declares no edges, so it has said nothing"* — the E1 silence rule, surfaced. An agent
told the second knows it is choosing rather than complying.

## The procedural hole this closes

Adding a tool to this contract touches five places, and **the build passes if you forget the
most important one.** `tools/list` is derived from `tools.json`; `tools/call` is a hardcoded
match. A tool added to the contract and not to `call` is advertised by the server and errors
when invoked — and the E2E list assertion still passes, because both sides read the same
file.

This RFC adds the check that makes the two halves agree: **every tool the server lists must
answer a call without reporting `unknown tool`.** Argument errors are fine; not existing is
not.

## Cost

- One minor contract bump, `0.4.0` → `0.5.0`, additive, in the three places
  `versioning_layers.rs` already pins together.
- One new capability, `ontology`, which other servers must answer for honestly.
- Four `call` arms, four case files, one new claim-extraction function beside the counter
  that shares its discrimination.
- The light build carries all four: none of them needs the vector index, so they arrive in
  the build most consumers actually install and are exercised on every pull request rather
  than only in the full-feature job.

## Alternatives considered

- **Serve `extract_claims` directly**, as #281 implies. Rejected on the three grounds above;
  the *observation* that claims are the unit is right and the *implementation* is a markdown
  parser for a different node model.
- **Reconcile the two readers into one parity function.** Attractive, and much larger: it
  changes a parity contract held in three languages, and the discrimination rules are
  measured against corpora only this repository has. Worth its own RFC if the SDKs ever need
  claim extraction over YAML; not a prerequisite for serving one.
- **A `standing` filter on `retrieve` instead of a new tool.** Rejected: `retrieve` returns
  nodes and ranks them by similarity. A claim is not a node and the standing filter is not a
  ranking; folding them makes one adaptive tool into two tools wearing one name, which is the
  shape the contract already rejected for `query`/`semantic_search`.
- **Read the tag glosses from `agent-conduct.md` at runtime.** Rejected as fragile — it makes
  a tool's answer depend on parsing prose in a vendored file that may be absent. The test-time
  pin gets the same anti-drift property without the runtime dependency.

## Open questions

- **Should `claims` accept a `node` argument, or is that `get_node` with extra steps?**
  Proposed: yes, because "what does *this* node assert, and at what standing" is the question
  an agent asks after a traversal, and answering it costs one filter.
- **Does `licensed_edges` grow to properties?** It would then be misnamed. If an agent needs
  the property contract too, the answer is probably that the compiled JSON Schema (#258) is
  already that, served as a resource rather than a tool.
