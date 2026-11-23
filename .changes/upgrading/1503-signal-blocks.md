### A signal quoted in prose is a block that regen refreshes

**A new REGEN generator, `yidam signal`, writes one computed signal into prose (#1503, RFC-0058).**

In a node file, `yidam signal <name>` reads the node's own row.
Anywhere else, `yidam signal <node-ref> <name>` names the node first.
Three modifiers shape a number: `places=N`, `percent` and `grouped`.

```yaml
description: |
  Travels as <!-- REGEN: yidam signal travels_as -->open<!-- /REGEN --> below the impoundment.
```

**REGEN is now read in node files, inline blocks only.**

`regen --check`, `unclaimed` and `malformed-regen-block` all read `.yidam/corpus/` node files.
No node file held a REGEN block when this shipped, so no output changes until one is written.
`yidam run` refreshes every signal block in its `compute:` commit.
`yidam count` refreshes an inline count block in a node file too.
A write that would leave a node file unreadable as YAML is refused, and the file is kept.

**Readers strip the markers from node prose.**

`embed`, `export` and `lint`'s prose checks read a block's body and never its tags.
An older pin indexes the tags as prose without saying so.
That is why this release is a major bump.
Update the pin before writing the first block.

A corpus adopts the block by wrapping a quoted value in one whose body is that value.
The first `regen --check` passes, because nothing moved.
