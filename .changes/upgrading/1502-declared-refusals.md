### A paragraph can be ruled not a refusal, and a corpus can ask to be asked

**A node may now declare `not_refusals:` beside `refuses:` (#1502, RFC-0057).**

Each entry's `span` quotes a paragraph a person ruled declines nothing.
`derive check` no longer proposes that paragraph as `derive-refusal-candidate`.
Neither list is read as the node's prose.

**`refusal-span-drift` now reports three more errors.**

It fails a drifted `not_refusals:` span, as it fails a drifted `refuses:` span.
It fails a span on either list that crosses a blank line.
It fails a paragraph that holds a span from each list.
No corpus declared a refusal when this shipped, so no existing corpus should see these.

**A new Info check, `refusal-undeclared`, runs only where a corpus asks.**

Add this to `.yidam/corpus/universal.yml` to turn it on:

```yaml
refusals:
  propose: true
```

It names each paragraph that reads like a refusal and that neither list rules on.
It never gates, because about four in ten of the paragraphs it names are not refusals.

A corpus that keeps a refusal ledger can move it into its nodes.
Each `refusal` row becomes a `refuses:` entry, and each `not-a-refusal` row a `not_refusals:` entry.
An older pin reads `not_refusals:` strings as prose, so update the pin in the same commit.
