### An edge `source:` counts as a citation

**`catalog-uncited` and `verified-unsourced` read edge sources (#1158).** Before, only a
`links:` target or a prose link to a catalog entry counted as a citation. An entry cited only
from edge `source:` values was reported uncited. A node whose `[verified]` claim rested on such
an edge was reported unsourced. Yet `edge-source-unresolved` resolved every one of those edges.

Now the resolver every citation count shares reads edge sources too. It admits the same two
catalog spellings: the entry's file stem, or a path written as a `target:` is. `catalog audit`
and the MCP claim surface read the same function, so their counts move with the gate's.

Two checks may change on upgrade. `catalog-used-by-drift` compares an entry's hand-written
`used-by` list to the nodes citing it. More citations can mean more drift. On the largest corpus
we measured it rose from 79 to 86 findings, all Warn. `catalog-unobtained-but-cited` is Error and
now fires for an edge resting on an `obtained: false` entry. No corpus we measured had one.
