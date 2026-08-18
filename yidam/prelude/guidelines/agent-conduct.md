# Agent Conduct

Guidelines for agents operating in yidam-derived repositories.

## Commit deliberately

Every commit is a permanent node in the knowledge graph. Before committing:
- Is this a complete, coherent knowledge event?
- Is the commit message legible as a graph event description?
- Are new nodes linked to existing ones?

Do not commit partial work or exploratory scratch to the settled baseline. Use branches for
open-ended exploration; commit to the baseline when knowledge is settled.

## Link generously

New nodes should reference existing related nodes. Orphan nodes — files with no incoming
or outgoing edges — weaken the graph. When adding a file, ask: what does this connect to?

## Stay within scope

The corpus grows through sustained, directed inquiry. Do not add nodes speculatively or
outside the domain of the current repository. Breadth should be driven by need, not by
completeness anxiety.

## Make synthesis explicit

When two ideas relate, say so in the files — and in the commit. A synthesis commit that
adds edges between existing nodes is a first-class knowledge contribution, not housekeeping.

## Preserve provenance

Do not delete or rewrite committed nodes without a record of why. If a node is superseded,
mark it as such and link to its successor. The graph's history is part of its value.

## Mark claim confidence

Corpus nodes often contain claims at different levels of certainty. Tag them inline so
readers and agents can assess the node's reliability without reading sources:

- `[verified]` — supported by a committed primary source linked from this node or its catalog entry
- `[inference]` — a reasonable conclusion drawn from verified facts; not directly witnessed
- `[open]` — a live question; the answer is unknown, contested, or under investigation

**Rules:**
- Untagged claims are only implicitly verified if the node is a direct transcription of a
  primary source. In all other cases, tag every non-obvious claim.
- `[inference]` is not a weakness — it is honest. Untagged inference is the problem.
- `[open]` claims do not need to be resolved before committing. An open question is a valid
  and permanent knowledge contribution.
- A synthesis node will typically contain all three: verified facts it draws on, inferences
  it makes, and open questions it generates. This is expected and good.

### `[verified]` is a claim about provenance, not about confidence

The distinction decides the hard cases. A figure can be almost certainly correct and still
not be `[verified]`, because the tag says *a committed source supports this*, not *I am
sure*.

The case that forces it: the source of record is unreachable — the filing authority blocks
automated clients, the publisher is offline — and an aggregator or republisher carries the
same figures. Those figures may be retrieved, computed on, and published. They support
`[inference]`, and they never support `[verified]`, however good they are. A republished
bulk file is closer to the source than a derived presentation is, and it is still not the
source.

Where a connector reaches such a source, make the distinction **unrepresentable rather than
advisory**. A provenance type whose aggregator variants cannot produce the stronger tag, and
a test asserting it, is worth more than a paragraph telling agents to be careful:

```rust
impl Provenance {
    /// False for every aggregator kind. `[verified]` is about provenance, and an
    /// aggregator is not the source of record however accurate its figures are.
    pub fn supports_verified(&self) -> bool { ... }
}
```

Name the substitution wherever the figures appear, not only at the connector.

### A claim tag cannot reach a class definition

Every rule above runs on tags, and a tag attaches to a claim somebody makes on a node. A
class definition in `<class>.ont.yml` is not that. It is the meaning every instance takes
on by being filed under the class — asserted identically, silently, untagged, and for each.

So a class whose definition *states a proposition* puts that proposition beyond the reach of
the entire apparatus. The worked case: a class defined at genesis as "a procedural mechanism
**deployed to obtain** an outcome the ordinary path would not yield," in a corpus whose
first evidentiary rule was *attribute intent, never assert it*. Every instance asserted a
purpose by existing. It survived five resolutions and three arguments about its instances,
because every safeguard was pointed at instances.

