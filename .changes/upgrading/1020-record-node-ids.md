### The consumption record names the nodes `retrieve` returned

**A `retrieve` line in `.yidam/record/calls.jsonl` now carries `node_ids` (#1020).**
It lists the ids `retrieve` returned, in rank order. Every other tool records `null`.

`yidam record` uses them to name the corpus nodes no recorded `retrieve` returned.
Its JSON report gains `consumption.reach`, which is `null` over a record written before this change.
A line without the key is still read. Nothing needs to change in a corpus.
