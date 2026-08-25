# RFC-0018 — The query surface — typed traversal bounded by the ontology (`yidam query`)

- **Status:** Draft
- **Track:** I13
- **Relates to:** RFC-0016 (the report JSON contract results are emitted on), RFC-0005 (the
  MCP contract the anchored form joins), RFC-0001 (report conventions and golden fixtures),
  RFC-0003 (the light binary this must run in), RFC-0017 (which draws the same distinction
  between the markdown node model and the YAML corpus instances this walks)
- **Versioning layers touched:** tooling (`yidam` CLI) — no parity-surface change and no
  ontology change; see [Is this a CLI surface or a parity function?](#is-this-a-cli-surface-or-a-parity-function)
- **Downstream reference case:** Project BOSC (watermark-directory)
- **Parent epic:** #250 (E2) — this RFC is #260, and #261, #262, #263 and #264 are built
  against it

## Summary

The corpus has six export formats so that it can be queried somewhere that is not yidam, and
no way to ask it a structured question here. The entire traversal surface is
`yidam neighbors --depth N`, which floods outward in both directions and reads no type.
Epic E1 typed the graph — classes, properties, relationships, targets, and whether a class's
vocabulary is closed — and *nothing traverses by any of it.*

This RFC specifies a small, total query surface: a **path** of typed, directed hops between
class patterns, entered either at a class or at a vector anchor, checked against the ontology
before it runs, and reporting what it cost to answer. It is deliberately narrower than a
pattern language, because the first thing built on it is a benchmark (#264) whose anchored arm
needs typed hops with a vector entry point and nothing more.

Two things in it are not obvious and are the reason the RFC is longer than the grammar:

1. **The typecheck cannot be "reject anything the ontology does not declare."** E1 measured
   that reading and rejected it: a non-empty `edges:` list says *these relationships exist*,
   not *and no others may*, and reading it as the second put 210 errors on a corpus that was
   doing nothing wrong. The rule here is three-valued and follows `edge_policy` — plus one
   extra rule that closes the typo hole the permissive reading opens.
2. **The query report carries its own cost.** `bench` is then a fold over query reports rather
   than a second instrumented traversal, so there is one accounting of nodes read and hops
   taken, exactly as there is one edge resolver.

## Problem

### The whole traversal surface is an undirected flood

`walk_neighbors` chains outbound and inbound edges unconditionally and filters on neither
relationship nor direction
([`graph.rs:155-164`](../../yidam/cli/src/cmd/graph.rs#L155-L164)):

```rust
let outward = edges.iter().filter(|(from, _, _)| *from == current) …
let inward  = edges.iter().filter(|(_, to, _)| *to == current) …
for (next, relationship, direction) in outward.chain(inward)…
```

`relationship` and `direction` are carried out on the result rows as *labels*. They are never
inputs. There is no way to ask "the gages that measure this reach" as distinct from "everything
within two hops of this reach" — and on `examples/streamflow` the second is 91% of the corpus
at depth 2 and all of it at depth 3.

### E1 typed the graph and no traversal reads the types

`.ont.yml` now declares, and lint now enforces: the class an instance belongs to
([`unknown-class`](../../yidam/cli/src/cmd/lint/checks.rs#L382), Error), the properties it may
and must carry ([`undeclared-property`](../../yidam/cli/src/cmd/lint/checks.rs#L624),
[`missing-property`](../../yidam/cli/src/cmd/lint/checks.rs#L685)), the type of each value
([`property-type`](../../yidam/cli/src/cmd/lint/checks.rs#L732)), which relationships a class
licenses ([`unlicensed-edge`](../../yidam/cli/src/cmd/lint/checks.rs#L887)), and which class
each relationship may land on
([`edge-target-class`](../../yidam/cli/src/cmd/lint/checks.rs#L952), Error).

`unlicensed-edge`'s own rationale states the gap in as many words
([`checks.rs:938`](../../yidam/cli/src/cmd/lint/checks.rs#L938)):

> a relationship in no declaration is worth seeing, because **a traversal that walks by
> relationship will not find it**

The check is describing a traversal that does not exist. This RFC is that traversal.

### The certified traversal primitive is typeless too

`find_reachable` and `find_citations` are on the parity surface, implemented in all three
SDKs against shared fixtures. Their edge model is `{ from, to }` — the fixtures carry no
relationship name at all
([`fixtures/find_reachable/linear-chain.toml`](../../yidam/prelude/sdks/parity/fixtures/find_reachable/linear-chain.toml)):

```toml
edges = [
  { from = "corpus/a.md", to = "corpus/b.md" },
  { from = "corpus/b.md", to = "corpus/c.md" },
]
```

So the one traversal yidam certifies across three languages cannot express a typed hop either.
That matters for the design question below: making the query a parity function is not adding a
function, it is re-versioning the traversal model.

### The thesis has no executable, and the thing that would measure it needs this first

`docs/research/system` argues that ontological anchoring converts O(*n*) scan into O(depth)
lookup. #264's validity assessment established that the honest measurement against a top-*k*
baseline is **precision at fixed budget**, not cost alone — which requires an anchored arm that
can be *pointed at the right nodes*, by class and by relationship, rather than flooded outward.
That arm is a typed path with a vector entry. It is the narrowest thing that closes §4's
outstanding evidence line and serves §5, and it is what this surface specifies.

## Proposal

### The shape: a path, not a pattern graph

A query is a left-to-right **path**: an entry, then zero or more typed hops, each landing on a
class pattern. It returns the nodes matched by the **last** step, and reports what the walk
cost to get there.

```
yidam query 'reach[regulated=yes] -measured-by-> gage -sources-from-> concept'
```

A path is enough for every question `bench` asks, is trivially total (no joins, no
backtracking, no fixpoint), and reads in the direction the traversal runs.

### Grammar

```
query    := entry ( hop step )*
entry    := step | anchor
step     := class filter?
anchor   := "~" quoted filter?
class    := ident | "*"
filter   := "[" pred ( "," pred )* "]"
pred     := prop op value
op       := "=" | "!=" | "~"
prop     := ident
value    := bare | quoted
hop      := "-" rel "->"        ; follow the edge in its authoring direction
          | "<-" rel "-"        ; follow it backwards
rel      := ident
ident    := [A-Za-z_][A-Za-z0-9_-]*
quoted   := '"' … '"'
```

Whitespace between tokens is insignificant. On `examples/streamflow`:

| Query | Reads as |
|---|---|
| `reach` | every reach |
| `reach[regulated=yes]` | the regulated reaches |
| `reach -measured-by-> gage` | the gages measuring some reach |
| `concept <-exhibits- reach` | the concepts some reach exhibits |
| `~"reservoir release timing" -exhibits-> concept` | anchor by meaning, then walk one typed hop |
| `*[claim_tag=open]` | every node of every class that declares `claim_tag`, tagged open |

**Arrow direction is the authoring direction of the stored edge**, not the reading direction of
the query. A link lives on the node that wrote it, so `reach -measured-by-> gage` and
`gage <-measured-by- reach` name the same edges from opposite ends. This is the only part of
the syntax that has to be learned rather than guessed, and it is the part that makes
`direction: out` / `direction: in` in `.ont.yml` mean something at query time.

`*` matches every class. Under `*`, a property predicate restricts the candidate set to the
classes that declare that property (or that a universal declaration covers), and the report
names the classes it excluded — silently narrowing would make `*[claim_tag=open]` look like a
whole-corpus answer when it is not.

**Projection** is a flag, not syntax: `--select node,class,label` (the default),
`--select properties.parameter`, `--select body` to include the node's prose. Projection is
where the token cost lives, so it is where the benchmark's budget is set.

**Bounded result:** `--limit N`, default 50. The report always carries `matched` beside
`returned`, so a truncated answer can never be mistaken for a complete one. Ordering is
deterministic — corpus order, which `walk_corpus_instances` already sorts and `neighbors`
already depends on — except for an anchored entry, whose entry nodes are ordered by score.
Golden fixtures (#261) require this to be specified, not incidental.

### The typecheck rule

The rule the epic decision states is *"a query that does not typecheck against the schema is
rejected before it runs."* Applied naively, that rule is wrong, and E1 already measured why.
The rule below is what "typecheck" has to mean for an ontology that closes some things and
describes others.

#### Classes are closed

`unknown-class` is Error severity: an instance of a class with no `.ont.yml` is a lint failure.
So a query naming a class the corpus does not declare can only ever match nothing, and is
**rejected** with the declared class list and the nearest name.

The one exception is the one `unknown_class` itself carves out
([`checks.rs:383`](../../yidam/cli/src/cmd/lint/checks.rs#L383)): a corpus with no `.ont.yml`
files at all has no schema layer, which is a different problem from a misspelling. There, class
names are not checked and the report says the corpus is unschematised.

#### Relationships are closed only as far as `edge_policy` closed them

`EdgePolicy` ([`checks.rs:29`](../../yidam/cli/src/cmd/lint/checks.rs#L29)) is the field E1
added precisely because a non-empty `edges:` does not claim completeness. A hop naming a
relationship the class does not declare resolves as:

| `edge_policy` | Verdict | Why |
|---|---|---|
| `exhaustive` | **reject** | the class said its vocabulary is closed; this hop cannot ever match |
| `characteristic` | run, no note | the class said undeclared verbs are deliberate coinage |
| `unstated` (default) | run, with a `warn` diagnostic | worth seeing; gating would enforce a contract nobody wrote |

This mirrors `unlicensed-edge` exactly — Error on `exhaustive`, silent on `characteristic`,
Warn on `unstated` — because it is the same question asked from the other side. A query engine
that rejected what the gate permits would be a second, stricter opinion about the same
ontology, and the corpus that declares `characteristic` on all 18 of its classes could not
query its own 210 edges.

#### The realised vocabulary closes the hole `characteristic` opens

Permissiveness reintroduces the failure #261 forbids: under `characteristic`, a mistyped
relationship runs and returns empty, and an empty result is indistinguishable from a typo.

So one further rule, independent of policy:

> **A relationship name that no instance in the corpus has authored is rejected.**

The realised vocabulary — every `relationship:` actually written on a resolved link — is
already computed when the graph is built. A name nothing has ever written can only ever return
empty, so refusing it costs no expressiveness and closes the typo hole. As with `unknown-class`,
the rule is skipped when the realised vocabulary is empty: a corpus with no edges is a corpus
without a graph, not a corpus full of typos.

#### Targets are closed where the class named one

If a class declares the relationship but only toward class C, a hop asking for class B is
**rejected**, naming the declared targets. `edge-target-class` is Error severity for the same
reason: an edge to the wrong thing resolves, traverses, and exports, and is simply false. A
declaration with an empty `target` licenses every class, exactly as the check reads it
([`checks.rs:971`](../../yidam/cli/src/cmd/lint/checks.rs#L971)), and so does a query hop
against it.

#### Properties, by name and by value

A property name must be declared by the class, or matched by `.yidam/corpus/universal.yml` —
by exact name or by pattern ([`universal.rs:32-49`](../../yidam/cli/src/universal.rs#L32-L49)),
so `seeded_because` and `fy2024_profile` are queryable without being declared on sixteen
classes. An undeclared name is **rejected** with the class's declared list.

Values are checked with the rules `property-type` already applies
([`property_type_violation`](../../yidam/cli/src/cmd/lint/checks.rs#L732)), and reuse its
messages verbatim:

- `claim_tag = maybe` → *"`maybe` is not an evidence tag — write `verified`, `inference`, or
  `open`"*. A predicate the corpus could not satisfy is a query error, not an empty result.
- `date` values must be ISO-8601 at whatever precision they are written; `=` matches at the
  precision given, so `observed_on=2026-08` matches every day in that month.
- `string`, `text`, `ref` compare as text. `~` is case-insensitive substring containment —
  the same meaning `keyword_retrieve` gives it
  ([`tools.rs:167`](../../yidam/cli/src/cmd/serve/tools.rs#L167)), so "contains" means one
  thing in this repository.
- A type the corpus coined is unconstrained, as `property_type_violation`'s fall-through
  already leaves it. A check that failed on vocabulary it had not heard of would make coining
  impossible, and that argument does not weaken because the caller is a query.

#### An empty result and an unknown name are never the same answer

Everything above is arranged so that the two are distinguishable, and where a name is legal but
unproductive the report says so rather than shrugging. A query that typechecks and matches
nothing returns:

```
0 results — `bears-on` is not declared by `reach` (edge_policy: unstated) and no reach authors it
```

**Exit codes.** A query that does not typecheck is a *usage* error and exits **2**, the code
`main.rs` already uses for asking the binary for something that is not a thing
([`main.rs:525`](../../yidam/cli/src/main.rs#L525)). A query that runs and matches nothing exits
**0**. `query` is read-only and never gates — it appears in `--help` without the `*` that marks
the ten commands which write, and exit 1 is reserved for the gates.

### The report carries its own cost

`bench` must not instrument a second traversal. The rule `graph.rs` states for edge resolution
([`graph.rs:12`](../../yidam/cli/src/cmd/graph.rs#L12)) — *"a consumer resolving edges itself
would be re-deriving it — and would disagree with the gate … silently, in the direction of
'looks fine here'"* — applies with equal force to counting: two cost accountings will disagree,
and the one that flatters the thesis is the one that gets published.

So the RFC-0016 JSON payload carries, beside `results`:

```json
"cost": {
  "nodes_read": 3,
  "edges_walked": 4,
  "hops": 2,
  "chars": 1180,
  "tokens": 295,
  "corpus_nodes": 8
}
```

- **`nodes_read` is the count of nodes whose content the query had to evaluate or return — not
  the number of files the process opened.** The executor loads the whole corpus to resolve
  edges and always will; an agent consuming the result does not. The benchmark's arms are agent
  costs, and a field name that let those be confused would produce a number meaning nothing.
- `tokens` is `chars / 4`, the approximation `export_llms` already documents and names as an
  approximation ([`export_llms.rs:33`](../../yidam/cli/src/cmd/export_llms.rs#L33)). One
  estimate, one place, no second tokenizer.
- `corpus_nodes` is N, carried so `bench` can print the narrowing ratio and its ceiling
  (#264 decision 3) without recomputing the corpus.

The payload also echoes the **parsed query** as structured steps, so a programmatic consumer can
read back what it asked without re-parsing the string.

### The anchor, and the feature gate

`~"…"` resolves natural-language text to entry nodes through the existing index — the
mechanism `docs/research/system` describes, and #263's hybrid anchoring.

- `--anchor-k` defaults to **1**. An anchor is a starting point, not an answer; a five-wide
  anchor followed by a two-hop walk is a flood wearing a type. `retrieve`'s own default of 5
  ([`tools.rs:151`](../../yidam/cli/src/cmd/serve/tools.rs#L151)) is right for retrieval and
  wrong here. The report lists the resolved entry nodes with their scores, so what it anchored
  on is always visible, and `bench` can vary k as part of the budget.
- The anchor needs `--features index`, which is not the default build. **`query` degrades and
  says so; a measurement refuses.** In a light build an anchor falls through to
  `keyword_retrieve` and the report carries `"degraded": true` with a reason, reusing
  `retrieve`'s convention including `degraded_reason` present-and-null when not degraded
  ([`vector.rs:65-68`](../../yidam/cli/src/cmd/serve/vector.rs#L65-L68)). `bench`, by contrast,
  **errors out** on a light build rather than publishing a number, because #264's third finding
  is that a keyword baseline makes the measurement worthless and a summary printing a token
  count would not surface the flag.

This splits the decision #263 asks for exactly where the harm is: an interactive query with a
keyword anchor is degraded and useful; a benchmark with one is dishonest.

### `--at`: typechecking against a past ontology

Per #262, a query at a past commit typechecks against **that commit's** ontology, because that
is the schema the data obeys. Both ontologies are in hand, so the disagreement is reported
rather than resolved silently: when the same query would typecheck differently against HEAD,
the report carries an `info` diagnostic naming the step and both verdicts. Reconstruction uses
the existing history walk, never a checkout.

### What is deliberately not expressible, and why

1. **No joins or variable binding.** Not a pattern language. A path has one answer set and
   needs no unification.
2. **No disjunction across steps** — no `(a|b)`. One path, one traversal, one cost.
3. **No aggregation.** No `count`, no `group by`. The report carries the counts, so an
   aggregate would be a second way to compute a number that is already in the payload.
4. **No negation over paths** — "reaches with no `measured-by`" is not expressible. That
   question is `orphan-out`; "edges landing on the wrong class" is `edge-target-class`. **The
   questions a query would need negation for are already checks** — computed once, dated,
   baselined, and gated. A query answering them would be a second, ungated opinion about the
   same invariant, which is the drift RFC-0001 exists to stop.
5. **No transitive closure** (`-refines*->`) in the first cut. A recursive relationship is
   written out (`concept -refines-> concept -refines-> concept`), which is honest about its
   bound. See the open question — #264's measurement showed the two arms converging by depth 3
   on a small corpus, and an unbounded closure is exactly where the distinction stops existing.
6. **No writes**, ever. Read-only.
7. **No dependency corpora.** `retrieve` spans installed packages and labels each result with
   its `origin`; traversing across a package boundary is E3 (#251). The report carries
   `"scope": "local"` so a client never has to infer it.
8. **No full-text search over prose**, beyond `~` on declared properties and the anchor.
   That is `retrieve`'s job, and the anchor is how a query reaches it.

## Is this a CLI surface or a parity function?

#260 asks this and it deserves an argument rather than an assumption.

**Recommendation: a CLI surface. The SDKs consume results through the RFC-0016 report contract
and the MCP tool, and no parity function is added.**

The case for it:

- **The executor must read the resolved edge set, and resolution is the CLI's rule.** Three
  implementations of `normalize(dir.join(target))` are three chances to disagree with the gate
  about which edges are broken — silently, in the flattering direction
  ([`graph.rs:12`](../../yidam/cli/src/cmd/graph.rs#L12)).
- **The typecheck depends on three things that are not at parity and would all have to move.**
  `edge_policy` is not published on the compiled schema — `compile_class_schema` emits
  `x-yidam-edges` with `relationship`, `target` and `direction`, and no policy. `universal.yml`
  matching is a **regex**, and Rust's `regex`, JavaScript's `RegExp` and Python's `re` do not
  agree on what a pattern means; putting it at parity would pin three engines' behaviour while
  claiming to pin a schema. `property_type_violation`'s messages are the gate's, and the query
  reuses them verbatim on purpose.
- **`--at` needs the git history walk.** `lint::history::replay` exists in the CLI and in no
  SDK. Shipping it to three languages to support one command is a large surface for a small
  gain.
- **The parity discipline's stated purpose is to stop the tool and the model drifting.** A
  query engine is tooling. What must not drift is the *node model*, and that is already covered.
- **The certified traversal is typeless.** `find_reachable`'s edge model has no relationship at
  all. Making the query a parity function is not adding an eleventh function; it is
  re-versioning the traversal model in all three SDKs and every fixture that uses it.

The cost accepted, stated plainly: a Python or TypeScript consumer cannot execute a query
in-process. It shells out to the light binary or calls the MCP tool. That is the same bargain
RFC-0001 and RFC-0016 already struck — *the CLI computes verdicts; a client computes
affordances* — and RFC-0003's publishable reports-only binary is what makes it a bargain
rather than a barrier: `query` needs no `fastembed` and no `lancedb`, so it ships in the build
a Node or Python consumer can install. Only the vector anchor needs the heavy feature, and it
degrades.

The middle ground worth naming: putting **only the query parser** at parity — pure text to
structure, the same shape as `parse_markers` and `extract_links` — so an editor could parse a
query without shelling out. Rejected for now, because a parser without a typechecker gives an
editor half a diagnostic: it can say the string is malformed and cannot say `gauge` is not a
class. See the open question for the trigger that would change this.

## Migration & compatibility

- **New subcommand.** `yidam query`, read-only, in the light build. Nothing existing moves.
- **No ontology change.** Every field the typecheck reads — `properties`, `type`, `edges`,
  `relationship`, `target`, `direction`, `edge_policy`, and `universal.yml` — E1 already landed.
  A corpus that passes lint today is queryable today.
- **No parity-surface change.** `sdks/parity/VERSION` stays 0.7.0.
- **No report-contract break.** `FORMAT_VERSION` stays `"1"`
  ([`report.rs:32`](../../yidam/cli/src/report.rs#L32)) — a new payload is additive, and
  consumers must ignore what they do not know.
- **Forward note, not this RFC's change:** exposing the anchored query as an MCP tool (#263)
  bumps `mcp/VERSION` 0.5.0 → 0.6.0 and lands with that issue.
- **Adoption:** derived repositories get the command on the next binary. A corpus whose classes
  declare no `edge_policy` gets warnings on undeclared hops and correct results — no repository
  has to change anything to start querying.

## Alternatives considered

- **SPARQL over the existing RDF export.** Rejected, and this is the decision the epic settled:
  it imports a model with no notion of claim tags, commit ranges, or `--at`, and it moves the
  answer outside the tool that owns edge resolution. It is also a query over a *derived
  snapshot* — supporting `--at` would mean re-exporting history.
- **Extend `neighbors` with `--relationship` and `--direction` flags.** The cheap thing someone
  will propose, and it fails on both counts. It types one hop and cannot express a path whose
  hops differ, which is the shape the benchmark's anchored arm needs. And a flag pile has
  nowhere to put a typecheck: an unrecognised `--relationship` can only return empty, which is
  precisely the failure #261 forbids.
- **A general pattern language (Cypher-shaped) as the first cut.** Rejected as first: larger to
  specify, fixture and version, and #264's assessment establishes that the measurement needs
  typed hops with a vector entry and nothing more. It remains the direction closure and joins
  would grow toward, if a real query demands them.
- **A structured query object (YAML/JSON) instead of a string.** Rejected as the authored form:
  `bench` commits its goal set to the repository, and a one-line query is reviewable in a diff
  where a nested object is not. The structured form exists anyway — the report echoes the parsed
  steps — so a programmatic consumer is not forced to build strings.
- **Query as a parity function.** Argued above, with the cost recorded rather than waved away.

## Open questions

- **Bounded transitive closure** (`-refines-{1,3}->`). Genuinely useful for a recursive
  relationship, and genuinely where the two benchmark arms converge. Lean: defer until
  `bench --scaling` shows whether a bounded repeat still separates them at N = 512 and above.
  Deciding it before that measurement would be deciding it by taste.
- **Ordering operators** (`<`, `>`) on `date` and numeric properties. Lean: defer. There is no
  numeric property type today — `property_type_violation` knows `string`, `text`, `date`, `ref`
  and `claim` — so ordering would be lexical everywhere except `date`, which is a trap worth
  not shipping.
- **Anchor width.** Default `--anchor-k 1` is argued above but not measured. Lean: revisit once
  `bench` can report precision at k ∈ {1, 3, 5}; the answer is a measurement, not an opinion.
- **Client-side validation at parity.** The trigger would be the editor wanting to underline a
  bad query in an unsaved buffer, which is exactly what `universal.rs` already supports for
  classes. The minimum move is publishing `edge_policy` on `x-yidam-edges` in
  `compile_class_schema` (a parity bump), not moving the executor. Lean: not yet; revisit with
  RFC-0016's editor work.
- **`*` with a property predicate.** The proposal reports the classes it excluded. The
  alternative is refusing `*` with a predicate outright. Lean: report — silent narrowing is the
  failure, and refusing costs the one query (`*[claim_tag=open]`) most likely to be asked first.
