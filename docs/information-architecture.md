# Information architecture

### Top-level directories (in a derived repo)

| Directory | Role | Lifecycle |
|-----------|------|-----------|
| `agents/` | Domain agent definitions | Permanent |
| `crates/` | Rust domain computer — connectors, calculators, index | Permanent |
| `packages/` | Other-language packages (Python, TypeScript) | Permanent |
| `web/` | Web interface layer (optional) | Permanent |
| `docs/` | Repo-level documentation | Permanent |
| `.yidam/corpus/` | The knowledge graph nodes | Permanent |
| `.yidam/catalog/` | Data source provenance nodes | Permanent |
| `.yidam/decisions/` | Structured records of choices made at bootstrap and beyond | Permanent |
| `.yidam/sangha/` | Governance protocol and resolution records | Permanent |
| `.yidam/skills/` | Domain-specific reusable agent capabilities | Permanent |
| `.yidam/.vendor/` | Inherited yidam prelude; read-only in derived repos | Permanent |
| `samudaya/` | Pre-bootstrap seed layer | Consumed at genesis |
| `sadhana/` | Scaffold templates | Consumed at genesis |

### Corpus node structure

Each corpus node is a small, focused file:

- 2–10 sentences is typically right; 40 lines is the hard ceiling
- One concept per file; one file per concept
- Kebab-case, descriptive, stable filenames (renaming severs edges)
- Must have at least one outgoing link
- Uncertainty is valid if labeled: prefix title with `?` or open a branch

**Authored nodes** — written through deliberate knowledge work. Permanent, non-regenerable.

**Generated nodes** — produced by a pipeline from a primary source. Regenerable. Committed as
operational events, not epistemic events.

### Catalog node structure

- One file per data source
- Filename: `author-year.md` for papers, `slug.md` for datasets/APIs
- Content: source name, type, location, one-sentence description, access constraints
- Optional: `used-by` list of corpus node links for reverse traversal

### Ontology class definitions

During bootstrap, the schema layer is written to `.yidam/corpus/<class>.ont.yml`. If a
foundational ontology was chosen (BFO or UFO), each class carries a `foundational_type` field;
omit it entirely for "none" alignment:

```yaml
class: <name>
label: <Human-Readable Label>
foundational_type:           # omit if alignment is "none"
  ontology: bfo | ufo
  type: <bfo or ufo type value>
  iri: <url>                 # optional; exported as skos:exactMatch
description: |
  <one sentence>
properties:
  - name: <field>
    type: string | date | ref | text
    description: <one line>
edges:
  - relationship: <verb phrase>
    target: <class name>
    direction: out | in
    description: <one line>
```

**BFO type values** (partial): `material-entity`, `occurrent`, `process`, `quality`,
`disposition`, `role`, `function`, `site`

**UFO type values**: `kind`, `subkind`, `role`, `phase`, `relator`, `mode`, `quality`,
`event`, `situation`

### Corpus instance objects

```yaml
class: <class-name>
label: <Human-Readable Instance Name>
description: |
  <one or more sentences>
properties:
  <field>: <value>
links:
  - target: ../<other-class>/<other-instance>.yml
    relationship: <verb phrase>
  - target: ../<class>.ont.yml
    relationship: instance-of
```

### Decision records

`.yidam/decisions/<slug>.yml`:

```yaml
id: <slug>
summary: <one line>
context: |
  <what the choice was about>
decision: |
  <what was chosen>
rationale: |
  <why this, not alternatives considered>
```

### Resolution records

`.yidam/sangha/resolutions/<evolution>.md`:

```markdown
---
evolution: <name>
date: <YYYY-MM-DD>
synthesized-by: ma/<elector>
tips:
  - ma/<elector>@<short-hash>
---

## What was resolved
## What changed
## What remains open
```
