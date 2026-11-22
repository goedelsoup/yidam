"""travel-support.py — what each node's travel tier rests on.

`travel-tier.sh` computes the tier: the weakest claim over a node and everything beneath it.
This computes the set that minimum is taken over. `supporting_nodes` is how many corpus nodes
sit beneath a node by its outgoing links, transitively, `instance-of` aside. `off_corpus_links`
is how many of the node's own links reach something that is not a corpus node here, which the
chain rule cannot rank. A tier that rests on nothing, or on a link it could not follow, reads
differently from one that rests on ten nodes, and the tier alone cannot say which it is.

It is the example of a calculator that uses the kit (RFC-0056 §2.3). It reads the graph with
`yidam_kit.read()` and writes the table with `yidam_kit.signals()`, so it has no parser and no
YAML writer of its own. The kit is a file it reads, so a kit update changes this step's input
state and the step re-runs. It runs under `python3 -I`, so nothing installed on the machine
reaches it.

A derived repository has the kit at `.yidam/.vendor/prelude/kit/yidam_kit.py`, and its step lists
`.yidam/.vendor/prelude/kit/**` in `reads`. This example vendors no prelude, so its copy sits
beside this script, where `.yidam/capabilities/**` already reads it. A gate holds the copy to the
prelude's, byte for byte.

Its signal names are its own. A name is corpus-wide, and `travel-tier` and `travel-tier-typed`
already hold theirs.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.environ["YIDAM_IN"], ".yidam/capabilities"))
import yidam_kit  # noqa: E402

graph = yidam_kit.read()
by_id = {n.id: n for n in graph.nodes}

# The links the chain rule follows: every outgoing link but `instance-of`, to a corpus node.
beneath = {
    n.id: [l.resolved for l in n.links if l.relationship != "instance-of" and l.resolved in by_id]
    for n in graph.nodes
}


def closure(start):
    # A walk with a seen set, not a recursion: a corpus graph may hold cycles.
    seen, todo = set(), list(beneath[start])
    while todo:
        b = todo.pop()
        if b not in seen and b != start:
            seen.add(b)
            todo.extend(beneath[b])
    return seen


rows = []
for n in graph.nodes:
    off = [l for l in n.links if l.relationship != "instance-of" and l.resolved not in by_id]
    rows.append({
        "node": n.ref,
        "supporting_nodes": len(closure(n.id)),
        "off_corpus_links": len(off),
    })

text = yidam_kit.signals(
    rows,
    method={
        "rule": "The transitive closure of a node's outgoing links, `instance-of` aside, over "
        "the corpus nodes here: the set `travel-tier` takes its minimum over.",
        "source": "guidelines/agent-conduct.md, \"When claims leave the repository\"",
    },
    summary={
        "nodes": len(rows),
        "resting_on_nothing": sum(1 for r in rows if r["supporting_nodes"] == 0),
    },
)

out = os.path.join(os.environ["YIDAM_OUT"], ".yidam/computed")
os.makedirs(out, exist_ok=True)
with open(os.path.join(out, "travel-support.yml"), "w", encoding="utf-8") as f:
    f.write("# Computed by the `travel-support` calculator and committed by `yidam run`.\n")
    f.write("# Recomputed from the corpus; edit the corpus, not this file.\n")
    f.write(text)
