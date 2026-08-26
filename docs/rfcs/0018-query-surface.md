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
class patterns, entered at a class — by enumeration or by vector similarity — checked against
the ontology before it runs, and reporting what it cost to answer. It is deliberately narrower
than a pattern language, because the first thing built on it is a benchmark (#264) whose
anchored arm needs typed hops with a vector entry point and nothing more.

Three things in it are not obvious and are the reason the RFC is longer than the grammar:

1. **The typecheck cannot be "reject anything the ontology does not declare."** E1 measured
   that reading and rejected it: a non-empty `edges:` list says *these relationships exist*,
   not *and no others may*, and reading it as the second put 210 errors on a corpus that was
   doing nothing wrong. The rule here follows `edge_policy`, and the requirement that an
   unknown name never look like an empty result is met by a **diagnostic**, not by a second
   rejection rule — because a rejection rule strict enough to catch a typo is also strict
   enough to refuse legal queries.
2. **The query report carries its own cost.** `bench` is then a fold over query reports rather
   than a second instrumented traversal, so there is one accounting of nodes read and hops
   taken, exactly as there is one edge resolver.
3. **Two of E2's children are more expensive than their issues assume**, and this RFC says so
   rather than discovering it during implementation. `--at` cannot be built on
   `lint::history::replay`, and the `degraded` convention `serve` uses is not a mechanism this
   can reuse. Both are written up in full below.

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
([`unknown-class`](../../yidam/cli/src/cmd/lint/checks.rs#L425), Error), the properties it may
and must carry ([`undeclared-property`](../../yidam/cli/src/cmd/lint/checks.rs#L677),
[`missing-property`](../../yidam/cli/src/cmd/lint/checks.rs#L735)), the type of each value
([`property-type`](../../yidam/cli/src/cmd/lint/checks.rs#L886)), which relationships a class
licenses ([`unlicensed-edge`](../../yidam/cli/src/cmd/lint/checks.rs#L952)), and which class
each relationship may land on
([`edge-target-class`](../../yidam/cli/src/cmd/lint/checks.rs#L1018), Error).

`unlicensed-edge`'s own rationale states the gap in as many words
([`checks.rs:963`](../../yidam/cli/src/cmd/lint/checks.rs#L963)):

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

A query is a left-to-right **path**: an entry step, then zero or more typed hops, each landing
on a class pattern. It returns the nodes matched by the **last** step, and reports what the
walk cost to get there.

```
yidam query 'reach[regulated~yes] -measured-by-> gage -sources-from-> concept'
```

A path is enough for every question `bench` asks, is trivially total (no joins, no
backtracking, no fixpoint), and reads in the direction the traversal runs.

### Lexing: a hop is one whitespace-delimited token

This has to be settled before the grammar, because relationship names contain hyphens and so
do the arrows. Four of `examples/streamflow`'s six relationship names are hyphenated
(`instance-of`, `measured-by`, `downstream-of`, `sources-from`), and so are two thirds of
every `relationship:` written across the example and test corpora. A character-level grammar
in which `-` is both an ident continuation and the arrow's tail has no single parse of
`reach -measured-by-> gage`, and `assumption <-supports- study-design` has two
(`supports` / `study-design`, and `supports-study` / `design`). Both halves of that second
split are real names in the test corpora.

So the lexer is specified, and it is one rule:

> **Split the query on whitespace that is not inside `"…"` or `[…]`. Classify each token by
> its own shape. Never re-lex a token.**

| Token shape | Is |
|---|---|
| begins `-`, ends `->` | a forward hop; the relationship is the token with those affixes removed |
| begins `<-`, ends `-` | a backward hop; likewise |
| anything else | a step |

**Whitespace around a hop is therefore required**, and it is the only syntactic obligation the
surface imposes. `reach-measured-by->gage` is one step token naming no class and is rejected as
such. A relationship name is taken **verbatim** from inside the affixes and is never parsed
further, which is what makes `<-contrasts-with-` unambiguous.

Two consequences worth stating rather than discovering:

- A relationship name that begins or ends with `-`, or contains whitespace, `>`, `[`, `]`,
  `"`, or `~`, is not expressible. The typecheck rejects a query that would need one and says
  which name it is, rather than mis-lexing it. This is checkable — the declared and authored
  names are both in hand.
- `~` means *approximate* in both places it appears: similarity entry on a step
  (`reach~"…"`), substring containment in a predicate (`[regulated~yes]`). The two contexts
  are disjoint — one is outside brackets, one is inside — so no ambiguity arises from the
  reuse, and the shared reading is deliberate.

### Grammar

```
query    := step ( hop step )*
step     := ( class | "*" ) anchor? filter?
anchor   := "~" quoted
filter   := "[" pred ( "," pred )* "]"
pred     := prop op value
op       := "=" | "!=" | "~"            ; first operator character wins
prop     := ident
value    := bare | quoted
hop      := "-" rel "->"                ; follow the edge in its authoring direction
          | "<-" rel "-"                ; follow it backwards
class    := ident
rel      := verbatim token contents (see lexing)
ident    := [A-Za-z_][A-Za-z0-9_-]*
bare     := [^\s,\]"]+                  ; one or more; may begin with a digit
quoted   := '"' ( [^"\\] | '\\' ["\\] )* '"'
```

`quoted` escapes exactly two characters — `\"` and `\\`. There are no other escapes and no
literal newline inside a quoted value. Values are compared as UTF-8 with **no Unicode
normalization**, which matters because real corpus values carry em dashes:
`regulated: "yes — inherited from upstream"` is one of two `regulated` values in
`examples/streamflow`.

A `bare` value cannot contain whitespace, `,`, `]` or `"`; anything else needs quoting. It may
begin with a digit, so `observed_on=2026-08` is a legal predicate and `2026-08` is not an
`ident`.

### Examples, against `examples/streamflow`

| Query | Reads as | Matches |
|---|---|---|
| `reach` | every reach | 2 |
| `reach[regulated~yes]` | reaches whose `regulated` value contains "yes" | 2 |
| `reach[regulated=yes]` | reaches whose `regulated` value **is** `yes` | **0** |
| `reach -measured-by-> gage` | the gages measuring some reach | 2 |
| `concept <-exhibits- reach` | the concepts some reach exhibits | 2 |
| `reach~"reservoir release timing" -exhibits-> concept` | anchor by meaning within `reach`, then one typed hop | ≤ 1 hop from the anchor |
| `*[claim_tag=open]` | every node of every class declaring `claim_tag`, tagged open | 3 |

The third row is not a typo, and it is the reason the first example in this RFC uses `~`.
`reach.ont.yml` declares `regulated` as `type: string`, and both instances hold prose —
`"yes — inherited from upstream"` and `"yes — discharge set by outlet works"`. `=` is exact
text comparison, so `reach[regulated=yes]` correctly matches nothing. The row is kept in the
table because #261's first golden fixture will be one of these queries, and freezing an empty
answer under a caption that promises two is exactly the failure a fixture is supposed to
prevent.

**Arrow direction is the authoring direction of the stored edge**, not the reading direction of
the query. A link lives on the node that wrote it, so `reach -measured-by-> gage` and
`gage <-measured-by- reach` name the same edges from opposite ends. This is the only part of
the syntax that has to be learned rather than guessed, and it is the part that makes
`direction: out` / `direction: in` in `.ont.yml` mean something at query time.

**Projection** is a flag, not syntax: `--select node,class,label` (the default),
`--select properties.parameter`, `--select body` to include the node's prose. Projection is
where the token cost lives, so it is where the benchmark's budget is set.

**Bounded result:** `--limit N`, default 50. The report carries `matched` beside `returned`, so
a truncated answer is never mistaken for a complete one — at the cost stated under
[cost](#the-report-carries-its-own-cost): `matched` requires the full candidate walk, so
`--limit` bounds the projection and not the traversal. Ordering is deterministic — corpus
order, which `walk_corpus_instances` already sorts and `neighbors` already depends on — except
for an anchored entry, whose entry nodes are ordered by score.

### What the query may traverse

An edge is traversable when it is authored on an instance, resolves inside the corpus, and
lands on **another instance** — the same set `instance_links` reads
([`checks.rs:615-640`](../../yidam/cli/src/cmd/lint/checks.rs#L615-L640)), and the same rule
`unlicensed-edge` states: *a link to the class file or into the catalog is a citation, not a
relationship.*

This matters more than it looks. Every `gage` in `examples/streamflow` authors three link
kinds, and only one is traversable:

| Written | Target | Traversable |
|---|---|---|
| `instance-of` | `../gage.ont.yml` | no — the class file |
| `sourced-from` | `../../catalog/usgs-nwis.md` | no — a catalog citation |
| `sources-from` | `../concept/low-flow.yml` | yes |

Note `sourced-from` and `sources-from` sitting one character apart in the same file, one a
citation and one an edge. A query naming the wrong one must not silently return nothing, which
is what the next section is for.

Note also that this is **not** the set `graph.rs` reports: `neighbors` keeps in-corpus links
whose target file does not exist ([`graph.rs:335`](../../yidam/cli/src/cmd/graph.rs#L335)
filters on `resolved`, not on `exists`). A query cannot walk an edge to a file that is not
there, so it filters on `exists`. The two readers disagree, and this RFC picks the narrower
one deliberately rather than by accident.

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
([`checks.rs:407-411`](../../yidam/cli/src/cmd/lint/checks.rs#L407-L411)): a corpus with no `.ont.yml`
files at all has no schema layer, which is a different problem from a misspelling. There, class
names are not checked and the report says the corpus is unschematised.

#### Relationships are closed only as far as `edge_policy` closed them

`EdgePolicy` ([`checks.rs:38`](../../yidam/cli/src/cmd/lint/checks.rs#L38)) is the field E1
added precisely because a non-empty `edges:` does not claim completeness. A hop naming a
relationship the class does not declare resolves as:

| Class state | Verdict | Why |
|---|---|---|
| `edges:` empty or absent | run, no note | a policy on an empty list bounds nothing |
| `edge_policy: exhaustive` | **reject** | the class said its vocabulary is closed; this hop cannot ever match |
| `edge_policy: characteristic` | run, no note | the class said undeclared verbs are deliberate coinage |
| `edge_policy` unstated | run, with a `warn` diagnostic | worth seeing; gating would enforce a contract nobody wrote |

The first row is load-bearing and is easy to omit. `unlicensed_edge` short-circuits on an empty
edge list **before** it consults the policy
([`checks.rs:922`](../../yidam/cli/src/cmd/lint/checks.rs#L922)):

```rust
if class.edges.is_empty() || class.edge_policy == EdgePolicy::Characteristic { continue; }
```

— and that behaviour is pinned by a test named
`a_policy_without_edges_still_licenses_everything`. A table that consulted only the policy
would reject every hop out of a class that wrote `edge_policy: exhaustive` and no `edges:`,
which the gate licenses. That is a query engine stricter than the gate, and the corpus
declaring `characteristic` on all 18 of its classes could not query its own 210 edges.

#### The realised vocabulary diagnoses; it does not reject

An earlier draft of this RFC added a second rejection rule — *a relationship no instance has
ever authored is rejected whatever the policy* — to stop a typo returning an empty result under
`characteristic`. It is withdrawn, because it is wrong in four separate ways and the fourth is
fatal:

1. It rejects a relationship a class **declares** but no instance has authored yet — a young
   corpus's legal queries, refused as typos.
2. It does not catch the typo it exists for. `sourced-from` is authored twice in
   `examples/streamflow`, so a rule keyed on "has anyone written this" waves it straight
   through.
3. It breaks `--at`: an earlier commit's authored vocabulary is strictly smaller, so a query
   that is legal at HEAD is rejected against history — exactly where the disagreement
   diagnostic below is supposed to speak.
4. Rejection is the wrong instrument. #261 asks that *"an empty result and an unknown name must
   never be indistinguishable"* — that is a requirement on the **answer**, and a diagnostic
   satisfies it without refusing anything.

So the authored vocabulary is a diagnostic input. When a hop's relationship is undeclared by the
source class, the report says which of three situations it is in:

- **authored elsewhere in the corpus** — runs; the diagnostic names the classes that author it.
- **authored nowhere, but within one edit of a declared name** — runs; the diagnostic names the
  near miss. This is the case that catches `sourced-from` for `sources-from`, which the
  rejection rule missed.
- **authored nowhere and near nothing** — runs, returns empty, and the diagnostic says
  precisely that.

The authored vocabulary is a new aggregation — `graph_data` returns
`GraphReport { corpus_dir, nodes, classes }` and no distinct-relationship set exists anywhere
in the CLI today. It is computed over **every authored `relationship:`, including citations and
dangling links**, which is deliberately wider than the traversable set: a name written on a
link into the catalog is still a name this corpus uses, and reporting it as "authored nowhere"
would be false.

#### Targets are closed where the class named one

If a class declares the relationship but only toward class C, a hop asking for class B is
**rejected**, naming the declared targets. `edge-target-class` is Error severity for the same
reason: an edge to the wrong thing resolves, traverses, and exports, and is simply false. A
declaration with an empty `target` licenses every class, exactly as the check reads it
([`checks.rs:996`](../../yidam/cli/src/cmd/lint/checks.rs#L996)), and so does a query hop
against it. `*` on the target side is the query-side twin of that empty `target:` and licenses
every class in the same way.

#### Property names, and predicates by operator

A property name must be declared by the class, or matched by `.yidam/corpus/universal.yml` —
by exact name or by pattern ([`universal.rs:32-49`](../../yidam/cli/src/universal.rs#L32-L49)),
so `seeded_because` and `fy2024_profile` are queryable without being declared on sixteen
classes. An undeclared name is **rejected** with the class's declared list.

Predicate *values* are a separate question from predicate *names*, and the operator decides it.
`property_type_violation` ([`checks.rs:757`](../../yidam/cli/src/cmd/lint/checks.rs#L757))
takes a declared type and a value and no operator — it answers *may the corpus store this*, not
*may someone ask about this*. Using it operator-blind rejects satisfiable predicates:
`reach[claim_tag!=maybe]` is satisfied by every reach in `examples/streamflow`, and
`reach[claim_tag~ope]` matches `open`. So:

| Operator | Meaning | Operand check |
|---|---|---|
| `=` | exact comparison against the value as written | `property_type_violation` — asking for a value the type cannot hold is a rejection |
| `!=` | the negation of `=` | none; a `warn` diagnostic when the operand could never be a value of the declared type, since the predicate is then trivially true of every node that carries the property |
| `~` | contiguous, case-insensitive substring containment over the value's serialized text | none — it applies to every scalar type, so `claim_tag~ope` and `observed_on~2026` are both legal |

Three further rules the naive version leaves undefined:

- **An absent property never matches**, for any operator including `!=`. A reach with no
  `claim_tag` is not in `reach[claim_tag!=maybe]`. The alternative — three-valued logic — buys
  nothing here and makes `!=` mean two things depending on the corpus.
- **A list value matches if any element matches.** `claim_tag: [open]` is legal YAML that the
  claim counter reads as one claim, and `property_type_violation` accepts it
  ([`checks.rs:785-792`](../../yidam/cli/src/cmd/lint/checks.rs#L785-L792)); a predicate must read the
  same bytes the same way.
- **`=` on a `date` compares at the precision written**, so `observed_on=2026-08` matches every
  day in that month. Ordering operators do not exist — see the open questions.

Messages for the rejecting cases are `property_type_violation`'s own, verbatim:
`claim_tag=maybe` reports *"`maybe` is not an evidence tag — write `verified`, `inference`, or
`open`"*. A type the corpus coined is unconstrained, as that function's fall-through already
leaves it; a check that failed on vocabulary it had not heard of would make coining impossible,
and that argument does not weaken because the caller is a query.

#### `*` under every closure rule

`*` matches every class, and each rule above needs its `*` case stated or two implementers will
disagree about the exit code of one string:

- **As a step's class:** the candidate set is every class.
- **With a property predicate:** the candidate set narrows to the classes that declare that
  property, or that a universal declaration covers. If **no** class declares it, the query is
  **rejected**, exactly as a named class would be. Without this rule `*[regualted=yes]` returns
  an empty result at exit 0 while `reach[regualted=yes]` is rejected — the same typo, two
  answers, which is the failure this section exists to close.
- **As a hop's source (`* -rel-> B`):** the hop runs from every class the ladder above admits,
  and is rejected only when **every** class rejects it — that is, no class declares `rel` and
  every class is `exhaustive`. The report names the classes excluded and why.
- **As a hop's target (`A -rel-> *`):** always licensed; see the target rule above.

In every narrowing case the report names the classes it excluded. Silently narrowing would make
`*[claim_tag=open]` look like a whole-corpus answer when it is not.

#### Diagnostics are a new channel, not a reused one

The `warn` and `info` diagnostics above have nowhere to go today. `report.rs` defines
`FORMAT_VERSION`, `YidamBlock`, `Envelope`, `emit`, `Span` and `Format`, and the envelope
carries `format_version`, `yidam`, `root` and the flattened payload
([`report.rs:75-80`](../../yidam/cli/src/report.rs#L75-L80)) — there is no diagnostic or
severity channel in it. The crate's one `Severity` is lint's
([`lint/model.rs:17`](../../yidam/cli/src/cmd/lint/model.rs#L17)), and it is not reusable here:
a lint finding is keyed by a `&'static str` check id and a corpus `node` path and carries
`in_baseline`, tying it to the baseline ratchet. A query diagnostic has no check id, no node,
and no baseline — **it is about a step**.

So this RFC defines the channel, as a payload field rather than an envelope change:

```json
"diagnostics": [
  { "level": "warn", "step": 1, "code": "undeclared-relationship",
    "message": "`bears-on` is not declared by `reach` (edge_policy: unstated)" }
]
```

`level` is `warn` or `info` only. An error is not a diagnostic — it is the rejection, which
lives in the payload's `rejected` field and decides the exit code. `code` is drawn from a
closed, documented set so a client can branch without matching prose.

#### An empty result and an unknown name are never the same answer

Everything above is arranged so the two are distinguishable, and where a name is legal but
unproductive the report says so rather than shrugging:

```
0 results — `bears-on` is not declared by `reach` (edge_policy: unstated),
            and no node in this corpus authors it
```

#### Exit codes

A rejected query **emits its report and exits 1**. That is the shape four commands already
have — `doctor` ([`doctor.rs:601`](../../yidam/cli/src/cmd/doctor.rs#L601)), `regen`
([`regen.rs:126`](../../yidam/cli/src/cmd/regen.rs#L126)), `rename`
([`rename.rs:411`](../../yidam/cli/src/cmd/rename.rs#L411)) and `index-verify`
([`index_verify.rs:203`](../../yidam/cli/src/cmd/index_verify.rs#L203)) all print, then
`std::process::exit(1)`.

Exit **2** is not available and must not be borrowed. Its only site is `main.rs:614`, inside the
clap pre-dispatch arm for `InvalidSubcommand | ErrorKind::UnknownArgument`
([`main.rs:602-615`](../../yidam/cli/src/main.rs#L602-L615)) — reached *before*
`match cli.command`, so no command body can produce it — and
`tests/binary_pin.rs:140` pins it as the unrecognized-subcommand code. Returning `Err` from a
command body exits 1 with `Error: {:?}` prose and no envelope at all, which would make every
rejection this section specifies invisible to a JSON consumer.

A query that runs and matches nothing exits **0**. `query` still gates on nothing: exit 1 here
says the *query* was wrong, never that the corpus is. It appears in `--help` without the `*`
that marks the ten commands which write.

### The report carries its own cost

`bench` must not instrument a second traversal. The rule `graph.rs` states for edge resolution
([`graph.rs:12`](../../yidam/cli/src/cmd/graph.rs#L12)) — *"a consumer resolving edges itself
would be re-deriving it — and would disagree with the gate … silently, in the direction of
'looks fine here'"* — applies with equal force to counting: two cost accountings will disagree,
and the one that flatters the thesis is the one that gets published.

So the payload carries, beside `results` and `diagnostics`:

```json
"cost": {
  "steps": 3,
  "edges_walked": 5,
  "nodes_read": 7,
  "chars": 240,
  "tokens": 60,
  "corpus_nodes": 8
}
```

Every field defined, because half-defined cost fields are how a benchmark acquires a number
nobody can reproduce:

| Field | Is |
|---|---|
| `steps` | path length asked for — the count of `step` productions. **Not** `hops`: `graph.rs:277-282` reserves that name for hops actually taken, having already had this collision once |
| `edges_walked` | traversable edges the executor followed, summed over hops, counting an edge once per traversal |
| `nodes_read` | the union of nodes a predicate was evaluated against, nodes a hop's class was tested on, and nodes in the projection |
| `chars` | serialized size of `results` under the selected projection |
| `tokens` | `chars / 4`, the approximation `export_llms` documents and names as an approximation ([`export_llms.rs:33`](../../yidam/cli/src/cmd/export_llms.rs#L33)) |
| `corpus_nodes` | N, carried so `bench` can print the narrowing ratio and its ceiling (#264 decision 3) without recomputing the corpus |

**`nodes_read` is not process I/O.** The executor loads the whole corpus to resolve edges and
always will; an agent consuming the result does not. The benchmark's arms are agent costs, and
a field name that let those be confused would produce a number meaning nothing.

The numbers above are the real ones for
`reach[regulated~yes] -measured-by-> gage -sources-from-> concept` on `examples/streamflow`:
the predicate is evaluated against 2 reaches, `measured-by` walks 2 edges to 2 gages,
`sources-from` walks 3 edges to 3 distinct concepts, and the default projection of those 3
serializes to 240 characters. **`nodes_read` is 7 of 8** — the anchored arm reads seven eighths
of this corpus to answer a three-step query.

That is the unflattering reading, and it is printed here on purpose. It is also #264's second
finding restated as an executable: `examples/streamflow` is below the arithmetic floor of the
claim, `bench` on it is a regression guard rather than evidence, and a cost block whose worked
example quietly reported 3 would have been the first place that stopped being visible.

One honest consequence of reporting `matched` beside `returned`: computing `matched` requires
the full candidate walk, so **`--limit` bounds the projection — `chars` and `tokens` — and not
the traversal.** That is the right bound for #264, where the budget is what the agent reads.

### The anchor is class-qualified

`class~"…"` resolves natural-language text to entry nodes of that class through the existing
index — the mechanism `docs/research/system` describes, and #263's hybrid anchoring.

**The class is required** (or `*`), and that is a correction to the obvious design. A bare
`~"reservoir release timing" -exhibits-> concept` cannot be typechecked before it runs: the
hop's verdict depends on the source class's `edge_policy`, and the source class is whatever
retrieval happens to return. `exhibits` is declared only on `reach`, so an anchor landing on a
gage would take a different branch of the ladder — decided after retrieval, in a surface whose
headline promise is that queries are checked before they run. Requiring the class makes the
check static, makes property predicates on the anchor well-defined, and maps onto the
`class_filter` argument `retrieve` already threads through both its vector and keyword paths
([`tools.rs:155`](../../yidam/cli/src/cmd/serve/tools.rs#L155)). `*~"…"` remains available and
takes the `*` rules above.

- `--anchor-k` defaults to **1**. An anchor is a starting point, not an answer; a five-wide
  anchor followed by a two-hop walk is a flood wearing a type. `retrieve`'s own default of 5
  ([`tools.rs:154`](../../yidam/cli/src/cmd/serve/tools.rs#L154)) is right for retrieval and
  wrong here. The report lists the resolved entry nodes with their scores, so what it anchored
  on is always visible, and `bench` can vary k as part of the budget.
- **The anchor is local.** `keyword_retrieve` chains `state.dep_nodes` after `state.nodes`
  ([`tools.rs:226-229`](../../yidam/cli/src/cmd/serve/tools.rs#L226-L229)) — correct for
  retrieval, where an agent should be told the answer lives in a corpus this repository cites.
  A query labelled `"scope": "local"` must not silently enter through a dependency's node, so
  the query's anchor restricts to local nodes on both paths.

#### The feature gate, and what "reuse the convention" actually costs

**`query` degrades and says so; a measurement refuses.** In a light build an anchor falls
through to keyword search and the report says so; `bench` **errors out** rather than publishing
a number, because #264's third finding is that a keyword baseline makes the measurement
worthless and a summary printing a token count would not surface the flag.

The `degraded` / `degraded_reason` convention is **borrowed, not reused**, and the difference is
worth a paragraph because the naive reading makes this look free:

- Those keys are fields on an MCP `tools/call` result, not on the RFC-0016 envelope — `report.rs`
  has neither.
- The "present-and-null when not degraded" half lived in `cmd/serve/vector.rs`, which was
  `#[cfg(feature = "index")]` — so it did not exist in the build that can actually degrade,
  and the light path hard-coded the other branch. **One convention, written in two files, only
  one of which every build compiles.**

So the query report defines `anchor.degraded` and `anchor.degraded_reason` with the same key
names and the same present-and-null discipline, and the *reason strings* come from one place:
`Retrieval::degraded_reason` distinguishes "no index built" from "this binary cannot read the
index this corpus has", and lifting it out of `serve` is the one refactor this RFC asks for —
so `query` and `serve` cannot come to disagree about why retrieval is degraded.

> **Landed in #263**, and the paragraph above is left in the past tense it now needs rather
> than rewritten, because the split it describes is the argument for the move. `Retrieval` is
> [`retrieval/mod.rs:56`](../../yidam/cli/src/retrieval/mod.rs#L56) and the embedder is
> [`retrieval/vector.rs`](../../yidam/cli/src/retrieval/vector.rs); `vector::retrieve` became
> `vector::search` and returns *scores* rather than a response, so both branches of the
> `degraded` shape are now built in one ungated place
> ([`tools.rs:191`](../../yidam/cli/src/cmd/serve/tools.rs#L191)) — which is the half of the
> problem this section identified and did not propose fixing.

### `--at` is not free, and #262 should know it

#262 argues that `--at` is nearly free because `lint::history::replay` already reconstructs the
corpus at a commit. It does not, for this purpose, and the gap is structural rather than a
matter of plumbing:

| What a query at a commit needs | What `replay` does |
|---|---|
| that commit's ontology | `is_instance` excludes `.ont.yml` outright ([`history.rs:41-62`](../../yidam/cli/src/cmd/lint/history.rs#L41-L62)) |
| declared properties, types, targets, `edge_policy` | `blob_expectation` deserializes **one** field from a class blob — `direction` — into a three-valued `Expectation` ([`history.rs:225-262`](../../yidam/cli/src/cmd/lint/history.rs#L225-L262)) |
| relationship names on edges | `targets_of` drops them: `.filter_map(\|l\| l.target.as_ref())` ([`history.rs:82`](../../yidam/cli/src/cmd/lint/history.rs#L82)) |
| a revision to stop at | `change_stream` runs `git log --reverse … -- .yidam/corpus` with no revision argument and no parameter to supply one ([`history.rs:95-107`](../../yidam/cli/src/cmd/lint/history.rs#L95-L107)) — genesis to HEAD, always |

`replay` is the right *shape* and the wrong function. `--at` needs its own reconstruction:
read the tree at a rev and build the same structure `graph_data` builds, from blobs. The
property worth preserving is the one #262 actually names — **never touch the working tree** —
and that survives; what does not survive is the estimate.

Two rules for that reconstruction:

- It must resolve edges with the same rule the present-tense walk uses, or the historical graph
  and the current one will disagree about what points at what. `targets_of`'s own docstring
  already states this obligation for `replay`, and it transfers.
- The query typechecks against **that commit's** ontology, because that is the schema the data
  obeys. Both ontologies are in hand, so where the same query would typecheck differently
  against HEAD, the report carries an `info` diagnostic naming the step and both verdicts —
  rather than silently picking either.

> **Landed in #262**, as [`cmd/query/at.rs`](../../yidam/cli/src/cmd/query/at.rs): one
> `git ls-tree -r` and one `git cat-file --batch` — the two-subprocess shape `replay` uses,
> costing the size of the corpus rather than the length of history. Three things the estimate
> did not name, found in the building:
>
> - **`--select body` was reading the working tree.** Node paths are keys and are never
>   opened, except that one projection read `node.path` from disk — so a query at a past
>   commit answered with today's prose under a report saying which commit it was about. Fixed
>   by carrying the text on the node, which `load_nodes` already had in hand and dropped.
> - **A similarity anchor cannot be evaluated as of another commit.** The index is built from
>   one commit's text. Refused with `anchor-at-revision`; degrading to keyword search would be
>   a different retrieval than the same query gets at HEAD, which makes a series where the
>   answer changed because the *arm* changed.
> - **`--between` needs its own report type.** A series has no single `matched` and no single
>   `cost`, and a report carrying both shapes leaves one half meaningless — which a consumer
>   reads as zero. It also does not gate: one rejected row in a range is the ordinary case for
>   a class the corpus grew into.

> **Corrected in #325**, from a post-merge review reproduced against a built binary. Two
> failures, each with several faces:
>
> - **The reconstruction was not the walk.** `--at HEAD` on a clean tree must be the identity
>   and was not. A revision reached the git argv unseparated, so `--at=--output=<path>`
>   truncated an arbitrary file — a read-only command writing to the working tree, which is
>   the one property this section says survives. `ls-tree` was parsed without `-z`, so a node
>   whose path git quotes left the corpus silently; filtered on object *type*, so a symlink's
>   target became a phantom node; and `is_instance` required depth exactly 2 where the live
>   walk accepts 2 or more, so a node one directory further in existed at HEAD and at no
>   revision. `git log` was neither topologically ordered — a branchy history produced changes
>   that never happened — nor `--no-show-signature`, so `log.showSignature=true` made a
>   revision out of the word `Good`. An unreadable blob became an empty node rather than an
>   error. And the divergence note compared against `Graph::load` — the *working tree* — while
>   saying "HEAD", so an untracked `.ont.yml` moved a claim about a commit.
> - **Rejections escaped the envelope.** `rejected_report` hardcoded `at: null`, so a JSON
>   consumer read `anchor-at-revision` — a code only `--at` can produce — beside the key that
>   means *the working tree*, and in text mode a refusal about a tag was byte-identical to a
>   refusal about now. A revision that is not one returned `Err`, printing `Error: …` with an
>   empty stdout: no envelope, against the rule stated at the top of this section. `--between`
>   returned `Ok(())` unconditionally, so a syntax error rendered as an empty range at exit 0
>   — the query-text rejections are now taken once, before any tree is read, and gate. The
>   series marker compared the rendered *count*, so the one commit that swapped a node for
>   another was unmarked, and the per-row diagnostics were computed and never printed.
>
> The comparison also gained the case it was quietest on. `check` accepts every class name
> against a corpus with no `.ont.yml`, so at every commit before the ontology existed both
> verdicts came back `Ok` with no diagnostics — and a zero-result answer read as *nothing
> matched* where the truth is *nothing was checked*. `Checked` now carries `unschematised`,
> and `divergence` reads it and the narrowed class set, not only the diagnostic codes.

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
   same invariant, which is the drift RFC-0001 exists to stop. (`!=` inside a filter is a
   predicate on one property's value, not negation over a path.)
5. **No transitive closure** (`-refines*->`) in the first cut. A recursive relationship is
   written out (`concept -refines-> concept -refines-> concept`), which is honest about its
   bound. See the open question — #264's measurement showed the two arms converging by depth 3
   on a small corpus, and an unbounded closure is exactly where the distinction stops existing.
6. **No writes**, ever. Read-only.
7. **No dependency corpora**, including through the anchor. Traversing across a package
   boundary is E3 (#251). The report carries `"scope": "local"` and, per the anchor rule above,
   that label is made true rather than asserted.
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
- **`--at` needs a git history reconstruction** which, per the section above, does not fully
  exist even in the CLI. Shipping one to three languages to support one flag is a large surface
  for a small gain.
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
  ([`report.rs:32`](../../yidam/cli/src/report.rs#L32)). `diagnostics`, `cost`, `anchor`,
  `scope` and `rejected` are new *payload* fields on a new report, which is additive — consumers
  must ignore what they do not know.
- **One refactor asked for:** lifting `Retrieval::degraded_reason` out of `cmd/serve` so `query`
  and `serve` share one set of reason strings. Internal; no surface changes.
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
- **Reject every undeclared relationship, or every unauthored one.** The first is what the epic
  decision says literally and what E1 measured and rejected (210 errors on a compliant corpus).
  The second was in this RFC's first draft and is withdrawn above, with the four reasons.
- **A character-level grammar with backtracking, instead of the whitespace-delimited hop
  token.** Rejected: it makes `-` mean two things in one production, and the disambiguation
  would have to be either arbitrary (longest relationship wins) or ontology-directed (lex
  against the declared names), and the second makes lexing depend on the corpus — so the same
  string parses differently in two repositories.
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
- **The near-miss threshold.** The "within one edit of a declared name" diagnostic needs a
  concrete rule (edit distance 1? a prefix/suffix match?). Lean: edit distance 1 on the name,
  because the case it exists for — `sourced-from` against `sources-from` — is exactly that, and
  a looser rule turns a useful note into noise.
- **Client-side validation at parity.** The trigger would be the editor wanting to underline a
  bad query in an unsaved buffer, which is exactly what `universal.rs` already supports for
  classes. The minimum move is publishing `edge_policy` on `x-yidam-edges` in
  `compile_class_schema` (a parity bump), not moving the executor. Lean: not yet; revisit with
  RFC-0016's editor work.
