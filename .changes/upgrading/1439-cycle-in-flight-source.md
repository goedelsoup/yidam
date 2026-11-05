### `cycle` now spells an in-flight act `in_flight`

**`next[].source` for an in-flight phase is `"in_flight"`. It used to be `"inflight"` (#1439).** The schema always declared `"in_flight"`, so a validating consumer used to reject any `cycle` report that had a phase in flight.

**If you match on `source`, match `"in_flight"`.** This applies to a script or a prompt that reads `cycle --format json` or the MCP `cycle` tool. A match on `"inflight"` will stop finding anything.
