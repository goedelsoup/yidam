# streamflow

*A worked yidam corpus: streamflow on regulated rivers.*

Eight instances across three classes, one catalog source, two decision records, one skill.
Small enough to read in ten minutes and structured enough that the ontology is doing real
work.

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

## Running the gates

```sh
cp -R examples/streamflow /tmp/streamflow
cd /tmp/streamflow && git init -q && git add -A && git commit -qm genesis
yidam graph-check     # 8 instances across 3 classes — all clean
yidam lint            # 0 finding(s), no errors
yidam open-questions  # four live questions
```

See [docs/quickstart.md](../../docs/quickstart.md) for the loop this is really for: watch the
gate pass, break it, watch it fail, repair it.
