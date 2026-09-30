### Lint warns on a node linked only to its class

**`only-instance-of` is a new warning (#1072).** It reports a node whose only link is to its
own `.ont.yml`. The finding names the relationships its class declares. Add one of them to
the node, or delete the node if it is a stub.

A class the ontology names only as an edge's target is a leaf by design. Its instances are
exempt. The warning never fails the gate.