**Read `.ont.yml` files against your evidentiary rules directly, and on a schedule** — no
instance-level check will do it for you. A class definition describes a *kind*; the moment
it starts describing a *reason*, it is making a claim no reader will see it make.
`yidam lint` reports the shape of this as `class-asserts-purpose`, but a lint over wording
is a prompt to look, not a proof of absence.

## Prefer a base rate to a refusal

Where a documentary sequence invites a causal reading the record cannot support, saying *do
not infer that* is weaker than saying *here is how ordinary that outcome is*. A refusal asks
a reader not to draw the inference. A base rate removes the reason to. Where the corpus can
compute one it should — in the same passage as the sequence, not below it.

### But a reference class defined by the outcome it is meant to place is not a base rate

This corollary is the one that costs something, and it was learned by audit: of six nodes
written under the rule above, **five had no computable denominator, and four of those failed
the same way.**

The failure is seductive because it feels like diligence. You gather the cases resembling
the one at hand until a fraction appears — but the filters get chosen *after* the outcome is
known, so the class ends up holding only cases that could have come out the way this one
did. Two shapes to recognize:

- **The sequence restated as a fraction.** "Six of eight plans were four-year plans," where
  all eight are already on the thread under discussion.
- **One act counted twice.** "Two of two officers removed themselves," where being an
  officer *is* the precondition for the act.

A denominator earns the name only if it was drawn without reference to how this case turned
out, and if the population could have contained cases that came out otherwise. Check the
shape too: a count of who *is* something does not place a claim about who *became* it.

**Where no base rate is computable, say so and say why.** That sentence is worth as much as
a rate and is not a failure to have looked. A fabricated denominator is worse than none,
because it launders the sequence into arithmetic.

## When claims leave the repository

A repository whose output is internal is checked by the gate. A repository that publishes —
a site, a report, a brief, anything read by someone who will not read the corpus — needs the
derivation checked too, and three rules generalize from the case that built them.

**A derived assertion travels only as far as the weakest claim beneath it.** Its tier is the
**minimum tag across the whole supporting chain**, computed rather than declared. `[verified]`
may reach public material; `[inference]` reaches attributed memos and backgrounders;
`[open]` does not leave the repository. Declared tiers drift the moment a supporting node is
revised — computing it means a downgrade upstream propagates on the next build.

**Cite a span, not a node.** An external assertion names a **verbatim span** of the corpus
node it rests on, and the gate asserts that span appears there character-for-character. This
does not verify the inference; nothing can. It forces the actual sentence to sit beside the
assertion, where the gap between them is visible to a reader.

**A refusal in the cited block fails the build.** Where a corpus node carries a refusal
beside the claim — a sentence of the form *this corpus does not infer X from this* — an
assertion citing across it is refused, and the author must answer it rather than route
around it. This one is invisible to the tag apparatus: the case that produced it was a node
stating a fact at `[verified]` and refusing the inference from it one sentence later. A
tag-only gate passes that. **Those refusal sentences are among the most valuable text a
corpus holds, and until something reads them, nothing does.**

## The safeguards were built against carelessness, not against interest

Worth stating plainly, because it is the finding that generalizes furthest and the one
easiest to feel exempt from.

Every mechanism above — claim tags, catalog anchors, the graph gate, the lint — catches an
agent that was sloppy. None of them catches an agent that wants a particular answer.
Motivated reasoning does not produce untagged inference. It produces a claim that is true,
correctly sourced, correctly tagged, and standing in front of the twenty that went
unmentioned. Every check passes. The corpus is wrong anyway.

The repository that found this had caught eight errors from the inside and **not one of them
was a selection error** — all eight were caught by an elector who wanted nothing. Selection
is invisible to a gate because a gate reads what is there.

The partial defense is to write down the search rather than only the finding: **what was
examined and not used**, kept beside the claim it supports. It is a weak instrument and it
is the only auditable trace of selection that exists. If you are working somewhere with an
interest in the answer, behave accordingly, and write the ledger.
