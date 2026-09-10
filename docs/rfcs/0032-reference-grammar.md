# RFC-0032 — One name for a thing, and one parser that reads it

- **Status:** Draft
- **Track:** I27
- **Relates to:**
  - RFC-0005 (which declared the `yidam://` scheme normative for MCP resources; §4.1 amends that section rather than contradicting it)
  - RFC-0019 (whose `cites:` is the only identifier in the system that can name a foreign node at a known revision, and which is a struct rather than a string)
  - RFC-0027 (whose §5 argued that a `url` a reader follows cannot be a `yidam://` URI, and whose `public_base` this generalises off one profile)
  - RFC-0008 (which measured that a claim has no identity a checker can read; §4.4 is that result written into the grammar as a refusal)
  - RFC-0031 (whose node-prose work owns the *scope* half of the evidence-tag request; this owns the provenance half)
- **Versioning layers touched:** SDK+parity (one parser, three languages) / tooling (every surface that builds or splits an id) / **MCP contract** (§4.1 adds a slot to the resource namespace — a minor bump, in all three copies). No on-disk format change: `links:` and `cites:` keep their shapes.
- **Amended 2026-09-09 (#777):** the precondition below is weaker than this RFC first claimed, and §4.1 and §4.6 move with it. Two of sixteen measured corpora do not satisfy it, so the parser reports conformance instead of assuming it.
- **Precondition, landed:** `name-not-a-slug` (#775). A grammar over names cannot be specified while a name may contain a space; that check makes the character set an invariant **of a corpus that passes lint** — which is not every corpus. See the amendment in §4.1.
- **Downstream reference case:** `goedelsoup/ohio-education-funding` (129 nodes, 168 crate references folded into evidence-tag details). The tag measurements below are over thirteen corpora, on maturity grounds; §4.1's conformance measurement is over all sixteen with tracked nodes, because that question is about adoption rather than practice

## Summary

A corpus node has **eleven** string forms across this repository. Two of them can say which
corpus a node came from, one can say which revision, and none can say both. The forms are not
alternatives a caller chooses between — they are what different surfaces invented independently
because there was no first form to reuse, and one of them,
[`find_node`](../../yidam/cli/src/cmd/serve/tools.rs#L199), already accepts three spellings that
no contract mentions. An ad-hoc resolver is what an absent grammar looks like from the inside.

The absence has a second face. A derived corpus folded 168 crate paths and 145 catalog and node
references into evidence-tag details, because the tag bracket was the only place a reference
could be written at all, and then grew a regex to read them back out. Measured across thirteen
derived corpora, **~89% of those details name a source rather than qualifying a claim**. That is
not a tag problem wearing a reference costume; it is this problem, reported from a corpus.

So: one grammar, rendered two ways. A `yidam://` **identifier** that works inside a `.yiz`
tarball with no host, and an `https://` **locator** for anything a stranger has to follow. One
parser on the parity surface, and it becomes the only place an identifier is built or split.

## Problem

### 1 — The scheme spends its authority on the collection

`yidam://corpus/<class>/<name>` parses, under RFC 3986, as authority `corpus` and path
`/<class>/<name>`. The naming authority slot — the one component whose job is to say *who names
this* — holds a collection kind. `yidam://graph/summary`, `yidam://skills/<name>` and
`yidam://decisions/<name>` do the same with three more kinds. There is nowhere left to put a
corpus.

That is not a latent tidiness issue. [`resources.rs:56`](../../yidam/cli/src/cmd/serve/resources.rs#L56)
interpolates `node.id` into the URI, and the loop it sits in reads `state.nodes` only —
[`dep_nodes`](../../yidam/cli/src/cmd/serve/mod.rs#L55) is a separate field by deliberate design.
So a dependency node has **no resource URI at all**, while
[`find_any_node`](../../yidam/cli/src/cmd/serve/tools.rs#L222) reads one happily by its qualified
id. One server answers a question through its tool surface that its resource surface cannot
address, and RFC-0005 declares the scheme normative without mentioning dependencies.

### 2 — The RDF export mints subjects in a scheme nothing can dereference

[`instance_iri`](../../yidam/cli/src/cmd/export_rdf.rs#L135) types every instance at
`yidam://corpus/<class>/<name>` and [`dataset_iri`](../../yidam/cli/src/cmd/export_rdf.rs#L301)
names the dataset with a bare authority. No RDF consumer can follow either, and two corpora
holding `concept/foo` mint the same subject. The export already knows the distinction it is
failing to apply: [`export_rdf.rs:317`](../../yidam/cli/src/cmd/export_rdf.rs#L317) tests a
foreign alignment IRI for a scheme it can dereference and demotes it to a literal when it fails.
It applies that test to other people's IRIs and not to its own.

Separately, [`YIDAM_NS`](../../yidam/cli/src/cmd/export_rdf.rs#L23) was
`https://yidam.dev/ontology#`, and that domain does not resolve — `curl` answers 000, not 404. It
shipped in every corpus's exported triples, which is worse than a dead link: an unowned namespace
already published in other people's data is registrable by a stranger, who would then be
authoritative for terms this project minted.

> **Resolved 2026-09-09 (P3).** Both halves. The namespace moved to
> `https://goedelsoup.github.io/yidam/ontology#`, the one origin with a demonstrated 200 — decision
> B's owned origin behind one constant. Subjects became `urn:yidam:<corpus>/<kind>/<path>`, derived
> by re-scheming [`render_reference`](../../yidam/prelude/sdks/rust/src/uri.rs#L283)'s output rather
> than assembled a second time, and the dataset became `urn:yidam:<corpus>` — an authority with no
> path, because a corpus is what references are resolved *against* and not a thing inside one.
>
> **`<corpus>` here is the genesis commit hash, not the declared package name**, and the first
> implementation had it the other way round. Zero of the sixteen corpora with tracked nodes declare
> a package name — none has a `.yidam/tonpa.toml` at all — so a subject built on it fell back to a
> constant for every corpus that exists, leaving §2's collision exactly where it was while
> appearing to fix it. The genesis hash is also the only corpus identity that cannot be *renamed*,
> which a published subject needs and a nickname cannot promise. §4.5 is unchanged: the declared
> name is right for an identifier a person reads or types, and these are different jobs — the same
> split §4.2 makes between an identifier and a locator, one level down.
>
> Running the exporter over two real corpora found two more things no fixture had:
>
> - **55 subjects were malformed**, 37 of 185 in one corpus and 18 of 810 in the other, all one
>   shape: `../../catalog/<file>.md` link targets minted as
>   `urn:yidam:<hash>/node/../../catalog/<file>.md` — relative segments inside an identifier, typed
>   as a node, for a thing that is a catalog entry. The `catalog` kind in §4.1 is what they are, so
>   the fix was to use the grammar rather than to escape around it. Nothing else escapes the corpus
>   directory in either corpus, so this is complete and not a first case. A prefix-checking guard
>   cannot see this; the guard now parses each subject back and applies `reference_conforms`.
> - **An unidentifiable corpus is refused**, not given a shared name. Any constant is the same
>   string for every corpus that reaches that branch, so two of them merged into one store would
>   conflate their nodes. The branch is reachable — a worktree copied out of its parent, a shallow
>   clone without the root commit, a `.yiz` extracted to a plain directory — and **the bundle
>   manifest cannot supply it**, because it records `genesis` as an ISO *date*. The addressing
>   plan's decision C assumed a digest was already there; see #781.
>
> Two things this deliberately did **not** do. §4.2's locator branch is not implemented: it needs a
> corpus to declare a base, `public_base` is RFC-0027's and unshipped, and a branch nothing can
> reach is the surface-with-no-consumer shape. And the namespace is owned but does not yet serve a
> document — the docs site is versioned, so a page describing these terms reaches `/yidam/main/…`
> on merge and this path at the next `cli/v*` tag. The guard asserts **ownership**, not resolution,
> because a check keyed on an artifact that does not exist yet goes red the day it lands.

### 3 — Only a struct can say *which corpus, at which revision*

[`qualified_id`](../../yidam/cli/src/model.rs#L380) renders `pkg::class/name` and is the only
string form carrying a corpus. [`ExternalCitation`](../../yidam/prelude/sdks/rust/src/corpus.rs#L77)
carries `package`, `node`, `commit` and `tag` — the only identifier in the system that can name a
foreign node at a known revision, and it is four fields rather than a string, so it cannot appear
in a resource URI, an RDF subject, a query result, or a rendered citation. Meanwhile
[`resolve_link_target`](../../yidam/cli/src/model.rs#L397) resolves an on-disk edge from a
relative path, which is a *fifth* convention, and unrelated to any of the above.

### 4 — Eleven forms, and the count is the argument

| Form | Built by | Corpus? | Revision? |
|---|---|---|---|
| `class/name` | [`model.rs:350`](../../yidam/cli/src/model.rs#L350) | — | — |
| `pkg::class/name` | [`qualified_id`](../../yidam/cli/src/model.rs#L380) | yes | — |
| `.yidam/corpus/class/name.yml` | tolerated by [`find_node`](../../yidam/cli/src/cmd/serve/tools.rs#L199) | — | — |
| `../other-class/thing.yml` | [`resolve_link_target`](../../yidam/cli/src/model.rs#L397) | — | — |
| `yidam://corpus/class/name` | [`resources.rs:56`](../../yidam/cli/src/cmd/serve/resources.rs#L56) | — | — |
| the same string as an RDF subject | [`instance_iri`](../../yidam/cli/src/cmd/export_rdf.rs#L135) | — | — |
| `file:///…/class/name.yml` | [`path_to_uri`](../../yidam/cli/src/cmd/lsp.rs#L106) | n/a | — |
| `/node/class/name` | [`graph.ts:145`](../../yidam/editors/web/src/lib/graph.ts#L145) | — | — |
| a GraphML `node id` | `export_graphml.rs` | — | — |
| `{package, node, commit, tag}` | [`ExternalCitation`](../../yidam/prelude/sdks/rust/src/corpus.rs#L77) | yes | yes |
| `<public-base>/class/name` | RFC-0027 §5, unshipped | yes | — |

Nine of the eleven can say neither. Three surfaces also disagreed about *encoding* the same id
until #775 wrote the character set down, which is the precondition this RFC needed — though what
that check makes invariant is narrower than this RFC first assumed, and §4.1's amendment measures
by how much.

## Proposal

### 4.1 — The grammar

```
identifier   yidam://<corpus>/<kind>/<path>[@<rev>][#<property-path>]
locator      https://<base>/<kind>/<path>
relative     <kind>/<path>  |  <path>          resolved against the containing corpus
kind         node | crate | catalog | skill | decision
```

The corpus moves into the authority and the kind into the path, which is the minimal change that
creates the slot §1 lacks. `<path>` is `<class>/<name>` for `node` and a single segment for the
others. Every segment in a **conforming** corpus is a slug — the rule
[`name_not_a_slug`](../../yidam/cli/src/cmd/lint/checks.rs#L1184) reports against, through its
predicate [`is_slug`](../../yidam/prelude/sdks/rust/src/uri.rs#L122) — so **no percent-encoding is
required anywhere in this grammar**, which is why there is one string form and not one per
encoder.

> **Amended 2026-09-09 (#777): a conforming corpus, not every corpus.** This RFC was written on
> the claim that #775 made the character set an invariant. Measured with the shipped binary
> against the sixteen corpora that have tracked nodes, fourteen conform and **two do not**:
> `yidam-proto-001d` reports 6 findings and `yidam-proto-002c` 16, with `yidam lint` exiting 1 in
> both. Both are the youngest corpora measured, and both took class names from their domain's own
> vocabulary in its own case — `NonConformance`, `DischargePoint`. Kebab-case is what corpora
> converge on, not what they start with, and `yidam lint --bless` is the supported way to adopt a
> check over names you already wrote. So a non-conforming corpus is a state the system offers, not
> merely one it fails to prevent.
>
> The grammar is unchanged: a conforming id still needs no escaping, which is the property §4.6's
> single parser rests on. What changes is that the parser may not *assume* it. §4.6 carries the
> consequence.

**Which relative form applies is decided by arity, not by preference.** `<kind>/<path>` and a
bare `<path>` share a shape, and a corpus may legitimately declare a class named `node` — nothing
reserves the five words. So `<path>`'s segment count decides: reading the leading word as a kind
must leave exactly that kind's arity (two segments for `node`, one for the others), or the word was
a class name and the whole string is the path. `node/concept/foo` is the node `concept/foo`;
`node/foo` is the node `foo` in a class called `node`. Where both readings are valid — `skill/foo`,
in a corpus with a class named `skill` — the kind wins, because the grammar puts a kind in that
position, and §4.6's renderer emits the explicit `node/skill/foo` for the other reading so that
parse and render stay inverses. Nothing is reserved and nothing collides.

RFC-0005's normative scheme section is **amended**: three of the five URIs it froze map into the
new shape as aliases, so a client written against 0.18.0 keeps working. **The other two do not map
at all (#779):** `yidam://corpus/<class>` names a collection and `yidam://graph/summary` a computed
report, and the grammar addresses things with identity. Both parse as *not a reference*, which is
correct for a reference parser and leaves a decision for P4 — whether the resource surface is a
superset of this grammar or a different vocabulary that overlaps it. That is where the contract bump
is spent and where the question belongs.

### 4.2 — Two renderings, because they have two jobs

RFC-0027 §5 established that a field whose purpose is *a reader can follow this* cannot hold a
`yidam://` URI. That argument does not generalise into *identity must be a URL* — it generalises
into *stop asking one string to do both*. An identifier must survive a `.yiz` tarball, a private
corpus and an offline clone, none of which have a host. A locator must be followable. So:

- The identifier is authoritative for equality, traversal, tool arguments and the lock file.
- The locator is derived, per-corpus, from a declared base, and is what appears anywhere a
  stranger reads the output — a rendered citation, an RDF subject, a search result's `url`.
- An RDF subject uses the locator where a base is declared and a `urn:yidam:` form where none is.
  It never uses a `yidam://` IRI, which is §2's defect.

### 4.3 — A revision is a pin, not identity

`x` and `x@abc` denote the same node in two states, and the resolver reports which it returned.
[`ExternalCitation`](../../yidam/prelude/sdks/rust/src/corpus.rs#L77) already settled this by
holding `node` and `commit` as two fields; following the split means no existing identifier
changes meaning, where folding the revision into identity would silently make every id in every
corpus name something else.

### 4.4 — A fragment addresses a declared property, and never a claim

RFC-0008 measured this rather than assumed it: across 29 resolutions and 70 tips, 22 claims
entered corpora in resolution commits and **0 matched byte-identically** at any participating tip,
because a sentence search that ends at `\n` extracts one claim as two different strings in two
hard-wrapped nodes. Identity by surface form is identity by line wrapping.

So the fragment names something that *has* a name — a declared property path — and the grammar
**refuses** a fragment naming a claim. The consequence for the request that prompted this RFC:
*which nodes state this figure?* is answerable the moment the figure is the addressed thing, and
unanswerable as long as the claim wrapped around it is. Address the figure.

### 4.5 — The corpus name, and a collision nobody can currently see

The authority is the declared package name from
[`PackageMeta`](../../yidam/cli/src/deps.rs#L34) — the value every surface already prints. It is
a nickname chosen by whichever repository declared the dependency, so it is unique within one
`.yidam/tonpa/` and not globally. Rather than invent a global identifier and make every printed
id unreadable, this RFC makes a collision **detectable**: a consumer holding two corpora that
claim one name says so, using the genesis digest each manifest already carries. This is #610's
question one layer down and the answers must not contradict.

### 4.6 — One parser

`yidam_core::uri` parses and renders the grammar and becomes the only place an identifier is
built or split. It retires [`find_node`](../../yidam/cli/src/cmd/serve/tools.rs#L199)'s three
tolerated spellings, [`qualified_id`](../../yidam/cli/src/model.rs#L380),
[`instance_iri`](../../yidam/cli/src/cmd/export_rdf.rs#L135),
[`resources.rs:56`](../../yidam/cli/src/cmd/serve/resources.rs#L56)'s prefix chain and
[`graph.ts:145`](../../yidam/editors/web/src/lib/graph.ts#L145)'s route builder. It lands on the
parity surface, so Rust, TypeScript and Python must agree — three implementations and one shared
case set, not one implementation.

**It is total, and it reports conformance rather than requiring it (amended, #777).** §4.1's
measurement found two corpora whose segments are not slugs, reachable through the `--bless` the
check itself offers. Three ways to handle that, and only one of them ends the problem this RFC is
about:

- *Refuse a non-slug segment.* Then 6 of `yidam-proto-002c`'s 10 nodes cannot be named at all, and
  every consumer grows a fallback path for the corpus in front of it — the per-surface
  improvisation §1 diagnoses, reintroduced by the fix for it.
- *Escape it.* One string form becomes one per encoder, which is the state §4.1's no-escaping
  property exists to leave.
- *Parse structurally, and answer whether the result conforms.* The parser splits on `/`, `@` and
  `#` for any input, and a separate predicate says whether every segment is a slug. A caller that
  must emit a URI can then act on the answer.

The third, because the repository already does exactly this one layer out:
[`export_rdf.rs:317`](../../yidam/cli/src/cmd/export_rdf.rs#L317) tests a foreign alignment IRI for
a scheme it can dereference and demotes it to a literal when it fails. §2's complaint is that it
applies that test to other people's IRIs and not to its own. Reporting conformance on our own
identifiers is that same test, turned inward — which makes this amendment continuous with the
problem statement rather than a concession against it.

Enforcement stays where it already is: `name-not-a-slug` is the check, at `Error`, and a corpus
that blessed its findings has recorded a state rather than acquired a licence. The parser's job is
to be honest about what it was handed.

## What this does not touch

- **The vault and catalog schemes.** `s3://`, `file://`, and the catalog's four location kinds
  are locators for bytes. They work, they are argued, and they are not identity.
- **The on-disk format.** `links:` keeps relative paths and `cites:` keeps its four fields. §4.3
  adds a *rendering* of `cites:`, not a replacement for it.
- **The scope half of the evidence-tag request.** What is asserted, and one sentence carrying two
  standings, are RFC-0031's and #710's. No identifier helps there, and the measurement that
  settled the split put ~11% of details on that side.
- **Making more findings dated.** #774 is a separate gap.
- **The `[verified · checked]` axis.** Parked deliberately; it bumps the frozen `claim_tags`
  vocabulary and should be costed on its own evidence.

## Migration & compatibility

Nothing in a corpus changes. The identifier is derived from the same path segments the current
forms are derived from, so every existing node has the same identity before and after; what
changes is that the string naming it is produced in one place.

The MCP contract takes a minor bump in all three copies it is written down in
(`mcp/tools.json`, `mcp/VERSION`, and the README example). Because RFC-0005's five URIs stay
valid as aliases, a client that never adopts the new form keeps working, and one that does gains
the ability to address a dependency node — which no client can do today.

Sequencing: this RFC's §4.1–4.3 and §4.6 are P1 of the addressing plan and need no other work.
§4.2's locator half depends on a corpus declaring a base, which is RFC-0027's `public_base`
generalised; §2's namespace fix is independent and can land beside it.

## Alternatives considered

- **Restructure `yidam://` and stop there.** The smallest diff, and it leaves §2 unfixed: RDF
  subjects stay in an unregistered scheme and corpora still collide. Rejected because the RDF
  export is a shipped surface, not a future one.
- **Adopt `urn:yidam:` as the primary form.** Formally correct — a node id is a name, not a
  location — and legitimate as an RDF subject with no domain. Rejected as the *primary* because
  it renames a scheme frozen in a contract with three copies, is unfamiliar to every client
  author, and still needs a locator for followable URLs. It survives in §4.2 as the RDF fallback,
  which is the one job a URN does better than either alternative.
- **HTTPS-first: identity is a URL.** Dereferenceable and RDF-clean by construction. Rejected
  because it makes identity contingent on hosting: a corpus in a tarball, or private, could not
  name its own nodes. RFC-0027 made `--public-base` a hard refusal for this reason.
- **Fold the revision into identity.** Cleaner to reason about formally, and it silently changes
  what every id in every existing corpus refers to, with no migration that can fix prose already
  written.
- **Let the fragment name a claim.** What the downstream request asked for, and what RFC-0008
  measured to be impossible by surface form. Declined in §4.4 with the measurement, so it is not
  re-proposed.

## Open questions

- **Does the alias mapping need to be normative, or is it a compatibility note?** If a
  tools-only server must reproduce it exactly, it belongs in the contract; if it is only the Rust
  server's transitional courtesy, it does not. Sharpened by #779: the question only arises for the
  three URIs that *have* a mapping, because the other two turn out to address things the grammar
  does not name.
- **What does the locator do for a dependency node whose corpus declares no base?** RFC-0027 §5
  chose to omit and count the omission. That is right for a search result and possibly wrong for
  an RDF subject, where omitting the node loses an edge rather than a row.
- **Is `crate` the right kind name?** It names a Rust crate today and the compute layer is
  `crates/` and `packages/`. A kind that means *the code beside this corpus* may want a name that
  is not one language's.
