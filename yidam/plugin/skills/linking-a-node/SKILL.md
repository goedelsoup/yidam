---
name: linking-a-node
description: Use before adding a markdown link between nodes in a yidam corpus — a repository with a .yidam/ directory. A class declares which relationships it may author and to what, and the yidam MCP server can be asked before the link is written. Triggers on "link", "edge", "relate two nodes", "which relationship", "add a link to a node", "connect these concepts".
---

# Linking a node

Markdown links between corpus files are the edges of the graph, and they are not free-form.
A class declares the relationships its instances may author, the class each targets, and the
direction each is written from. `yidam graph-lint` reads those declarations.

**Call `licensed_edges`** with the class name — the `<class>.ont.yml` stem, which is also the
directory its instances live in — before you write the link.

## Reading the answer

- `declares_edges: false` **does not mean the class licenses nothing.** It means the class has
  said nothing and the gate skips it. The two are opposite answers, and treating them as one
  is how you end up reporting every instance in a corpus whose ontology is not filled in.
  `note` says which case you are in, in words.
- `edges` carries **both directions**. The same relationship is documented from both ends, and
  the licensing check ignores direction — so a relationship listed as `direction: in` on your
  class is still the answer to "what may this link to".
- An **empty `target` licenses any target class**, rather than none.

## What licensing is not about

The `instance-of` link up to `../<class>.ont.yml` and a citation into `catalog/` are not
relationships, and no class declares them. A link to a file that is not there is a
`dangling-edge` finding, which is a different check with a different repair.

## If the tool is not there

`licensed_edges` is in the `ontology` capability. A server that holds nodes and edges but no
`.ont.yml` declares `"ontology": false` at connect time and refuses the tool with
`capability-not-supported`. That is a statement about the corpus, not a fault: there are no
class declarations to answer from, so no link is licensed or unlicensed. Read the node's
neighbours with `neighbors` and follow the corpus's existing convention instead.

## Where the reasoning is

`.yidam/.vendor/prelude/GRAPH.md` — the graph encoding, and why an orphan node is not yet
knowledge. Every node needs at least one outgoing link; that rule is in the prelude, and the
class declarations are what say *which* one.
