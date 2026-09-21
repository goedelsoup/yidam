# RFC-0035 — What a class is meant to span, and what a hole in it is worth (`coverage:`)

- **Status:** Draft
- **Track:** I30
- **Relates to:**
  - RFC-0028 (§5 reserved `kind: coverage` against this and scheduled nothing; this RFC argues the class-contract half and still schedules nothing)
  - RFC-0024 (whose settlement — the repository's own rule decides, and an override is visible as one — is what lets a hole be a finding here and an error somewhere else)
  - RFC-0018 (whose typed traversal is what a declared extent becomes answerable by, and the reason this RFC adds no second query)
  - RFC-0020 (whose licence to draft `open:` and never `establish:` is the only shape in which a hole could ever become a question)
- **Versioning layers touched:** template (one new optional key in `<class>.ont.yml`, published by `yidam schema`) / tooling (two `yidam lint` checks). **No SDK change and no on-disk change to any node:** the key is additive and optional, and no released parser reads class bodies strictly.
- **Downstream reference case:** the series-completing corpus measured for #572 (A0) — 777 instance nodes in 84 commits, 583 of them in two classes (`appropriation` 322, `expenditure` 261), one commit landing 315 at once.

## Summary

A class declares what its instances **are** and cannot declare what the set of them is meant to
**span**. Every check `yidam lint` runs against `<class>.ont.yml` asks *is this instance
well-formed*; none asks *is the set complete*. This RFC adds one optional key — `coverage:`,
naming a property and a closed domain — and two lint checks over it, so that a corpus whose
classes are enumerable over a known domain can say so and be told where the holes are. It
changes no default, floods nothing, gates on nothing unless a class asks, and computes no rate.

It also argues that the question the filing issue asks is not the question its own sketch
answers, and resolves the two apart.

**This RFC schedules nothing.** RFC-0028 §5 put #578 on the record as unscheduled behind two
conditions, and neither is claimed here. What a Draft RFC changes is that the design stops being
re-derived from scratch each time one of those conditions is examined.

## Problem

### Five checks, one question, and it is not this one

`<class>.ont.yml` carries `properties:` and `edges:`, and `lint` holds every instance against
both:

| Check | `checks.rs` | Asks |
|---|---|---|
| `undeclared-property` | [`checks.rs:1490`](../../yidam/cli/src/cmd/lint/checks.rs) | a property the class never declared |
| `missing-property` | [`checks.rs:1663`](../../yidam/cli/src/cmd/lint/checks.rs) | a declared property the instance omits |
| `property-type` | [`checks.rs:1917`](../../yidam/cli/src/cmd/lint/checks.rs) | a value contradicting the declared `type` |
| `unlicensed-edge` | [`checks.rs:1983`](../../yidam/cli/src/cmd/lint/checks.rs) | a relationship the class does not declare |
| `edge-target-class` | [`checks.rs:2049`](../../yidam/cli/src/cmd/lint/checks.rs) | an edge resolving to a node of the wrong class |

Every one of them takes an instance and asks whether that instance is well-formed. The set of
instances is never the subject. So a corpus whose classes are enumerable over a known domain —
fiscal years, counties, bill versions, releases, chapters — cannot state that domain, and a
question about the domain cannot be asked at all.

The adjacent reports do not cover it either, and each declines for its own reason.
`graph-check` answers whether the graph holds together. `catalog-audit` answers which sources
nothing cites. `lint` answers well-formedness. A hole is none of the three: nothing is
malformed, nothing is unreachable, nothing is uncited. **There is simply no node, and no
declaration anywhere that says there should have been one.**

### The only coverage prose in the model is about a search

The word occurs once, in `GRAPH.md`'s description of the `scope` verb
([`GRAPH.md:670-671`](../../yidam/prelude/GRAPH.md#L670-L671)):

> a negative result about coverage is the only durable record that the coverage was checked

That sentence is about the breadth of a **search**, is enforceable by nothing, and is not a
property of a class. It is worth quoting because it is the whole of the model's position, and
because it shows the model already believes the negative result is worth recording — it simply
has no place to record one that is about a class rather than about an act.

### What the reservation costs today

RFC-0028 §5 reserved `kind: coverage` as a named, unimplemented value rather than leaving a
blank to be invented twice, and the reservation is a live short-circuit
([`kuten.rs:764-772`](../../yidam/cli/src/kuten.rs#L764-L772)): a corpus that declares the
coverage pressure gets `unmeasurable` naming #578, and the band answers nothing. That is the
right behaviour for an unimplemented rule and it is a standing cost — the question-pressure slot
ships with half its vocabulary inert.

### The measurement

Across eighteen derived corpora, exactly one is a corpus **completing series**: 777 instance
nodes in 84 commits, 583 in two classes, with commit subjects that are real epistemic testimony
— *"a $3.5 billion figure I committed as verified was a sub-row, caught by overlap"*. Its nodes
are not thin records; the median is 28 lines and carries claim tags, edges and reasoning. It
survived every control applied in A0 and is the only corpus in the population whose practice
yidam cannot represent.

n=1 is the reason this is a Draft and not a schedule. It is also, on its own, enough to design
against: the instrument is blind to what that repository is doing, and being blind to it is not
a judgement the instrument ever made.

### The question the issue asks is not the question its sketch answers

This is the part worth getting right before any of the rest is built. #578 opens with:

> which fiscal years have no `appropriation` node?

and sketches:

```yaml
class: fiscal-period
coverage:
  key: year
  domain: 2010..2024
```

These are two different statements and the issue slides between them.

- **(a) A class's own instances span a domain.** `fiscal-period` should have one instance per
  year from 2010 to 2024. This is what the sketch declares.
- **(b) Every member of a domain is covered by some *other* class's instance.** Every fiscal
  year has an `appropriation`. This is what the headline question asks.

A declaration of (a) on `fiscal-period` does not answer (b), and a declaration that tried to
answer (b) would have to name a second class and a relationship — at which point it is no longer
a statement about what a class's instances are, it is a statement about a join, and it belongs
on the edge rather than on the class.

**The proposal below declares (a) only, and argues that (b) then falls out of machinery that
already exists.** Once `fiscal-period` declares its extent, every year in the domain is an
instance node; "which fiscal years have no appropriation" becomes "which `fiscal-period` nodes
have no inbound `appropriated-in` edge", which is `orphan-in`-shaped and is already reachable by
RFC-0018's typed traversal bounded by the ontology. The missing record stops being invisible and
becomes a present node with a missing edge, which the instrument can already see.

That is the central bet of this RFC and it has a real cost, stated plainly: it asks a corpus to
**author** the members of its domain rather than merely declare them. Fifteen `fiscal-period`
nodes have to exist. The argument that this is correct rather than a burden is that a fiscal year
the corpus reasons about is a thing the corpus knows about, and in this model a thing the corpus
knows about is a node. The alternative — a synthetic domain that exists only in the ontology — is
weighed below and rejected for a reason that is about the model rather than about effort.

## Proposal

### The declaration

One optional key on a class, a mapping, with three sub-keys of which two are required to say
anything at all:

```yaml
class: fiscal-period
description: A twelve-month accounting period the appropriations are drawn against.
properties:
  - name: year
    type: date
coverage:
  key: year
  domain: 2010..2024
  policy: open      # optional; `open` is what absence means
```

`key:` names a property the class declares. `domain:` states the members. `policy:` says what a
hole is worth.

### `domain:` admits only closed forms, and that is the design rather than a limitation

Two forms, and deliberately no third:

| Form | Written | Members |
|---|---|---|
| Explicit list | `domain: [draft, engrossed, enrolled, chaptered]` | exactly those |
| Inclusive integer range | `domain: 2010..2024` | 2010 through 2024 |

There is **no pattern form, no open-ended range, and no regex.** #578's second scope decision is
the reason, and it is the one most likely to be eroded in review: *"a domain is not always
enumerable… the declaration must not tempt a corpus into inventing a false enumeration to satisfy
it."* Years and counties are enumerable; concepts are not. A grammar that admitted a loose form
would let a corpus write something that looks like a domain and is not one, and every hole it
then reported would be an artefact of the fiction.

So the rule is: **if the members cannot be written down, the class cannot declare coverage, and
that is the correct answer.** A class with a non-enumerable subject is not failing this check —
it is not in this check's population, exactly as a class with no `edges:` is not in
`unlicensed-edge`'s.

### Three policies, because `edge_policy` already made this distinction sayable

The precedent #578 names is the right one and it is worth being literal about why. `edges:`
could not previously say whether it was a bound or a description, and reading it as a bound put
210 errors on a corpus doing nothing wrong. A domain has the same ambiguity: it can be a target
a corpus is working toward, or a bound it asserts is already met.

| `policy:` | A hole is |
|---|---|
| `closed` | an **error**. The series is asserted complete, and the class asked for this gate. |
| `open` | a **warning**. The corpus is working through the domain; a hole is the work remaining. |
| `indicative` | **not reported**. The domain is recorded for what it says about the class, not to be checked against. |

Absence means `open`. **This is the one place the parallel with `edge_policy` deliberately
breaks, and the break is honest rather than sloppy:** `edge_policy`'s absent state is a third
thing because every class written before the field existed has an `edges:` list and no policy,
so silence there is genuinely *nobody said*. Here the whole `coverage:` block is new, so there is
no legacy class carrying a domain without a policy — a corpus that wrote a domain wrote it on
purpose, and the useful default for deliberate work in progress is a warning.

The `policy:` key sits inside the block rather than beside it as `coverage_policy:`, because
`coverage:` is already a mapping and can hold it; `edge_policy` is top-level only because
`edges:` is a bare list with nowhere to put it.

### One finding, and it is a `lint` check rather than a `graph-check` one

#578 sketches *"a `graph-check` finding for holes"*. That is the wrong home, for three reasons
that compound:

1. **`graph-check`'s findings are keyed to an instance.** `GraphCheckReport` carries
   `nodes_with_issues: Vec<NodeIssues>`, and a `NodeIssues` names a file. A hole has no file —
   the absence of one is the entire finding. It would have to go in the report's one class-level
   list, `classes_without_instances`
   ([`corpus.rs:131`](../../yidam/cli/src/cmd/corpus.rs#L131)), which is documented as *reported,
   never gated* and therefore could never express `policy: closed`.
2. **`graph-check` has no severity model.** It is a boolean gate plus that single ungated
   escape hatch. #578's third scope decision — *"a hole is a finding, not an error; severity
   belongs with the corpus, per RFC-0024"* — has nowhere to live there. `lint` has `Severity`,
   per-violation overrides, and `policy-override` reporting when a repository moves one.
3. **Every other class-contract check is a `lint` check.** Putting the sixth one somewhere else
   would mean a reader asking *what does my class file promise* has two commands to run.

So: **`coverage-hole`**, one violation per missing member, filed against the class file — the
same subject `class-asserts-purpose` already files against. Default `Severity::Warn`, escalating
to `Error` under `policy: closed`, suppressed entirely under `policy: indicative`.

It carries **no ageing and no `escalate_after`.** A hole is not a finding that rots while
somebody ignores it; it is a finding that is being filled. A corpus that has not yet reached 2012
is working, and a check that grew angrier about 2012 every month would be measuring the calendar
rather than the corpus.

### Silence stays silence, read one field at a time

The rule `GRAPH.md` states for `properties:` and `edges:`, and which `max_lines`
([`checks.rs:173`](../../yidam/cli/src/cmd/lint/checks.rs)) is the closest precedent for, holds
here without amendment:

- A class with no `coverage:` has said nothing about extent, and neither check runs.
- A `coverage:` with a `key:` and no `domain:` has **also** said nothing. It is a field somebody
  started and did not finish, and inventing a domain from it would be the flood #578's first
  scope decision warns about — landing hardest on the corpus with the least reason to trust its
  graph.
- Declaring `coverage.key: year` does **not** make `year` a required property. That is
  `required: true`'s statement, one field over, and reading one declaration as another is the
  over-read this model refuses everywhere else.

**No domain is ever derived from observed instances.** A class whose instances happen to run
2011–2019 has not thereby declared a 2011–2019 domain, and a check that inferred one would report
a corpus against a contract it never wrote, then congratulate it for meeting it.

### A declaration nobody's instances can satisfy reports once, not N times

If **no** instance of the class carries `key:`, the declaration is inert, and reporting every
member of the domain as a hole would bury the one fact that can be acted on. So the second check,
**`coverage-key-unread`** (`Severity::Info`, one violation on the class file), says the
declaration is reading a property nothing carries — and `coverage-hole` stays silent for that
class.

This is the same suppression `lint` and `graph-check` already apply to an unreadable file, for
the reason stated there: *"adding six findings derived from the emptiness buries the one that can
be acted on."*

**There is no third check for a `key:` naming an undeclared property**, and the omission is
deliberate: if instances carry it, `undeclared-property` already fires on each of them; if they
do not, `coverage-key-unread` fires once. A third check would report a third time about the same
two facts.

### How a value is matched to a member

A member is matched by the key value's written form, with one rule carried over from the existing
type check rather than invented: **a `date` property is read at the precision it is known to.**
`GRAPH.md` already accepts `YYYY`, `YYYY-MM` and `YYYY-MM-DD` and says why — the precision is the
fact, and demanding more does not make the record accurate, it makes it invented. So against an
integer-range domain a `date` value is matched on its year component, and `2010-03` and `2010`
both satisfy member 2010.

### No rate, and no score

`coverage-hole` reports the members that are missing. **Nothing anywhere computes or publishes a
coverage percentage**, and `yidam score` gains no criterion from this RFC.

This is a constraint, not an omission. A number that goes up is the most reliable way to produce
exactly the behaviour #578 warns about — a corpus inventing an enumeration it can satisfy. Naming
the missing members and declining to average them is the whole difference between a finding a
corpus can act on and a target it can game.

## What this does not touch

- **Cardinality.** #578's own title is the argument: *a class says what an instance is and never
  how many there should be*. Coverage asks whether the domain is spanned, not how many instances
  a member has — so **there is no duplicate check**, and there should not be. `appropriation` has
  many instances per fiscal year and that is what an appropriation is.
- **The join — question (b).** No declaration here says *every member must be reached by an edge
  from class X*. That is a statement about a relationship, it belongs on the edge, and it is the
  first thing a second RFC should take up if this one lands.
- **Un-reserving `kind: coverage`.** This RFC removes the blocker RFC-0028 §5 named; it does not
  build the pressure band. What a corpus with declared extent should be *pressed* to open is a
  kuten question and §5 owns it, including the choice of predicate — and A3's erratum on the
  epistemic half is a standing warning that the obvious extraction is often the wrong one.
- **`graph-check`.** Its report gains no field and its gate is unchanged.
- **The `scope` verb.** The commit vocabulary is untouched. The sentence quoted above stays what
  it is: prose about an act, which this neither enforces nor replaces.
- **Any existing class.** Every `.ont.yml` in every derived repository is exactly as checked
  after this as before it.

## Migration & compatibility

**Template**, for a new optional key and its schema entry; **tooling**, for two lint checks. No
SDK change: the key is additive, class bodies are parsed permissively, and no released parser
reads a class strictly enough to reject it.

`yidam schema` publishes the key and the three `policy:` values, so a typo is underlined in the
editor where it can be fixed as it is typed — which is why an unrecognized `policy:` value parses
as `open` rather than failing, on the same reasoning `EdgePolicy::parse` gives for its own
unknown values: a check that failed on vocabulary it had not heard of would make coining anything
impossible.

Adoption is writing the block. Declining to write it leaves a corpus exactly as checked as it was.

Registration is the usual surface for a lint check and should be done in one pass: the check
bodies, the roster assertions in `lint/mod.rs`, and the parity fixture at
`sdks/parity/fixtures/reports/basic/expected/lint.json`.

## Alternatives considered

**Declare coverage on the edge instead of the class.** This answers the headline question (b)
directly and is genuinely tempting. It is rejected *for now* rather than on the merits: it is a
strictly larger change, it needs the class to be able to state its own extent first — an edge
rule over a domain nobody enumerated has nothing to range over — and #578's whole argument for
class lineage is that extent is the same kind of statement as `edge_policy`. Build the smaller
one, and let the join be decided against a corpus that has actually declared a domain.

**A synthetic domain, whose members are not nodes.** The domain would exist only in the ontology
and `coverage-hole` would report against instances without requiring the members to be authored.
Rejected because it introduces a second kind of thing the corpus knows about — one that can be
referred to, is missing from every export and every traversal, and cannot carry a claim tag or a
source. RFC-0018's traversal, `export-rdf`, and the whole retrievability surface would each have
to decide what to do with it. The model has one answer for a thing the corpus knows about, and
this RFC declines to add a second.

**Put it in a kuten profile.** Rejected in the filing and the argument holds: two corpora with
identical ontologies could then disagree about whether `fiscal-period` covers 2010–2024, which is
not a thing they should be able to disagree about. A kuten declares what a *practice* aims at;
this declares what a *class* is meant to span, and it is true regardless of which practice the
repository runs.

**Infer the domain from observed instances and report the gaps.** Rejected above, and it is worth
recording that it is the cheapest option and the only one that needs no declaration at all. It
reports every corpus against a contract none of them wrote.

## Open questions

1. **Does `domain:` need a date-range form?** `2010-01..2012-12` is writable and an explicit list
   of 36 months is not pleasant. The case for holding to two forms is that every added form is a
   new way to write a fiction; the case against is that monthly and quarterly series are common
   and the list will be generated rather than typed.
2. **Should a hole be openable as a question?** RFC-0020 licenses `propose` to draft `open:` and
   never `establish:`, and *"here is a member of a declared domain with no instance"* is precisely
   a question the work already implies rather than a claim. This is the natural bridge to #573's
   coverage half, and nothing here decides it.
3. **Is `indicative` earning its place?** It exists by symmetry with `characteristic`, and unlike
   `characteristic` it has no measured corpus behind it. If nobody writes it, it is a value the
   schema has to keep explaining.
4. **The scheduling triggers are unchanged.** RFC-0028 §5 named two conditions — a second
   series-completing corpus at a comparable vintage, or the measured outlier's practice filing a
   concrete upstream need. This RFC claims neither has fired, and an argued design does not
   substitute for one. It is a Draft so that the next time one of them is examined, the question
   is whether to accept a design rather than whether to invent one.
