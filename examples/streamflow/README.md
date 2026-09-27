# streamflow

*A worked yidam corpus: streamflow on regulated rivers.*

Eight instances across three classes, one catalog source, two decision records, one skill,
and a benchmark goal set. Small enough to read in ten minutes and structured enough that the
ontology is doing real work.

## The domain, in one paragraph

A river reach below a dam does not behave like a river. Its discharge is set by an operating
rule, so statistics defined on unregulated catchments — daily means, annual minima,
base-flow indices — describe the operator rather than the catchment while still wearing the
units of a natural process. That gap is what this corpus is about, and it is why the corpus
has more `[open]` claims than settled ones.

## What is illustrative and what is real

The **conventions** are real: the USGS parameter code for discharge, the units, the 7Q10
definition, what base-flow separation is and why it is not unique.

The **stations and reaches are not**. There is no Canyon Outlet gage. They carry real
conventions so the shape of a real record is legible, and they reproduce no observation from
any real station. A corpus that invented plausible discharge values for a real gage would be
a fabricated record, and this one is meant to be copied.

## The shape of it

```text
.yidam/
  corpus/
    reach.ont.yml      concept.ont.yml    gage.ont.yml
    reach/             concept/           gage/
      tailwater          low-flow           canyon-outlet
      lower-canyon       hydropeaking       valley-bridge
                         base-flow-separation
                         instream-flow-right
  catalog/usgs-nwis.md
  decisions/three-classes.yml
  decisions/base-flow-index-carries-its-method.yml
  skills/read-a-regulated-record.md
  bench/goals.yml
  capabilities.toml
  capabilities/travel-tier.sh
  capabilities/travel-tier-typed.glu
  capabilities/disclosure-envelope.sh
```

## What each piece is here to demonstrate

**The three classes, and the fourth that was rejected.**
[`decisions/three-classes.yml`](.yidam/decisions/three-classes.yml) records the argument
against making an observation a corpus node. It is the most transferable thing in the
example: a corpus node is something a person authored and a sangha can be accountable for,
and admitting machine output at scale makes every corpus metric measure the connector.

**Claim tags at all three tiers.** Every node carries `claim_tag` as a *typed field*
declared on its class, not only as inline prose — the form `open-questions`, `status`,
`corpus-index` and the MCP server can all actually read. A corpus that tags only in prose
reports two open questions against its own count of twenty-six.

**A class property that had to be removed.**
[`decisions/base-flow-index-carries-its-method.yml`](.yidam/decisions/base-flow-index-carries-its-method.yml)
is the class-definition hazard in miniature. `base_flow_index` on the reach class would have
asserted, silently and for every instance, that the separation is a measurement — a claim
placed beyond the reach of the tag apparatus by not being a claim anyone makes out loud.

**A source, and what it does not answer.**
[`catalog/usgs-nwis.md`](.yidam/catalog/usgs-nwis.md) spends as much space on what NWIS
cannot tell you — the rating curve, the operating rule — as on what it publishes. That
section is the one most often left out of a catalog entry and the one most worth having.

**A ledger of what was examined and not used.** `concept/base-flow-separation` names the
published figures it excluded and the rule it excluded them under. It is a weak instrument
and the only auditable trace of selection that exists.

**A goal set that says what this corpus cannot show.**
[`bench/goals.yml`](.yidam/bench/goals.yml) is the fixed input to `yidam bench`: eight
questions with their complete expected answers, seven of them committed before either arm
was ever run and the eighth carrying `added_after:`, which says which change it was written
for and makes `bench` mark it rather than print it in the same column as the rest.
Two of the seven are there to lose. One asks something retrieval cannot express, one asks
something the ontology has no vocabulary for, and the file names both rather than reporting
five wins. It also states, in its own header, that a run over eight nodes is a regression
guard and not evidence for anything — the corpus is below the arithmetic floor of the claim
the benchmark is about.

