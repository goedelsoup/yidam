# RFC-0046 — A quotation property type

- **Status:** Implemented
- **Track:** G13
- **Relates to:**
  - RFC-0040 (the precedent for a new declared type: one arm in `property_type_violation`, one in each SDK's schema compiler, one parity fixture)
  - RFC-0023 (the vault, whose rule that *a vault stores bytes and git stores the record of them* decides where the bytes a quotation is checked against come from)
  - RFC-0034 (the citation checks, whose whitespace normalization a span is compared under)
  - RFC-0018 (the query surface, which refuses to answer a question about a quotation it cannot read)
- **Versioning layers touched:** SDK parity 0.17.0 → 0.18.0 (the `quotation` arm in all three class schema compilers). Tooling: three `yidam lint` checks, one arm in `property-type`, `yidam rename` rewrites `of:`, and `query` refuses `~` on a quotation. **No MCP contract bump:** a quotation has no order, which the contract already says of every type but `date` and `number`, and the `~` refusal uses a code the contract already freezes. No migration.
- **Downstream reference case:** `ohio-136th-assembly-hb-96`'s `anchors`, 364 quotations of the Governor's veto message declared `type: text`, five of which were retyped from the wrong revision of the bill in one phase (#1070).

## Summary

A corpus could not say that a value **is** a span of a document it catalogued. This RFC adds
**one type, `quotation`**. Its value names a catalog entry and the words taken from it, and
lint compares the words with the bytes the vault fetched. There are three checks. One reports
a quotation that names nothing. One reports words the bytes do not hold. One reports, at Info,
a quotation whose bytes this machine cannot read.

## Problem

hb-96 keeps its anchors as prose in a `text` property. When five were retyped from the wrong
revision, every one of them still read like a quotation. The corpus's own drift gate compared
file paths, so it could not tell. Nothing in the ontology could say which document the words
came from, so nothing could check them.

The citation checks (RFC-0034) already compare a span with the text it cites, but only when
the text is a node. A catalog entry is a record of a document. The document's bytes live in
the vault, outside git.

## Proposal

### One type, and its shape

```yaml
properties:
  - name: anchors
    type: quotation
```

```yaml
anchors:
  - of: dewine-hb96-veto-messages
    span: "(4) If a school district is affected"
    sha256: 2a985bfe…   # optional; required when the entry holds more than one artifact
```

A value is one mapping or a non-empty list of them. `of:` and `span:` are required. `sha256:`
is optional. No other key is admitted. `property-type` reports any other shape, with one
parser, `quotations::read`, serving the gate, `migrate`'s retype and `query`. The compiled
class schema says the same thing: an object with those three properties and
`additionalProperties: false`, or a non-empty array of them.

`of:` resolves exactly as an edge's `source:` does, by stem or by path from the node's
directory, through `source_targets`. A quotation and an edge cannot disagree about which
entry a name means.

### Three checks, one partition

| Check | Severity | Reported when |
|---|---|---|
| `quotation-unresolved` | Error | `of:` names no catalog entry, the entry records no artifact, the pin names none of its artifacts, or the entry holds several and nothing pins one |
| `quotation-span-drift` | Error | the artifact's bytes are here, and the span is not in them |
| `quotation-unchecked` | Info | the quotation resolves and its bytes cannot be read here |

Every declared quotation lands in exactly one of the three, or in none when its span is found.
Both Error checks have an empty population until a corpus declares the type, so no existing
corpus fails the day this ships.

### The bytes come from the vault cache

A lint run can reach only the bytes this machine has fetched. It resolves the cache the way
`yidam vault` does (`YIDAM_VAULT_CACHE`, then `XDG_CACHE_HOME`, then `~/.cache/yidam/vault`).
A CI runner usually has none, and there every quotation is `quotation-unchecked`. That is
reported rather than passed, because a quotation nobody compared is a different fact from one
that was compared and held. It is not failed either, because an empty cache is not a defect in
the corpus. The check that matters runs where a quotation gets written, which is the machine
that fetched the document.

The cached bytes are hashed before they are read. A cache file that does not hash to its name
is not the artifact the entry records. It is reported unchecked, because drift found in it
would be a finding about the cache.

### Text media, and a PDF's recorded reading

A span is compared with the bytes as stored, markup included, after the whitespace
normalization every span check here uses. `text/*`, the `+xml` and `+json` suffixes, and
`application/json`, `xml` and `yaml` are text.

A PDF is compared with a text reading of it (#1172). `yidam catalog-extract` takes the reading
once, files it in the vault cache under its own digest, and records it beside the PDF:

```yaml
artifacts:
  - sha256: 2a985bfe…
    media_type: application/pdf
    text:
      sha256: 91c3e0d4…
      extractor: pdf-extract 0.12.1
```

Lint loads the reading by that digest and hashes it, as it does any artifact. It never runs an
extractor, so a crate bump cannot change a finding with nothing in the corpus changed. A PDF
with no reading recorded is reported unchecked, and the finding names `catalog-extract`.

The reading is nested under the PDF's record and not listed as a second artifact. A second
record would be a second revision, and every quotation of a one-PDF entry would suddenly need a
pin. The pin still names the PDF; the reading follows it.

Extracted text keeps a hyphen a line broke at: `inter- est` where the page shows `interest` over
a line break. The bytes do not say whether the hyphen was the author's. So at a hyphen that ends
a line, a span matches with the hyphen, without it, or as extracted. Everywhere else a hyphen is
compared like any other character.

### The pin is required exactly where there is a choice

An entry holding one artifact has one revision, and a quotation of it can only mean that one.
An entry holding several is the hb-96 case. A check that passed when *any* revision held the
words would pass the very retyping that motivated this RFC. So `sha256:` is optional, and it
is required when the entry holds more than one artifact.

### A rename moves the quotation with the entry

`yidam rename catalog/old new` rewrites every `of:` that resolves to the entry, alongside the
`source:` lines it already rewrote (#1159), each in the spelling its author chose. Without
this, renaming an entry would make every quotation of it `quotation-unresolved`.

### A query asks only whether a quotation is there

A quotation is a mapping, and the query executor compares scalars. `prop?` works. An
ordering is refused `unordered-property`, as on every type without an order. `=` is refused
`unsatisfiable-predicate` because a bare operand is not a quotation, and `!=` warns
`trivial-predicate`. `~` is refused `unsatisfiable-predicate`, because otherwise it would
match nothing and say nothing, which is an empty answer that reads as a true one.

### A quotation cites its entry

A declared quotation's `of:` counts as a citation of the entry it resolves to, as an edge
`source:` does (#1174). `catalog-uncited`, `verified-unsourced`, `catalog audit` and serve's
citations read one count. That count reads the declaration the three checks above read, so
nothing is counted that lint does not check. An `of:` counts when it resolves, whatever the pin
or the cache says. A value lint cannot read counts nothing.

## What this does not touch

- **Rename-blind history checks.** #1070 also asked whether `history.rs`, `lineage.rs` and
  `line_citations.rs` miss a rename written as a delete plus an add. That audit is #1171.
- **Other media.** An image, a spreadsheet or an archive has no reading, and a quotation of one
  is reported unchecked.

## Migration & compatibility

A corpus that coined `quotation` for itself had a type the gate left alone. After this RFC,
the gate reads it. Such a corpus either matches the shape above or renames its coined type.
`yidam migrate` retypes a property only when every existing value already satisfies the new
type, so retyping a `text` property to `quotation` is refused until the values are rewritten
as mappings.

## Alternatives considered

- **A span in prose, checked by pattern.** This is what hb-96 had. It cannot name a revision,
  and a rename silently severs it.
- **Failing an unchecked quotation.** Every CI run would fail every corpus that uses the type.
- **Fetching bytes during lint.** That makes lint depend on the network and on credentials,
  which RFC-0023 keeps out of the gate.
- **Requiring the pin always.** That would add a digest to every quotation of a
  single-artifact entry, where the digest can only have one value.

## Open questions

- ~~**Should a quotation count as a citation of its entry (#1174)?**~~ **Answered: yes.** A
  quotation names its entry more exactly than an edge `source:` does, and the edge already
  counted. See *A quotation cites its entry*.
