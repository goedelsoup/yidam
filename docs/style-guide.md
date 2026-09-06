# Documentation style guide

*What each kind of page owes its reader: sentence length, heading grammar, terminology, and how
long a page should be. Written to be checkable, because a standard nothing measures decays.*

These docs are not one register. A reader running `yidam bootstrap` for the first time and a
reader weighing an RFC's alternatives want opposite things from a sentence, and a rule that
serves one harms the other. So the corpus is tiered, and Simplified Technical English is applied
hardest where a misread costs the reader a broken repository.

## The three tiers

| Tier | Pages | Sentence ceiling |
|---|---|---|
| **1 — strict** | Task pages: `quickstart`, `installation`, `configuration`, `troubleshooting`, `editor-setup`, `upgrading`, `mcp-server`, `artifact-vaults`, `sharing-derivations`, `cli-reference` | 20 words |
| **2 — moderate** | Reference: `vocabulary`, `information-architecture`, `git-branch-model`, `bootstrap-flow`, `domain-computer`, `web-interface`, the Ontology group, the Governance group, the Quality group, this page | 25 words |
| **3 — voice preserved** | Argument and narrative: `what-yidam-is`, `aesthetic-direction`, `contributing`, `versioning`, `walkthroughs/`, `research/`, `rfcs/` | none |

### Tier 1 — strict

The reader is executing something. Write so a sentence cannot be misread at speed.

- **One instruction per sentence.** A step that does two things is two sentences.
- **Imperative mood, active voice, present tense.** "Run `yidam doctor`", not "`doctor` should
  then be run".
- **State what is true, not what is false.** "A vault holds bytes; git holds the record" beats
  "a vault is not a mutable store". Definition by negation makes the reader hold two things.
- **One term per concept, every time.** Do not vary a noun for prose rhythm. The corpus has 620
  `is not X` / `rather than` constructions and they cluster here; they are the first thing to
  rewrite.
- **Twenty words.** Above that, split at the conjunction.

### Tier 2 — moderate

The reader is looking something up. The ceiling loosens because the content is descriptive
rather than procedural, but terminology discipline is absolute: these are the pages that *define*
what the rest of the corpus spends.

- Twenty-five words.
- Every domain noun the page introduces appears in [vocabulary](vocabulary.md), spelled the same
  way there and here.
- No term used more than fifty times across `docs/` may go undefined.

### Tier 3 — voice preserved

The reader is following an argument. There is no sentence-length target and no register change:
the em-dash, the subordinate clause and the aphorism are the house voice, and
[aesthetic direction](aesthetic-direction.md) commits to it deliberately — "serious,
contemplative, precise". Em-dash density runs at roughly 0.3 per sentence across every group in
the corpus, evenly, which is what a deliberate style looks like rather than what decay looks
like.

Fix only what is mechanically broken:

- List items that end without terminal punctuation, which glue into unreadable runs.
- Metadata blocks written as prose. An RFC's `Relates to:` is a list, one reference per line.
- Terminology that contradicts [vocabulary](vocabulary.md).

## Heading grammar

Headings are scanned, not read. Within a page type they take one grammatical form, chosen to
match how the page is used.

| Page type | Heading form | Example |
|---|---|---|
| Task | Verb-first — what the reader does | `Get the CLI`, `Watch the gate stop you` |
| Troubleshooting | Symptom-first — what the reader sees | `` `serve --mcp` returns `degraded` `` |
| Reference | The noun being looked up | `` `.yidam/config.toml` ``, `Environment variables` |
| Argument | Free — the register is the point | "Where `foundational_type` stops being a field" |

Two rules apply everywhere:

- **Heading levels do not skip.** A page's first subheading is `##`, not `###`. This is a
  correctness rule, not a rendering one: Starlight's contents panel measures depth from the
  shallowest heading a page actually has, so an all-`###` page renders a flat panel that looks
  right. What is wrong is the document outline — a skipped level is an error to anything reading
  the page structurally, including assistive technology — and the inconsistency with every other
  page, which makes `###` mean "subsection" here and "section" there.
- **A heading a reader cannot match to a task is not a heading.** An evocative line that hides
  what the section is for belongs in the section's first sentence, not its title.

## Length bands

Bands are derived from the pages that already work, not imposed. A page far outside its band is
a signal to merge, split, or write more — not an automatic defect.

| Page type | Target |
|---|---|
| Entry / orientation | 800 – 1,600 words |
| Task | 1,200 – 2,600 words |
| Model / reference | 600 – 1,500 words |
| Ontology | 700 – 1,300 words |
| Governance | 400 – 900 words |
| Quality | 400 – 1,200 words |
| Walkthrough | 1,500 – 2,700 words |
| RFC | no cap — template compliance instead |

A page under its band either needs writing or does not deserve a sidebar entry of its own. A page
far over it is usually two pages.

## Terminology

[Vocabulary](vocabulary.md) is the approved-terms list. One word carries one meaning and one part
of speech. Where English gives one word several senses, the vocabulary names each sense
separately and the docs use the qualified form.

The live case is **artifact**, which carries three distinct meanings in this corpus:

- **artifact** — bare, means the vault sense: bytes too large, too derived or too licensed for
  git.
- **artifact node** — a corpus node describing a thing produced or found in the domain.
- **knowledge artifact** — a derived repository as a whole.

Write the qualified form whenever the bare word could be read either way.

## Tables of contents

The sidebar in `yidam/web/docs/astro.config.mjs` is the only contents table, and it is gated in
both directions: a built page missing from it fails the build, and an entry naming no page fails
too. Do not keep a second list of pages anywhere — an ungated copy drifts, and
[`docs/README.md`](README.md) carried one that had lost eight entries before it was removed.

## Anchors and links

Internal links are checked. `scripts/check-anchors.mjs` resolves every link to a page *and* to
the heading it names, so renaming a heading breaks any link that targets it. Rename headings
deliberately, and run `mise run docs-test` before pushing.

## Citing a line

A link may name lines in a file this repository owns:

```markdown
"the words the passage says" ([`checks.rs:12-19`](../checks.rs#L12-L19))
```

Three rules hold, and `cargo test --test line_citations` gates all three.

**Quote the passage.** The quoted words go immediately beside the link. Put them before it in
quotation marks, after it behind a colon, or above it in a blockquote the link attributes.
Nothing may sit between the quote and the link but punctuation.

**A quoteless citation is weaker than it looks.** Only its existence can be checked. It still
resolves after sliding onto the wrong lines, and 119 of this repository's 149 citations are in
that state. Add a quote where the target is prose.

**State the range twice.** The label says `checks.rs:12-19` and the fragment says `#L12-L19`. The
two are compared, so a repair must edit both.

Line numbers move. When the gate goes red, the finding names the range the passage moved to —
copy it. That range is only as wide as the quote, so widen it back out if your citation covered
more.