**A calculator, and the commit it authors.**
[`capabilities.toml`](.yidam/capabilities.toml) declares three. `travel-tier` computes how far
each node's assertions may travel — the minimum claim tag over the node and the transitive
closure of its outgoing links. The prelude's own conduct guideline states that rule and states
that it must be *computed rather than declared*, because a declared tier drifts the moment a
supporting node is revised. A norm whose own words say "computed" and which nothing computes is
worth having here as the thing that computes it.

It is also what a `compute:` commit is for. The result carries its method rather than becoming a
property on a reach — which is the decision
[`base-flow-index-carries-its-method`](.yidam/decisions/base-flow-index-carries-its-method.yml)
already made about exactly this shape — and it reproduces no observation, so it is honest over a
corpus whose stations are illustrative.

Over this corpus the answer is worth reading. Every node travels as `[open]`, and **five of the
eight are downgraded**: they declare `[inference]` and rest on a chain reaching
`concept/base-flow-separation`, which is `[open]`. Nothing in this corpus may leave it. That is
the corpus telling the truth about itself, and no declared tier would have said so.

**A second step, and why it cannot run first.** `disclosure-envelope` reads
`.yidam/computed/travel-tier.yml` and partitions the corpus by where each node may go — the
question an operator asks with the first answer in hand. It declares
`after = ["travel-tier"]`, and that declaration is load-bearing rather than descriptive: its
`reads` name a file that exists only because the first step committed it, so a run that ignored
the ordering would invoke it against a tree with no input in it.

So `yidam run disclosure-envelope` against this corpus lands `travel-tier` first and then reads
what it wrote. Every capability also declares `.yidam/capabilities/**`, because a step stands in
a tree holding exactly what it declares and its own script is a file it depends on — which is
also what makes editing a calculator change the input state and re-run its step.

**The same rule in the other arm.**
[`travel-tier-typed.glu`](.yidam/capabilities/travel-tier-typed.glu) is the third capability, and
it is RFC-0042's downstream reference case: a calculator declared `run = { gluon = … }`, whose
entry point is typed `Corpus -> Computed` and which runs in the CLI's own process with no scratch
tree and no shell. It computes the same chain fixed point as `travel-tier.sh` — and agrees with it
exactly, over the same eight nodes and the same five downgrades.

What it commits is not the tier. A signal name is repository-wide, so two computed files may not
both claim `travels_as`; this one commits the rule's intermediate facts instead —
`weakest_beneath`, the node whose claim set the tier, and `chain_depth`, how many hops down that
node is. `travel-tier.sh` says how far each node's assertions travel and cannot say why; between
them the answer and its account are both in the corpus.

Two arms over one rule rather than one arm replacing the other, and that is not a preference. The
typed arm is behind a cargo feature outside the default set, so the binary `install.sh` downloads
reads this declaration, plans it, and declines the step by name — and refuses the whole plan while
it is in it, since a plan holding a step this binary cannot invoke is one that would half advance
the corpus. A bare `yidam run` therefore needs a build carrying `--features calculators-gluon`;
`yidam run travel-tier` and `yidam run disclosure-envelope` work in any build. The shell arm is
what keeps the rule reachable everywhere, which is why it stays exactly as it is.

`disclosure-envelope` is **not** portable to the typed arm and is worth saying so. It is a
pipeline's second stage: it reads the first step's committed signals, and the corpus the typed arm
is handed carries nodes, classes and resolved links but no previous step's answer. A second-stage
calculator can only be a shell one until that changes.

## Running the gates

```sh
cp -R examples/streamflow /tmp/streamflow
cd /tmp/streamflow && git init -q && git add -A && git commit -qm 'genesis: streamflow'
yidam graph-check     # 8 instances across 3 classes — all clean
yidam lint            # 0 finding(s), no errors
yidam open-questions  # four live questions

yidam run --dry-run   # the plan, in dependency order, writing nothing
yidam run             # two compute: commits, each with a receipt in .yidam/runs/
```

`run` writes commits and leaves the checkout alone, so after it the working tree is behind HEAD
— the report names the `git restore` that syncs it. Run it twice and the second skips both
steps as fresh, naming the reason, and writes nothing.

See [docs/quickstart.md](../../docs/quickstart.md) for the loop this is really for: watch the
gate pass, break it, watch it fail, repair it.
