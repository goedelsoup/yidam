### `cycle` acts now say whether a command does them

Each act in `cycle`'s `next`, and each clock naming a remedy, now carries `kind`.
It is `tool` or `person`.
A `tool` act carries `argv`, and running it from the report's `root` is the act.
A `next` item also carries `targets`, naming the nodes, steps or refs it is about.

A loop can now run `tool` acts without parsing the prose `act` string.
Treat a missing `kind` as `person`, because an older `yidam` sends none.

The MCP contract is now 0.31.0.
Its `cycle` notes now also describe `stale_steps` and `stale_steps_unread`.
